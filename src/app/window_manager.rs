/// Window show/hide/toggle logic with WS_EX_NOACTIVATE for no-focus display.
use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE, MAPVK_VK_TO_VSC, MapVirtualKeyW, SendInput, VIRTUAL_KEY, VK_CONTROL,
    VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GWL_EXSTYLE, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW,
    GetWindowRect, GetWindowThreadProcessId, HWND_TOPMOST, IsWindowVisible, SW_HIDE, SW_SHOWNA,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_EX_NOACTIVATE,
};

use crate::state::global_state::*;

pub fn toggle_window() {
    let hwnd = match MAIN_WINDOW_HANDLE.get() {
        Some(&h) if h != 0 => h,
        _ => return,
    };

    let now = timestamp_ms();
    let last = LAST_TOGGLE_TIMESTAMP.swap(now, Ordering::Relaxed);
    if now.saturating_sub(last) < 200 {
        return;
    }

    if IS_HIDDEN.load(Ordering::Relaxed) || !is_window_visible(hwnd) {
        show_window(hwnd);
    } else {
        hide_window(hwnd);
    }
}

fn show_window(hwnd: usize) {
    unsafe {
        let h = HWND(hwnd as *mut _);

        let fg = GetForegroundWindow();
        if !fg.0.is_null() && fg.0 as usize != hwnd {
            LAST_ACTIVE_HWND.store(fg.0 as usize, Ordering::Relaxed);
        }

        let ex_style = GetWindowLongPtrW(h, GWL_EXSTYLE);
        let _ = SetWindowLongPtrW(h, GWL_EXSTYLE, ex_style | WS_EX_NOACTIVATE.0 as isize);

        position_near_cursor(h);

        let _ = ShowWindow(h, SW_SHOWNA);
        let _ = SetWindowPos(
            h,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );

        IS_HIDDEN.store(false, Ordering::Relaxed);
        LAST_SHOW_TIMESTAMP.store(timestamp_ms(), Ordering::Relaxed);
    }
}

pub fn hide_window(hwnd: usize) {
    unsafe {
        let h = HWND(hwnd as *mut _);

        // Save window size before hiding
        let mut rect = RECT::default();
        if GetWindowRect(h, &mut rect).is_ok() {
            let w = rect.right - rect.left;
            let h_px = rect.bottom - rect.top;
            if w > 0 && h_px > 0 {
                save_window_size(w, h_px);
            }
        }

        release_win_keys();
        let _ = ShowWindow(h, SW_HIDE);

        let ex_style = GetWindowLongPtrW(h, GWL_EXSTYLE);
        let _ = SetWindowLongPtrW(h, GWL_EXSTYLE, ex_style & !(WS_EX_NOACTIVATE.0 as isize));

        IS_HIDDEN.store(true, Ordering::Relaxed);
        NAVIGATION_ENABLED.store(false, Ordering::SeqCst);
        NAVIGATION_MODE_ACTIVE.store(false, Ordering::SeqCst);
    }
    restore_last_focus();
}

pub fn activate_window_focus() {
    let hwnd = match MAIN_WINDOW_HANDLE.get() {
        Some(&h) if h != 0 => h,
        _ => return,
    };

    unsafe {
        let h = HWND(hwnd as *mut _);

        let fg = GetForegroundWindow();
        if !fg.0.is_null() && fg.0 as usize != hwnd {
            LAST_ACTIVE_HWND.store(fg.0 as usize, Ordering::Relaxed);
        }

        let ex_style = GetWindowLongPtrW(h, GWL_EXSTYLE);
        let next = ex_style & !(WS_EX_NOACTIVATE.0 as isize);
        let _ = SetWindowLongPtrW(h, GWL_EXSTYLE, next);
        let _ = SetWindowPos(
            h,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );

        force_focus_window(hwnd);
        IS_MAIN_WINDOW_FOCUSED.store(true, Ordering::Relaxed);
        IS_SEARCH_FOCUSED.store(true, Ordering::Relaxed);
    }
}

pub fn restore_last_focus() {
    let target = LAST_ACTIVE_HWND.load(Ordering::Relaxed);
    if target == 0 {
        return;
    }
    force_focus_window(target);
}

fn force_focus_window(hwnd: usize) {
    unsafe {
        let target = HWND(hwnd as *mut _);
        let fg = GetForegroundWindow();
        if fg.0.is_null() || fg.0 as usize == hwnd {
            return;
        }

        let target_thread = GetWindowThreadProcessId(target, None);
        let current_thread = GetCurrentThreadId();

        if target_thread != current_thread {
            let _ = AttachThreadInput(current_thread, target_thread, true);
            let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(target);
            let _ = BringWindowToTop(target);
            let _ = AttachThreadInput(current_thread, target_thread, false);
        } else {
            let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(target);
            let _ = BringWindowToTop(target);
        }
    }
}

fn release_win_keys() {
    let release_modifiers: Vec<INPUT> = vec![
        create_key_input(VK_LWIN, true),
        create_key_input(VK_RWIN, true),
        create_key_input(VK_MENU, true),
        create_key_input(VK_SHIFT, true),
        create_key_input(VK_CONTROL, true),
    ];
    unsafe {
        SendInput(&release_modifiers, std::mem::size_of::<INPUT>() as i32);
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
}

fn create_key_input(vk: VIRTUAL_KEY, is_up: bool) -> INPUT {
    unsafe {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: if is_up { vk } else { VIRTUAL_KEY(0) },
                    wScan: MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) as u16,
                    dwFlags: if is_up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYEVENTF_SCANCODE
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }
}

fn position_near_cursor(hwnd: HWND) {
    unsafe {
        // Check if edge docking is active
        if EDGE_DOCKING_ENABLED.load(Ordering::Relaxed) {
            position_docked(hwnd);
            return;
        }

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);

        let (screen_w, screen_h) = get_screen_size();

        // Use compact dimensions if in compact mode, or saved dimensions
        let (window_w, window_h) = if IS_COMPACT.load(Ordering::Relaxed) {
            (360i32, 220i32)
        } else {
            load_window_size()
        };

        let mut x = pt.x + 16;
        let mut y = pt.y - 40;

        if x + window_w > screen_w {
            x = pt.x - window_w - 16;
        }
        if y + window_h > screen_h {
            y = screen_h - window_h;
        }
        if y < 0 {
            y = 0;
        }
        if x < 0 {
            x = 0;
        }

        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            window_w,
            window_h,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}

/// Position the window docked to a screen edge.
fn position_docked(hwnd: HWND) {
    unsafe {
        let (screen_w, screen_h) = get_screen_size();
        let dock_edge = DOCK_EDGE.load(Ordering::Relaxed);

        let (window_w, window_h) = if IS_COMPACT.load(Ordering::Relaxed) {
            (360i32, 220i32)
        } else {
            load_window_size()
        };

        let (x, y) = match dock_edge {
            1 => (0, 0),                                     // Left
            2 => (screen_w - window_w, 0),                   // Right
            3 => ((screen_w - window_w) / 2, 0),             // Top center
            _ => (screen_w - window_w, screen_h - window_h), // Bottom-right (default)
        };

        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            window_w,
            window_h,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}

/// Get screen size (primary monitor).
fn get_screen_size() -> (i32, i32) {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN))
    }
}

/// Toggle pin (always on top) for the window.
pub fn toggle_pin() {
    let hwnd = match MAIN_WINDOW_HANDLE.get() {
        Some(&h) if h != 0 => h,
        _ => return,
    };

    let pinned = WINDOW_PINNED.load(Ordering::Relaxed);
    let new_pinned = !pinned;
    WINDOW_PINNED.store(new_pinned, Ordering::Relaxed);

    unsafe {
        let h = HWND(hwnd as *mut _);
        if new_pinned {
            // Set WS_EX_TOPMOST
            let _ = SetWindowPos(
                h,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        } else {
            // Remove topmost
            use windows::Win32::UI::WindowsAndMessaging::HWND_NOTOPMOST;
            let _ = SetWindowPos(
                h,
                Some(HWND_NOTOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

/// Toggle compact mode.
pub fn toggle_compact() {
    let current = IS_COMPACT.load(Ordering::Relaxed);
    IS_COMPACT.store(!current, Ordering::Relaxed);
}

/// Set the dock edge (0=none, 1=left, 2=right, 3=top, 4=bottom).
pub fn set_dock_edge(edge: i32) {
    DOCK_EDGE.store(edge, Ordering::Relaxed);
    EDGE_DOCKING_ENABLED.store(edge != 0, Ordering::Relaxed);
}

fn is_window_visible(hwnd: usize) -> bool {
    unsafe {
        let h = HWND(hwnd as *mut _);
        IsWindowVisible(h).as_bool()
    }
}

fn timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn send_paste_keystroke() {
    unsafe {
        let v_scan = MapVirtualKeyW(0x56, MAPVK_VK_TO_VSC) as u16;
        let ctrl_scan = MapVirtualKeyW(VK_CONTROL.0 as u32, MAPVK_VK_TO_VSC) as u16;

        let inputs: Vec<INPUT> = vec![
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: ctrl_scan,
                        dwFlags: KEYEVENTF_SCANCODE,
                        time: 0,
                        dwExtraInfo: 0x5749_4E50,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: v_scan,
                        dwFlags: KEYEVENTF_SCANCODE,
                        time: 0,
                        dwExtraInfo: 0x5749_4E50,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: v_scan,
                        dwFlags: KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0x5749_4E50,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: ctrl_scan,
                        dwFlags: KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0x5749_4E50,
                    },
                },
            },
        ];

        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Send Shift+Insert keystroke for paste.
pub fn send_shift_insert() {
    unsafe {
        let insert_scan = MapVirtualKeyW(0x2D, MAPVK_VK_TO_VSC) as u16; // VK_INSERT
        let shift_scan = MapVirtualKeyW(VK_SHIFT.0 as u32, MAPVK_VK_TO_VSC) as u16;

        let inputs: Vec<INPUT> = vec![
            // Shift down
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: shift_scan,
                        dwFlags: KEYEVENTF_SCANCODE,
                        time: 0,
                        dwExtraInfo: 0x5749_4E50,
                    },
                },
            },
            // Insert down
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: insert_scan,
                        dwFlags: KEYEVENTF_SCANCODE,
                        time: 0,
                        dwExtraInfo: 0x5749_4E50,
                    },
                },
            },
            // Insert up
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: insert_scan,
                        dwFlags: KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0x5749_4E50,
                    },
                },
            },
            // Shift up
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: shift_scan,
                        dwFlags: KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0x5749_4E50,
                    },
                },
            },
        ];

        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

// ── Window size persistence ──────────────────────────────────────────────

fn save_window_size(width: i32, height: i32) {
    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = crate::infrastructure::repository::settings_repo::SqliteSettingsRepository::new(
            ctx.db_conn.clone(),
        );
        use crate::infrastructure::repository::settings_repo::SettingsRepository;
        let _ = repo.set("app.window_width", &width.to_string());
        let _ = repo.set("app.window_height", &height.to_string());
    }
}

pub fn load_window_size() -> (i32, i32) {
    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = crate::infrastructure::repository::settings_repo::SqliteSettingsRepository::new(
            ctx.db_conn.clone(),
        );
        use crate::infrastructure::repository::settings_repo::SettingsRepository;
        let w = repo
            .get("app.window_width")
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(380);
        let h = repo
            .get("app.window_height")
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(600);
        return (w, h);
    }
    (380, 600)
}
