// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose, Engine as _};
use enigo::{Enigo, Keyboard, Key, Settings};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fs;
use std::mem::size_of;
use tauri::Manager;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY,
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

#[tauri::command]
fn paste_text(app: tauri::AppHandle, text: String) -> Result<(), String> {
    eprintln!("paste_text: start");

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    let mut clipboard = Clipboard::new().map_err(|e| format!("Failed to init clipboard: {e:?}"))?;

    let mut last_err: Option<String> = None;
    for _ in 0..8 {
        match clipboard.set_text(text.clone()) {
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
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| format!("Failed to init enigo: {e:?}"))?;
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

     let mut clipboard = Clipboard::new().map_err(|e| format!("Failed to init clipboard: {e:?}"))?;
     clipboard
         .set_image(ImageData {
             width: width as usize,
             height: height as usize,
             bytes: Cow::Owned(raw),
         })
         .map_err(|e| format!("Failed to set clipboard image: {e:?}"))?;

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
     let raw = serde_json::to_string(&state).map_err(|e| format!("Failed to serialize favorites: {e:?}"))?;
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

mod key_listener;
mod clipboard_listener;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    let builder = {
        use tauri_plugin_autostart::MacosLauncher;
        builder.plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
    };

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
            paste_text,
            paste_image,
            load_favorites,
            save_favorites,
            autostart_is_enabled,
            autostart_enable,
            autostart_disable
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
