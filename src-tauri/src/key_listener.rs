use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, PhysicalPosition};
use core::ffi::c_void;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, POINT};
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LCONTROL, VK_RCONTROL};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetCursorPos, GetForegroundWindow, GetMessageA, SetForegroundWindow,
    SetWindowsHookExA, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

static LAST_CTRL_RELEASE: AtomicI64 = AtomicI64::new(0);
static LAST_FOREGROUND_HWND: AtomicUsize = AtomicUsize::new(0);
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub fn focus_last_foreground_window() {
    let hwnd_val = LAST_FOREGROUND_HWND.load(Ordering::SeqCst);
    if hwnd_val != 0 {
        unsafe {
            let ok = SetForegroundWindow(HWND(hwnd_val as *mut c_void)).as_bool();
            if !ok {
                eprintln!("focus_last_foreground_window: SetForegroundWindow failed");
            }
        }
    }
}

pub fn start_listening(app: AppHandle) {
    let _ = APP_HANDLE.set(app);

    std::thread::spawn(|| unsafe {
        let instance = GetModuleHandleA(None).unwrap();
        let hook = SetWindowsHookExA(WH_KEYBOARD_LL, Some(hook_callback), instance, 0);

        if hook.is_err() {
            eprintln!("Failed to set keyboard hook");
            return;
        }
        let hook = hook.unwrap();

        let mut msg = MSG::default();
        while GetMessageA(&mut msg, None, 0, 0).into() {
            // Processing loop
        }
        
        let _ = UnhookWindowsHookEx(hook);
    });
}

unsafe extern "system" fn hook_callback(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code >= 0 {
        let vk_code = (*(l_param.0 as *const KBDLLHOOKSTRUCT)).vkCode;
        let event = w_param.0 as u32;

        if vk_code == VK_LCONTROL.0 as u32 || vk_code == VK_RCONTROL.0 as u32 {
             let flags = (*(l_param.0 as *const KBDLLHOOKSTRUCT)).flags.0;
             let is_up = (flags >> 7) & 1 == 1;
             
             if !is_up && (event == WM_KEYDOWN || event == WM_SYSKEYDOWN) {
                 // Key Down
             } else if is_up && (event == WM_KEYUP || event == WM_SYSKEYUP) {
                 // Key Up
                 let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64;
                 
                 let last = LAST_CTRL_RELEASE.load(Ordering::SeqCst);
                 
                 if (now - last) < 400 {
                     // Double click detected!
                     if let Some(app) = APP_HANDLE.get() {
                         if let Some(window) = app.get_webview_window("main") {
                             if window.is_visible().unwrap_or(false) {
                                 let _ = window.hide();
                             } else {
                                 let fg = GetForegroundWindow();
                                 LAST_FOREGROUND_HWND.store(fg.0 as usize, Ordering::SeqCst);
                                 let mut point = POINT::default();
                                 let _ = GetCursorPos(&mut point);
                                 let _ = window.set_position(PhysicalPosition::new(point.x, point.y));
                                 let _ = window.show();
                                 let _ = window.set_focus();
                             }
                         }
                     }
                     LAST_CTRL_RELEASE.store(0, Ordering::SeqCst); // Reset
                 } else {
                     LAST_CTRL_RELEASE.store(now, Ordering::SeqCst);
                 }
             }
        }
    }
    CallNextHookEx(None, code, w_param, l_param)
}
