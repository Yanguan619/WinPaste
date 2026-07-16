/// Sticky note window manager using Win32 APIs.
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, ShowWindow, SetWindowPos,
    RegisterClassW, WNDCLASSW, WINDOW_EX_STYLE, WINDOW_STYLE,
    SW_SHOWNA, HWND_TOPMOST, HWND_NOTOPMOST,
    SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SWP_NOACTIVATE, SWP_FRAMECHANGED,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use crate::infrastructure::repository::sticky_repo::{StickyNote, StickyRepository, SqliteStickyRepository};
use crate::{error, info};

const STICKY_WINDOW_CLASS: &str = "ClipboardStickyNote";
static STICKY_WINDOWS: OnceLock<Mutex<HashMap<usize, i64>>> = OnceLock::new();

fn sticky_windows() -> &'static Mutex<HashMap<usize, i64>> {
    STICKY_WINDOWS.get_or_init(|| Mutex::new(HashMap::new()))
}

unsafe extern "system" fn sticky_wnd_proc(
    hwnd: HWND,
    msg: u32,
    w_param: windows::Win32::Foundation::WPARAM,
    l_param: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT { unsafe {
    match msg {
        0x0002 => {
            // WM_DESTROY - clean up
            let hwnd_val = hwnd.0 as usize;
            sticky_windows().lock().unwrap().remove(&hwnd_val);
            windows::Win32::Foundation::LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, w_param, l_param),
    }
}}

pub fn create_sticky_window(note: &StickyNote) -> Option<usize> {
    unsafe {
        let h_instance = GetModuleHandleW(None).expect("Failed to get module handle");

        let class_name: Vec<u16> = STICKY_WINDOW_CLASS.encode_utf16().chain(std::iter::once(0)).collect();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(sticky_wnd_proc),
            hInstance: h_instance.into(),
            lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);

        let title: Vec<u16> = "Sticky".encode_utf16().chain(std::iter::once(0)).collect();

        let mut ex_style = WS_EX_TOOLWINDOW.0 as u32;
        if note.always_on_top {
            ex_style |= WS_EX_TOPMOST.0 as u32;
        }

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(ex_style),
            windows::core::PCWSTR(class_name.as_ptr()),
            windows::core::PCWSTR(title.as_ptr()),
            WINDOW_STYLE(0x00CF0000),
            note.x, note.y, note.width, note.height,
            None, None,
            Some(h_instance.into()),
            None,
        );

        if let Ok(h) = hwnd {
            let hwnd_val = h.0 as usize;
            sticky_windows().lock().unwrap().insert(hwnd_val, note.id);

            let _ = ShowWindow(h, SW_SHOWNA);
            let _ = SetWindowPos(
                h,
                if note.always_on_top { Some(HWND_TOPMOST) } else { Some(HWND_NOTOPMOST) },
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );

            info!("Sticky note {} created", note.id);
            Some(hwnd_val)
        } else {
            error!("Failed to create sticky window");
            None
        }
    }
}

pub fn create_sticky_from_clipboard(content: &str) -> Option<usize> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let note = StickyNote {
        id: 0,
        content: content.to_string(),
        content_type: "text".to_string(),
        x: 100,
        y: 100,
        width: 250,
        height: 200,
        always_on_top: false,
        created_at: now,
    };

    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = SqliteStickyRepository::new(ctx.db_conn.clone());
        match repo.create(&note) {
            Ok(id) => {
                let mut note = note;
                note.id = id;
                create_sticky_window(&note)
            }
            Err(e) => {
                error!("Failed to create sticky note: {}", e);
                None
            }
        }
    } else {
        None
    }
}

pub fn load_all_stickies() {
    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = SqliteStickyRepository::new(ctx.db_conn.clone());
        if let Ok(notes) = repo.get_all() {
            info!("Loading {} sticky notes", notes.len());
            for note in notes {
                create_sticky_window(&note);
            }
        }
    }
}
