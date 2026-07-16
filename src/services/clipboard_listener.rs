use std::sync::Arc;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, RemoveClipboardFormatListener,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW,
    RegisterClassW, SetWindowLongPtrW, GWLP_USERDATA, HWND_MESSAGE, MSG, WM_CLIPBOARDUPDATE,
    WNDCLASSW,
};

pub fn listen_clipboard(callback: Arc<dyn Fn() + Send + Sync + 'static>) {
    std::thread::spawn(move || {
        unsafe {
            let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();
            let window_class = "winpasteClipboardListener";
            let window_class_w: Vec<u16> = window_class
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let wnd_class = WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                hInstance: instance.into(),
                lpszClassName: PCWSTR(window_class_w.as_ptr()),
                ..Default::default()
            };

            RegisterClassW(&wnd_class);

            let hwnd = match CreateWindowExW(
                Default::default(),
                PCWSTR(window_class_w.as_ptr()),
                PCWSTR(std::ptr::null()),
                Default::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(HINSTANCE(instance.0)),
                None,
            ) {
                Ok(hwnd) => hwnd,
                Err(e) => {
                    crate::error!("Failed to create clipboard listener window: {:?}", e);
                    return;
                }
            };

            // Wrap callback in a Box to store in window user data
            let boxed_callback = Box::new(callback);
            let ptr = Box::into_raw(boxed_callback);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);

            if let Err(e) = AddClipboardFormatListener(hwnd) {
                crate::error!("Failed to add clipboard listener: {:?}", e);
                let _ = Box::from_raw(ptr);
                return;
            }

            crate::info!(">>> [CLIPBOARD] Windows event-driven listener started.");

            let mut msg = MSG::default();
            while !crate::state::global_state::SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed)
                && GetMessageW(&mut msg, None, 0, 0).as_bool()
            {
                DispatchMessageW(&msg);
            }

            let _ = RemoveClipboardFormatListener(hwnd);
            // Cleanup callback
            let _ = Box::from_raw(ptr);
        }
    });
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT { unsafe {
    match msg {
        WM_CLIPBOARDUPDATE => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                let callback = &*(ptr as *const Arc<dyn Fn() + Send + Sync + 'static>);
                callback();
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}}
