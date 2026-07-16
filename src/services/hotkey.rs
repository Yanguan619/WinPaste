/// Global hotkey system with dedicated hidden window and message pump.
use std::sync::OnceLock;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, RegisterClassW, DefWindowProcW, GetMessageW, TranslateMessage,
    DispatchMessageW, MSG, WNDCLASSW, WM_HOTKEY, PostQuitMessage,
    WINDOW_EX_STYLE, WINDOW_STYLE,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS};
use windows::core::w;

use crate::{error, info};

const HOTKEY_WINDOW_CLASS: &str = "ClipboardHotkeyWindow";
const MAIN_HOTKEY_ID: i32 = 1;
static HOTKEY_HWND: OnceLock<usize> = OnceLock::new();

pub fn parse_hotkey_string(hotkey: &str) -> Option<(HOT_KEY_MODIFIERS, u32)> {
    if hotkey.is_empty() {
        return None;
    }

    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut win = false;
    let mut vk: u32 = 0;

    for part in hotkey.split('+') {
        let p = part.trim().to_lowercase();
        match p.as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            "win" | "super" => win = true,
            _ => {
                if p.len() == 1 {
                    let ch = p.chars().next().unwrap();
                    if ch.is_ascii_alphanumeric() {
                        vk = ch.to_uppercase().next().unwrap() as u32;
                    }
                }
            }
        }
    }

    if vk == 0 {
        return None;
    }

    let mut modifiers = HOT_KEY_MODIFIERS(0);
    if ctrl { modifiers |= HOT_KEY_MODIFIERS(0x0002); } // MOD_CONTROL
    if shift { modifiers |= HOT_KEY_MODIFIERS(0x0004); } // MOD_SHIFT
    if alt { modifiers |= HOT_KEY_MODIFIERS(0x0001); } // MOD_ALT
    if win { modifiers |= HOT_KEY_MODIFIERS(0x0008); } // MOD_WIN

    Some((modifiers, vk))
}

unsafe extern "system" fn hotkey_wnd_proc(
    hwnd: HWND,
    msg: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT { unsafe {
    match msg {
        WM_HOTKEY => {
            let hotkey_id = w_param.0 as i32;
            if hotkey_id == MAIN_HOTKEY_ID {
                crate::app::window_manager::toggle_window();
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, w_param, l_param),
    }
}}

pub fn start_global_hotkey() {
    std::thread::spawn(|| unsafe {
        let h_instance = GetModuleHandleW(None).expect("Failed to get module handle");

        let class_name: Vec<u16> = HOTKEY_WINDOW_CLASS.encode_utf16().chain(std::iter::once(0)).collect();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(hotkey_wnd_proc),
            hInstance: h_instance.into(),
            lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class_name.as_ptr()),
            w!("Clipboard Hotkey"),
            WINDOW_STYLE(0),
            0, 0, 0, 0,
            None,
            None,
            Some(h_instance.into()),
            None,
        );

        if let Ok(h) = hwnd {
            let hwnd_val = h.0 as usize;
            let _ = HOTKEY_HWND.set(hwnd_val);

            // Register Ctrl+Shift+V
            let modifiers = HOT_KEY_MODIFIERS(0x0002 | 0x0004); // MOD_CONTROL | MOD_SHIFT
            if let Err(e) = RegisterHotKey(Some(h), MAIN_HOTKEY_ID, modifiers, 0x56) {
                error!("Failed to register Ctrl+Shift+V: {:?}", e);
            } else {
                info!("Registered global hotkey: Ctrl+Shift+V");
            }

            let mut msg = MSG::default();
            while !crate::state::global_state::SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed) && GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                let _ = DispatchMessageW(&msg);
            }

            let _ = UnregisterHotKey(Some(h), MAIN_HOTKEY_ID);
        } else {
            error!("Failed to create hotkey window");
        }
    });
}

pub fn stop_global_hotkey() {
    if let Some(&hwnd) = HOTKEY_HWND.get() {
        unsafe {
            let h = HWND(hwnd as *mut _);
            let _ = UnregisterHotKey(Some(h), MAIN_HOTKEY_ID);
            PostQuitMessage(0);
        }
    }
}
