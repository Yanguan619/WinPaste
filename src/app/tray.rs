/// System tray icon with context menu using raw Win32 APIs.
use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetMessageW, TranslateMessage, DispatchMessageW,
    RegisterClassW, RegisterWindowMessageW, PostQuitMessage,
    WM_COMMAND, WM_USER, WNDCLASSW, WINDOW_EX_STYLE, WINDOW_STYLE,
    CreatePopupMenu, AppendMenuW, TrackPopupMenu, SetForegroundWindow,
    DestroyMenu, GetCursorPos, LoadIconW, MSG,
    TPM_RIGHTBUTTON, TPM_NONOTIFY, TPM_RETURNCMD, MENU_ITEM_FLAGS,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION,
    NOTIFYICONDATAW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::core::w;

use crate::state::global_state::*;
use crate::{error, info};

const WM_TRAYICON: u32 = WM_USER + 1;
const CMD_SHOW: usize = 1001;
const CMD_SETTINGS: usize = 1002;
const CMD_QUIT: usize = 1003;
const TRAY_WINDOW_CLASS: &str = "ClipboardTrayWindow";
static TRAY_HWND: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT { unsafe {
    match msg {
        WM_TRAYICON => {
            let event = (l_param.0 >> 16) as u16;
            match event {
                0x0205 => show_context_menu(hwnd), // WM_RBUTTONUP
                0x0201 => crate::app::window_manager::toggle_window(), // WM_LBUTTONUP
                _ => {}
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            match w_param.0 & 0xFFFF {
                CMD_SHOW => crate::app::window_manager::toggle_window(),
                CMD_SETTINGS => {
                    // Show window and trigger settings view
                    crate::app::window_manager::toggle_window();
                }
                CMD_QUIT => {
                    info!("Quit requested from tray menu");
                    remove_tray_icon(hwnd);
                    // Force exit immediately — don't wait for WinUI3 element destruction
                    std::process::abort();
                }
                _ => {}
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, w_param, l_param),
    }
}}

unsafe fn show_context_menu(hwnd: HWND) { unsafe {
    if let Ok(hmenu) = CreatePopupMenu() {
        let _ = AppendMenuW(hmenu, MENU_ITEM_FLAGS(0x0000), CMD_SHOW, w!("显示主界面"));
        let _ = AppendMenuW(hmenu, MENU_ITEM_FLAGS(0x0000), CMD_SETTINGS, w!("设置"));
        let _ = AppendMenuW(hmenu, MENU_ITEM_FLAGS(0x0000), 0, None);
        let _ = AppendMenuW(hmenu, MENU_ITEM_FLAGS(0x0000), CMD_QUIT, w!("退出 Clipboard"));

        let mut pt = windows::Win32::Foundation::POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(hmenu, TPM_RIGHTBUTTON | TPM_NONOTIFY | TPM_RETURNCMD, pt.x, pt.y, Some(0), hwnd, None);
        let _ = DestroyMenu(hmenu);
    }
}}

unsafe fn add_tray_icon(hwnd: HWND) { unsafe {
    let h_icon = LoadIconW(None, w!("IDI_APPLICATION"))
        .unwrap_or_else(|_| windows::Win32::UI::WindowsAndMessaging::HICON(std::ptr::null_mut()));

    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        hIcon: h_icon,
        szTip: [0; 128],
        ..Default::default()
    };

    let tip = "Clipboard — 剪贴板管理";
    let tip_wide: Vec<u16> = tip.encode_utf16().chain(std::iter::once(0)).collect();
    let len = tip_wide.len().min(127);
    nid.szTip[..len].copy_from_slice(&tip_wide[..len]);

    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    let _ = Shell_NotifyIconW(NIM_SETVERSION, &nid);
}}

unsafe fn remove_tray_icon(hwnd: HWND) { unsafe {
    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    };
    let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
}}

pub fn start_tray() {
    std::thread::spawn(|| unsafe {
        let h_instance = GetModuleHandleW(None).expect("Failed to get module handle");

        let class_name: Vec<u16> = TRAY_WINDOW_CLASS.encode_utf16().chain(std::iter::once(0)).collect();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(tray_wnd_proc),
            hInstance: h_instance.into(),
            lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class_name.as_ptr()),
            w!("Clipboard Tray"),
            WINDOW_STYLE(0),
            0, 0, 0, 0,
            None, None,
            Some(h_instance.into()),
            None,
        );

        if let Ok(h) = hwnd {
            let hwnd_val = h.0 as usize;
            let _ = TRAY_HWND.set(hwnd_val);

            let taskbar_msg = RegisterWindowMessageW(w!("TaskbarCreated"));
            if taskbar_msg != 0 {
                TASKBAR_CREATED_MSG.store(taskbar_msg, Ordering::Relaxed);
                info!("Registered TaskbarCreated message: {}", taskbar_msg);
            }

            add_tray_icon(h);
            info!("Tray icon created");

            let mut msg = MSG::default();
            while !crate::state::global_state::SHUTDOWN.load(Ordering::Relaxed) && GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                let _ = DispatchMessageW(&msg);
            }

            remove_tray_icon(h);
            info!("Tray icon removed");
        } else {
            error!("Failed to create tray window");
        }
    });
}

pub fn stop_tray() {
    if let Some(&hwnd) = TRAY_HWND.get() {
        unsafe {
            remove_tray_icon(HWND(hwnd as *mut _));
            PostQuitMessage(0);
        }
    }
}
