// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use arboard::Clipboard;
#[cfg(not(target_os = "windows"))]
use arboard::ImageData;
use base64::{engine::general_purpose, Engine as _};
use enigo::{Enigo, Key, Keyboard, Settings};
use serde::{Deserialize, Serialize};
#[cfg(not(target_os = "windows"))]
use std::borrow::Cow;
use std::fs;
use std::io::Cursor;
use std::mem::size_of;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY,
};

#[cfg(target_os = "windows")]
use windows::core::{Interface, VARIANT};

#[cfg(target_os = "windows")]
use windows::Win32::System::Com::IServiceProvider;

#[cfg(target_os = "windows")]
use windows::Win32::{
    System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    },
    UI::{
        Shell::{
            IFolderView, IPersistFolder2, IShellBrowser, IShellItem, IShellView, IShellWindows,
            SHCreateItemFromIDList, SID_STopLevelBrowser, ShellWindows, SIGDN_FILESYSPATH,
        },
        WindowsAndMessaging::{GetAncestor, GA_ROOT},
    },
};

#[derive(Serialize, Deserialize, Clone)]
struct FavoriteFolder {
    id: String,
    name: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct FavoriteItem {
    id: String,
    #[serde(rename = "type")]
    item_type: String,
    content: String,
    timestamp: u64,
    folder_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct FavoritesState {
    folders: Vec<FavoriteFolder>,
    items: Vec<FavoriteItem>,
}

fn favorites_file_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e:?}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create app data dir: {e:?}"))?;
    Ok(dir.join("favorites.json"))
}

#[cfg(target_os = "windows")]
fn send_ctrl_v() -> Result<(), String> {
    let ctrl = VIRTUAL_KEY(0x11);
    let v = VIRTUAL_KEY(0x56);

    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: ctrl,
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: v,
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: v,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: ctrl,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err(format!("SendInput sent {sent} events"))
    }
}

#[tauri::command]
fn simulate_paste() {
    // Wait for window to fully hide and focus to return to previous app
    std::thread::sleep(std::time::Duration::from_millis(100));

    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    // Simulate Ctrl+V
    let _ = enigo.key(Key::Control, enigo::Direction::Press);
    let _ = enigo.key(Key::Unicode('v'), enigo::Direction::Click);
    let _ = enigo.key(Key::Control, enigo::Direction::Release);
}

fn try_set_clipboard_text(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| format!("Failed to init clipboard: {e:?}"))?;

    let mut last_err: Option<String> = None;
    for _ in 0..8 {
        match clipboard.set_text(text.to_string()) {
            Ok(_) => {
                last_err = None;
                break;
            }
            Err(e) => {
                last_err = Some(format!("Failed to set clipboard text: {e:?}"));
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
        }
    }

    if let Some(err) = last_err {
        Err(err)
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn try_set_clipboard_image(width: usize, height: usize, bytes: Vec<u8>) -> Result<(), String> {
    use std::ptr;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    const CF_DIB: u32 = 8;

    // RGBA to BGRA conversion (Windows DIB uses BGRA)
    let mut bgra = bytes.clone();
    for chunk in bgra.chunks_exact_mut(4) {
        chunk.swap(0, 2); // Swap R and B
    }

    // Windows DIB is bottom-up, so we need to flip rows
    let row_size = width * 4;
    let mut flipped = vec![0u8; bgra.len()];
    for y in 0..height {
        let src_start = y * row_size;
        let dst_start = (height - 1 - y) * row_size;
        flipped[dst_start..dst_start + row_size]
            .copy_from_slice(&bgra[src_start..src_start + row_size]);
    }

    // Build BITMAPINFOHEADER (40 bytes) + pixel data
    let header_size = 40usize;
    let dib_size = header_size + flipped.len();
    let mut dib_data: Vec<u8> = Vec::with_capacity(dib_size);

    // BITMAPINFOHEADER structure
    dib_data.extend_from_slice(&40u32.to_le_bytes()); // biSize
    dib_data.extend_from_slice(&(width as i32).to_le_bytes()); // biWidth
    dib_data.extend_from_slice(&(height as i32).to_le_bytes()); // biHeight (positive = bottom-up)
    dib_data.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    dib_data.extend_from_slice(&32u16.to_le_bytes()); // biBitCount (32 bits per pixel)
    dib_data.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB = 0)
    dib_data.extend_from_slice(&(flipped.len() as u32).to_le_bytes()); // biSizeImage
    dib_data.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    dib_data.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    dib_data.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    dib_data.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant
    dib_data.extend_from_slice(&flipped); // Pixel data

    eprintln!(
        "try_set_clipboard_image: width={}, height={}, dib_size={}",
        width,
        height,
        dib_data.len()
    );

    let mut last_err: Option<String> = None;

    for attempt in 0..8 {
        unsafe {
            eprintln!("try_set_clipboard_image: attempt {}", attempt);

            // Open clipboard
            if let Err(e) = OpenClipboard(HWND::default()) {
                eprintln!("try_set_clipboard_image: OpenClipboard failed: {:?}", e);
                last_err = Some(format!("Failed to open clipboard: {:?}", e));
                std::thread::sleep(std::time::Duration::from_millis(40));
                continue;
            }
            eprintln!("try_set_clipboard_image: OpenClipboard succeeded");

            // Empty clipboard
            if let Err(e) = EmptyClipboard() {
                eprintln!("try_set_clipboard_image: EmptyClipboard failed: {:?}", e);
                let _ = CloseClipboard();
                last_err = Some(format!("Failed to empty clipboard: {:?}", e));
                std::thread::sleep(std::time::Duration::from_millis(40));
                continue;
            }
            eprintln!("try_set_clipboard_image: EmptyClipboard succeeded");

            // Allocate global memory
            let hmem = match GlobalAlloc(GMEM_MOVEABLE, dib_data.len()) {
                Ok(h) => {
                    eprintln!(
                        "try_set_clipboard_image: GlobalAlloc succeeded, handle={:?}",
                        h
                    );
                    h
                }
                Err(e) => {
                    eprintln!("try_set_clipboard_image: GlobalAlloc failed: {:?}", e);
                    let _ = CloseClipboard();
                    last_err = Some(format!("Failed to allocate global memory: {e:?}"));
                    std::thread::sleep(std::time::Duration::from_millis(40));
                    continue;
                }
            };

            // Lock memory and copy data
            let pmem = GlobalLock(hmem);
            if pmem.is_null() {
                eprintln!("try_set_clipboard_image: GlobalLock returned null");
                let _ = CloseClipboard();
                last_err = Some("Failed to lock global memory".to_string());
                std::thread::sleep(std::time::Duration::from_millis(40));
                continue;
            }
            eprintln!("try_set_clipboard_image: GlobalLock succeeded");

            ptr::copy_nonoverlapping(dib_data.as_ptr(), pmem as *mut u8, dib_data.len());
            let _ = GlobalUnlock(hmem);
            eprintln!("try_set_clipboard_image: Data copied and unlocked");

            // Set clipboard data - use raw handle value
            let handle = windows::Win32::Foundation::HANDLE(hmem.0);
            eprintln!(
                "try_set_clipboard_image: Calling SetClipboardData with CF_DIB={}, handle={:?}",
                CF_DIB, handle
            );
            let result = SetClipboardData(CF_DIB, handle);

            if let Err(e) = &result {
                eprintln!("try_set_clipboard_image: SetClipboardData failed: {:?}", e);
                let _ = CloseClipboard();
                last_err = Some(format!("Failed to set clipboard data: {:?}", e));
                std::thread::sleep(std::time::Duration::from_millis(40));
                continue;
            }
            eprintln!(
                "try_set_clipboard_image: SetClipboardData succeeded: {:?}",
                result
            );

            let _ = CloseClipboard();
            eprintln!("try_set_clipboard_image: CloseClipboard done, SUCCESS!");

            // Success
            last_err = None;
            break;
        }
    }

    if let Some(ref err) = last_err {
        eprintln!("try_set_clipboard_image: FAILED with error: {}", err);
        Err(err.clone())
    } else {
        eprintln!("try_set_clipboard_image: completed successfully");
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn try_set_clipboard_image(width: usize, height: usize, bytes: Vec<u8>) -> Result<(), String> {
    let mut last_err: Option<String> = None;
    for _ in 0..8 {
        let mut clipboard = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                last_err = Some(format!("Failed to init clipboard: {e:?}"));
                std::thread::sleep(std::time::Duration::from_millis(40));
                continue;
            }
        };
        match clipboard.set_image(ImageData {
            width,
            height,
            bytes: Cow::Borrowed(&bytes),
        }) {
            Ok(_) => {
                last_err = None;
                break;
            }
            Err(e) => {
                last_err = Some(format!("Failed to set clipboard image: {e:?}"));
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
        }
    }

    if let Some(err) = last_err {
        Err(err)
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn try_get_explorer_folder_from_hwnd(hwnd: usize) -> Option<std::path::PathBuf> {
    // Best-effort: resolve the folder path of the last foreground Explorer window.
    // If we can't resolve a filesystem path (virtual folder, non-Explorer app, etc.), return None.
    unsafe {
        let root_hwnd = GetAncestor(
            windows::Win32::Foundation::HWND(hwnd as *mut std::ffi::c_void),
            GA_ROOT,
        );
        if root_hwnd.0.is_null() {
            return None;
        }
        let root_hwnd_val = root_hwnd.0 as isize;

        // Shell window APIs generally prefer STA.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let result = (|| {
            let shell_windows: IShellWindows =
                CoCreateInstance(&ShellWindows, None, CLSCTX_ALL).ok()?;

            let count = shell_windows.Count().ok()?;
            for i in 0..count {
                let disp = shell_windows.Item(&VARIANT::from(i)).ok()?;
                let wb: windows::Win32::UI::Shell::IWebBrowserApp = disp.cast().ok()?;

                let wb_hwnd = wb.HWND().ok()?;
                if wb_hwnd.0 != root_hwnd_val {
                    continue;
                }

                let sp: IServiceProvider = wb.cast().ok()?;
                let sb: IShellBrowser = sp.QueryService(&SID_STopLevelBrowser).ok()?;

                let view: IShellView = sb.QueryActiveShellView().ok()?;
                let fv: IFolderView = view.cast().ok()?;
                let pf: IPersistFolder2 = fv.GetFolder().ok()?;

                let pidl = pf.GetCurFolder().ok()?;
                let item: IShellItem = SHCreateItemFromIDList(pidl).ok()?;
                let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
                let s = p.to_string().ok()?;
                let path = std::path::PathBuf::from(s);
                if path.is_dir() {
                    return Some(path);
                }

                return None;
            }

            None
        })();

        CoUninitialize();
        result
    }
}

fn save_bytes_to_default_dir(
    app: &tauri::AppHandle,
    bytes: &[u8],
    extension: &str,
) -> Result<String, String> {
    // Prefer last active Explorer folder (Windows only). Otherwise fall back to Downloads/Coppy.
    #[cfg(target_os = "windows")]
    let base_dir = {
        let hwnd = key_listener::last_foreground_hwnd();
        if hwnd != 0 {
            if let Some(p) = try_get_explorer_folder_from_hwnd(hwnd) {
                p
            } else {
                app.path()
                    .download_dir()
                    .or_else(|_| app.path().app_data_dir())
                    .map_err(|e| format!("Failed to get output dir: {e:?}"))?
            }
        } else {
            app.path()
                .download_dir()
                .or_else(|_| app.path().app_data_dir())
                .map_err(|e| format!("Failed to get output dir: {e:?}"))?
        }
    };

    #[cfg(not(target_os = "windows"))]
    let base_dir = app
        .path()
        .download_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|e| format!("Failed to get output dir: {e:?}"))?;

    let out_dir = base_dir.join("Coppy");
    fs::create_dir_all(&out_dir).map_err(|e| format!("Failed to create output dir: {e:?}"))?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Failed to read system time: {e:?}"))?
        .as_millis();

    let file_name = format!("coppy_{ts}.{extension}");
    let path = out_dir.join(file_name);

    fs::write(&path, bytes).map_err(|e| format!("Failed to write file: {e:?}"))?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn save_image_data_url(app: tauri::AppHandle, data_url: String) -> Result<String, String> {
    let (meta, b64) = data_url
        .split_once(',')
        .ok_or_else(|| "Invalid data URL".to_string())?;

    let mut bytes = general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("Failed to decode base64: {e:?}"))?;

    let mut extension = if meta.contains("image/jpeg") || meta.contains("image/jpg") {
        "jpg"
    } else {
        "png"
    };

    // Data URLs come from our own encoder (clipboard_listener) as PNG.
    // Still, keep a small safety net: if decode/format is odd, re-encode to PNG.
    if extension == "png" {
        let img = image::load_from_memory(&bytes)
            .map_err(|e| format!("Failed to decode image: {e:?}"))?;
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .map_err(|e| format!("Failed to encode PNG: {e:?}"))?;
        bytes = out;
        extension = "png";
    }

    save_bytes_to_default_dir(&app, &bytes, extension)
}

#[tauri::command]
fn set_clipboard_text(text: String) -> Result<(), String> {
    try_set_clipboard_text(&text)
}

#[tauri::command]
fn set_clipboard_image(app: tauri::AppHandle, data_url: String) -> Result<(), String> {
    eprintln!("set_clipboard_image: start");

    let b64 = data_url
        .split_once(',')
        .map(|(_, b64)| b64)
        .ok_or_else(|| "Invalid data URL".to_string())?;
    let bytes = general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("Failed to decode base64: {e:?}"))?;

    // Save image to temp file for CF_HDROP (Explorer paste)
    let temp_path = save_image_to_temp(&app, &bytes)?;
    eprintln!("set_clipboard_image: saved to temp file: {}", temp_path);

    // Set clipboard with CF_HDROP (file drop) for Explorer
    set_clipboard_file(&temp_path)?;

    eprintln!("set_clipboard_image: done");
    Ok(())
}

#[cfg(target_os = "windows")]
fn save_image_to_temp(app: &tauri::AppHandle, bytes: &[u8]) -> Result<String, String> {
    use std::io::Cursor;

    // Decode and re-encode as PNG to ensure valid format
    let img =
        image::load_from_memory(bytes).map_err(|e| format!("Failed to decode image: {e:?}"))?;

    let temp_dir = app
        .path()
        .temp_dir()
        .map_err(|e| format!("Failed to get temp dir: {e:?}"))?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Failed to get time: {e:?}"))?
        .as_millis();

    let file_name = format!("coppy_clipboard_{}.png", ts);
    let file_path = temp_dir.join(file_name);

    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {e:?}"))?;

    fs::write(&file_path, &out).map_err(|e| format!("Failed to write temp file: {e:?}"))?;

    Ok(file_path.to_string_lossy().to_string())
}

#[cfg(not(target_os = "windows"))]
fn save_image_to_temp(_app: &tauri::AppHandle, _bytes: &[u8]) -> Result<String, String> {
    Err("Not implemented on this platform".to_string())
}

#[cfg(target_os = "windows")]
fn set_clipboard_file(file_path: &str) -> Result<(), String> {
    use std::ptr;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::System::Ole::CF_HDROP;

    // Convert path to wide string (UTF-16) with null terminator
    let wide_path: Vec<u16> = file_path.encode_utf16().chain(std::iter::once(0)).collect();

    // DROPFILES structure size (20 bytes) + file path (UTF-16) + double null terminator
    let dropfiles_size = 20usize;
    let path_bytes = wide_path.len() * 2; // Each UTF-16 char is 2 bytes
    let total_size = dropfiles_size + path_bytes + 2; // +2 for extra null terminator

    unsafe {
        // Open clipboard
        if OpenClipboard(HWND::default()).is_err() {
            return Err("Failed to open clipboard".to_string());
        }

        // Empty clipboard
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return Err("Failed to empty clipboard".to_string());
        }

        // Allocate global memory
        let hmem = match GlobalAlloc(GMEM_MOVEABLE, total_size) {
            Ok(h) => h,
            Err(e) => {
                let _ = CloseClipboard();
                return Err(format!("Failed to allocate memory: {e:?}"));
            }
        };

        let pmem = GlobalLock(hmem);
        if pmem.is_null() {
            let _ = CloseClipboard();
            return Err("Failed to lock memory".to_string());
        }

        // DROPFILES structure
        // pFiles (4 bytes): offset to file list = 20 (size of DROPFILES)
        // pt.x (4 bytes): 0
        // pt.y (4 bytes): 0
        // fNC (4 bytes): 0
        // fWide (4 bytes): 1 (Unicode)
        let dropfiles: [u8; 20] = [
            20, 0, 0, 0, // pFiles = 20
            0, 0, 0, 0, // pt.x = 0
            0, 0, 0, 0, // pt.y = 0
            0, 0, 0, 0, // fNC = 0
            1, 0, 0, 0, // fWide = 1 (TRUE)
        ];

        ptr::copy_nonoverlapping(dropfiles.as_ptr(), pmem as *mut u8, 20);

        // Copy file path as UTF-16
        let path_dest = (pmem as *mut u8).add(20) as *mut u16;
        ptr::copy_nonoverlapping(wide_path.as_ptr(), path_dest, wide_path.len());

        // Add extra null terminator at the end
        let end = path_dest.add(wide_path.len());
        *end = 0;

        let _ = GlobalUnlock(hmem);

        // Set clipboard data - CF_HDROP = 15
        let result = SetClipboardData(
            CF_HDROP.0 as u32,
            windows::Win32::Foundation::HANDLE(hmem.0),
        );
        let _ = CloseClipboard();

        if result.is_err() {
            return Err("Failed to set clipboard data".to_string());
        }

        eprintln!("set_clipboard_file: CF_HDROP set successfully");
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn set_clipboard_file(_file_path: &str) -> Result<(), String> {
    Err("Not implemented on this platform".to_string())
}

#[tauri::command]
fn paste_text(app: tauri::AppHandle, text: String) -> Result<(), String> {
    eprintln!("paste_text: start");

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    if let Err(err) = try_set_clipboard_text(&text) {
        eprintln!("paste_text: {err}");
        return Err(err);
    }

    #[cfg(target_os = "windows")]
    {
        key_listener::focus_last_foreground_window();
    }

    std::thread::sleep(std::time::Duration::from_millis(320));

    #[cfg(target_os = "windows")]
    {
        send_ctrl_v().map_err(|e| format!("Failed to send Ctrl+V: {e}"))?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut enigo =
            Enigo::new(&Settings::default()).map_err(|e| format!("Failed to init enigo: {e:?}"))?;
        let _ = enigo.key(Key::Control, enigo::Direction::Press);
        let _ = enigo.key(Key::Unicode('v'), enigo::Direction::Click);
        let _ = enigo.key(Key::Control, enigo::Direction::Release);
    }
    eprintln!("paste_text: done");
    Ok(())
}

#[tauri::command]
fn paste_image(app: tauri::AppHandle, data_url: String) -> Result<(), String> {
    eprintln!("paste_image: start");

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    let b64 = data_url
        .split_once(',')
        .map(|(_, b64)| b64)
        .ok_or_else(|| "Invalid data URL".to_string())?;
    let bytes = general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("Failed to decode base64: {e:?}"))?;

    let img = image::load_from_memory(&bytes)
        .map_err(|e| format!("Failed to decode image: {e:?}"))?
        .to_rgba8();

    let (width, height) = img.dimensions();
    let raw = img.into_raw();

    try_set_clipboard_image(width as usize, height as usize, raw)?;

    #[cfg(target_os = "windows")]
    {
        key_listener::focus_last_foreground_window();
    }

    std::thread::sleep(std::time::Duration::from_millis(320));

    #[cfg(target_os = "windows")]
    {
        send_ctrl_v().map_err(|e| format!("Failed to send Ctrl+V: {e}"))?;
    }

    eprintln!("paste_image: done");
    Ok(())
}

#[tauri::command]
fn load_favorites(app: tauri::AppHandle) -> Result<FavoritesState, String> {
    let path = favorites_file_path(&app)?;
    if !path.exists() {
        return Ok(FavoritesState {
            folders: Vec::new(),
            items: Vec::new(),
        });
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("Failed to read favorites: {e:?}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("Failed to parse favorites: {e:?}"))
}

#[tauri::command]
fn save_favorites(app: tauri::AppHandle, state: FavoritesState) -> Result<(), String> {
    let path = favorites_file_path(&app)?;
    let raw = serde_json::to_string(&state)
        .map_err(|e| format!("Failed to serialize favorites: {e:?}"))?;
    fs::write(&path, raw).map_err(|e| format!("Failed to write favorites: {e:?}"))?;
    Ok(())
}

#[tauri::command]
fn autostart_is_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    {
        use tauri_plugin_autostart::ManagerExt;
        return app
            .autolaunch()
            .is_enabled()
            .map_err(|e| format!("Failed to read autostart state: {e:?}"));
    }

    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        let _ = app;
        Err("Autostart is not supported on this platform".to_string())
    }
}

#[tauri::command]
fn autostart_enable(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    {
        use tauri_plugin_autostart::ManagerExt;
        return app
            .autolaunch()
            .enable()
            .map_err(|e| format!("Failed to enable autostart: {e:?}"));
    }

    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        let _ = app;
        Err("Autostart is not supported on this platform".to_string())
    }
}

#[tauri::command]
fn autostart_disable(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    {
        use tauri_plugin_autostart::ManagerExt;
        return app
            .autolaunch()
            .disable()
            .map_err(|e| format!("Failed to disable autostart: {e:?}"));
    }

    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        let _ = app;
        Err("Autostart is not supported on this platform".to_string())
    }
}

mod clipboard_listener;
mod key_listener;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    let builder = { builder.plugin(tauri_plugin_autostart::Builder::new().build()) };

    builder
        .setup(|app| {
            #[cfg(target_os = "windows")]
            key_listener::start_listening(app.handle().clone());

            clipboard_listener::start(app.handle().clone());

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            simulate_paste,
            set_clipboard_text,
            set_clipboard_image,
            paste_text,
            paste_image,
            save_image_data_url,
            load_favorites,
            save_favorites,
            autostart_is_enabled,
            autostart_enable,
            autostart_disable
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
