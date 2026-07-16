/// Low-level keyboard hook for navigation when the clipboard window is visible.
/// Intercepts arrow keys, Enter, Escape, Ctrl+Shift+0-9 quick paste, and sequential paste hotkey.
use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW,
    SetWindowsHookExW, TranslateMessage, DispatchMessageW, UnhookWindowsHookEx,
    HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

use crate::infrastructure::repository::clipboard_repo::ClipboardRepository;
use crate::state::global_state::*;
use crate::{error, info};

static mut KB_HOOK: HHOOK = HHOOK(std::ptr::null_mut());

const VK_UP: u32 = 0x26;
const VK_DOWN: u32 = 0x28;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_0: u32 = 0x30;
const VK_9: u32 = 0x39;
const VK_V: u32 = 0x56;

unsafe extern "system" fn keyboard_hook_proc(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT { unsafe {
    if code >= 0 {
        let is_key_down = w_param.0 == WM_KEYDOWN as usize || w_param.0 == WM_SYSKEYDOWN as usize;
        if is_key_down {
            let kb = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
            let vk = kb.vkCode;

            // Check if our window is visible
            let window_visible = !IS_HIDDEN.load(Ordering::Relaxed);
            let search_focused = IS_SEARCH_FOCUSED.load(Ordering::Relaxed);

            // Ctrl+Shift+0-9 quick paste (works even when window is hidden)
            if window_visible && !search_focused {
                if vk >= VK_0 && vk <= VK_9 {
                    let ctrl_held = get_async_key_state(0x11) < 0; // VK_CONTROL
                    let shift_held = get_async_key_state(0x10) < 0; // VK_SHIFT
                    if ctrl_held && shift_held {
                        let index = (vk - VK_0) as usize;
                        crate::services::paste_queue::quick_paste(index);
                        return LRESULT(1);
                    }
                }

                // Sequential paste hotkey (Alt+V by default)
                if vk == VK_V {
                    let alt_held = get_async_key_state(0x12) < 0; // VK_MENU
                    if alt_held {
                        let settings = if let Some(guard) = crate::APP_CTX.get() {
                            let ctx = guard.lock().unwrap();
                            ctx.settings.sequential_mode.load(Ordering::Relaxed)
                        } else {
                            false
                        };
                        if settings {
                            crate::services::paste_queue::paste_next_in_queue();
                            return LRESULT(1);
                        }
                    }
                }

                // Arrow key navigation
                match vk {
                    VK_UP => {
                        let current = SELECTED_INDEX.load(Ordering::Relaxed);
                        let new_idx = (current - 1).max(0);
                        SELECTED_INDEX.store(new_idx, Ordering::Relaxed);
                        return LRESULT(1); // Consume the key
                    }
                    VK_DOWN => {
                        let current = SELECTED_INDEX.load(Ordering::Relaxed);
                        let count = LIST_ITEM_COUNT.load(Ordering::Relaxed);
                        let new_idx = if count > 0 { (current + 1).min(count - 1) } else { 0 };
                        SELECTED_INDEX.store(new_idx, Ordering::Relaxed);
                        return LRESULT(1); // Consume the key
                    }
                    VK_RETURN => {
                        // Trigger paste of selected item
                        let idx = SELECTED_INDEX.load(Ordering::Relaxed);
                        paste_selected_item(idx);
                        return LRESULT(1);
                    }
                    VK_ESCAPE => {
                        // Hide window
                        if let Some(&hwnd) = MAIN_WINDOW_HANDLE.get() {
                            if hwnd != 0 {
                                crate::app::window_manager::hide_window(hwnd);
                            }
                        }
                        return LRESULT(1);
                    }
                    _ => {}
                }
            }
        }
    }
    CallNextHookEx(Some(KB_HOOK), code, w_param, l_param)
}}

/// Get async key state via user32.dll
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

/// Paste the clipboard entry at the given index (runs in background thread).
fn paste_selected_item(index: i32) {
    std::thread::spawn(move || {
        let entries = crate::CLIPBOARD_ENTRIES.lock().unwrap();
        let idx = index as usize;
        if idx >= entries.len() {
            return;
        }
        let entry_id = entries[idx].id;
        drop(entries);

        if let Some(db_conn) = crate::state::global_state::DB_CONN.get() {
            let repo = crate::infrastructure::repository::clipboard_repo::SqliteClipboardRepository::new(
                db_conn.clone(),
            );
            if let Ok(Some(entry)) = repo.get_entry_by_id(entry_id) {
                let _ = crate::services::clipboard_ops::paste_entry(&entry);
            }
        }
    });
}

/// Start the keyboard navigation hook on a background thread.
pub fn start_keyboard_hook() {
    std::thread::spawn(|| unsafe {
        let h_module = GetModuleHandleW(None).expect("Failed to get module handle");
        // HMODULE and HINSTANCE are the same underlying type in Win32
        let h_instance = std::mem::transmute(h_module);
        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_hook_proc),
            Some(h_instance),
            0,
        );
        match hook {
            Ok(h) => {
                KB_HOOK = h;
                info!("Keyboard navigation hook installed");

                let mut msg = MSG::default();
                while !SHUTDOWN.load(Ordering::Relaxed) && GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    let _ = DispatchMessageW(&msg);
                }

                let _ = UnhookWindowsHookEx(KB_HOOK);
                info!("Keyboard navigation hook removed");
            }
            Err(e) => {
                error!("Failed to install keyboard hook: {}", e);
            }
        }
    });
}

/// Stop the keyboard navigation hook.
pub fn stop_keyboard_hook() {
    unsafe {
        if !KB_HOOK.0.is_null() {
            let _ = UnhookWindowsHookEx(KB_HOOK);
            KB_HOOK = HHOOK(std::ptr::null_mut());
        }
    }
}
