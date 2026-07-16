/// Win+V interception via low-level keyboard hook.
/// When the user presses Win+V, we intercept it and show our clipboard manager instead.
use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, TranslateMessage, DispatchMessageW,
    UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

use crate::state::global_state::*;
use crate::{error, info};

static mut WINV_HOOK: HHOOK = HHOOK(std::ptr::null_mut());

const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;
const VK_V: u32 = 0x56;

unsafe extern "system" fn win_v_hook_proc(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT { unsafe {
    if code >= 0 {
        let is_key_down = w_param.0 == WM_KEYDOWN as usize || w_param.0 == WM_SYSKEYDOWN as usize;
        if is_key_down {
            let kb = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
            let vk = kb.vkCode;

            // Check if Win+V is pressed
            if (vk == VK_V) && !IS_RECORDING.load(Ordering::Relaxed) {
                let win_held = get_async_key_state(VK_LWIN as i32) < 0 || get_async_key_state(VK_RWIN as i32) < 0;
                if win_held {
                    // Intercept Win+V: show our window instead
                    crate::app::window_manager::toggle_window();
                    return LRESULT(1); // Consume the key
                }
            }
        }
    }
    CallNextHookEx(Some(WINV_HOOK), code, w_param, l_param)
}}

fn get_async_key_state(v_key: i32) -> i16 {
    unsafe {
        type GetAsyncKeyStateFunc = unsafe extern "system" fn(i32) -> i16;
        static GET_ASYNC_KEY_STATE: std::sync::OnceLock<GetAsyncKeyStateFunc> = std::sync::OnceLock::new();
        let func = GET_ASYNC_KEY_STATE.get_or_init(|| {
            let h = windows::Win32::System::LibraryLoader::GetModuleHandleW(
                windows::core::PCWSTR(b"user32.dll\0".as_ptr() as _)
            ).expect("Failed to get user32.dll handle");
            let ptr = windows::Win32::System::LibraryLoader::GetProcAddress(
                h,
                windows::core::PCSTR(b"GetAsyncKeyState\0".as_ptr() as _)
            ).expect("Failed to find GetAsyncKeyState");
            std::mem::transmute(ptr)
        });
        func(v_key)
    }
}

/// Start the Win+V interception hook on a background thread.
pub fn start_win_v_intercept() {
    std::thread::spawn(|| unsafe {
        let h_module = GetModuleHandleW(None).expect("Failed to get module handle");
        let h_instance: windows::Win32::Foundation::HINSTANCE = std::mem::transmute(h_module);
        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(win_v_hook_proc),
            Some(h_instance),
            0,
        );
        match hook {
            Ok(h) => {
                WINV_HOOK = h;
                info!("Win+V interception hook installed");

                let mut msg = MSG::default();
                while !crate::state::global_state::SHUTDOWN.load(Ordering::Relaxed) && GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    let _ = DispatchMessageW(&msg);
                }

                let _ = UnhookWindowsHookEx(WINV_HOOK);
                info!("Win+V interception hook removed");
            }
            Err(e) => {
                error!("Failed to install Win+V hook: {}", e);
            }
        }
    });
}

/// Stop the Win+V interception hook.
pub fn stop_win_v_intercept() {
    unsafe {
        if !WINV_HOOK.0.is_null() {
            let _ = UnhookWindowsHookEx(WINV_HOOK);
            WINV_HOOK = HHOOK(std::ptr::null_mut());
        }
    }
}
