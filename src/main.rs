//! Clipboard - WinUI3 Clipboard Manager
//!
//! A modern clipboard history manager built with windows-reactor (WinUI3).
//! Records clipboard history with fast search, tags, and paste operations.
//! WinPaste-inspired architecture ported from Tauri to native WinUI3.

// Disable unused warning noise during skeleton development
#![allow(dead_code)]

mod app;
mod database;
mod domain;
mod error;
mod infrastructure;
mod logger;
mod services;
mod state;
mod ui;

use crate::app::setup::AppContext;
use crate::domain::ClipboardEntryView;
use std::sync::{LazyLock, Mutex, OnceLock};

// ---------------------------------------------------------------------------
// Globals – shared across UI render, monitor pipeline, and service layer
// ---------------------------------------------------------------------------

/// Initialized once by `main()`; read by the UI thread and background
/// monitor thread.  Wrapped in `Mutex` because `mpsc::Receiver` is `!Sync`.
pub static APP_CTX: OnceLock<Mutex<AppContext>> = OnceLock::new();

/// Lightweight cached clipboard entries for the UI (no full `content`).
/// The background monitor thread prepends new entries here; the UI render
/// reads them. Full content is fetched from DB on demand.
pub static CLIPBOARD_ENTRIES: LazyLock<Mutex<Vec<ClipboardEntryView>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

// ---------------------------------------------------------------------------
// App entry point
// ---------------------------------------------------------------------------

/// Application render function - called by windows-reactor on each render pass.
fn app_ui(cx: &mut windows_reactor::RenderCx) -> windows_reactor::Element {
    ui::main_window::render(cx)
}

fn main() {
    // Set panic hook to log to file
    std::panic::set_hook(Box::new(|info| {
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("unknown");
        let payload = info.payload().downcast_ref::<&str>().unwrap_or(&"unknown");
        let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_else(|| "unknown".to_string());
        let msg = format!("[PANIC] thread '{}' at {}: {}", thread_name, location, payload);
        eprintln!("{}", msg);
        let _ = std::fs::write("C:\\Users\\11315\\AppData\\Roaming\\com.clipboard.app\\panic.log", &msg);
    }));

    // 0. Set DPI awareness for proper scaling on high-DPI displays
    // This ensures our window renders at the correct size on 125%, 150%, 200% displays.
    set_dpi_awareness();

    // 1. Bootstrap the Windows App SDK runtime
    if let Err(e) = windows_reactor::bootstrap() {
        eprintln!("Failed to bootstrap WinUI3: {}", e);
        return;
    }

    // 2. Initialize application services (data dir, logger, database)
    let ctx = app::setup::init();
    let _ = APP_CTX.set(Mutex::new(ctx));

    // 3. Start the global hotkey listener (Ctrl+Shift+V)
    services::hotkey::start_global_hotkey();

    // 3.1 Start keyboard navigation hook (arrow keys, Enter, Escape, quick paste)
    services::keyboard_nav::start_keyboard_hook();

    // 3.2 Start Win+V interception hook (if enabled)
    {
        let ctx = APP_CTX.get().unwrap().lock().unwrap();
        if ctx.settings.use_win_v_shortcut.load(std::sync::atomic::Ordering::Relaxed) {
            services::win_v_intercept::start_win_v_intercept();
        }
    }

    // 3.2 Load existing sticky notes from DB
    app::sticky_manager::load_all_stickies();

    // 4. Start the system tray icon
    app::tray::start_tray();

    // 5. Launch the WinUI3 application window
    if let Err(e) = windows_reactor::App::new()
        .title("Clipboard")
        .render(app_ui)
    {
        eprintln!("App exited with error: {}", e);
    }

    eprintln!("WinUI3 event loop exited, starting cleanup...");

    // 6. Cleanup — force exit immediately
    std::process::abort();
}

/// Set process DPI awareness for proper rendering on high-DPI displays.
/// Uses SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
/// which is the recommended approach for Windows 10 1703+.
fn set_dpi_awareness() {
    unsafe {
        // Try the V2 per-monitor DPI awareness first (Windows 10 1703+)
        // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 = -4
        type SetDpiFunc = unsafe extern "system" fn(isize) -> i32;
        type SetDpiAwareFunc = unsafe extern "system" fn() -> i32;

        let h_user32 = windows::Win32::System::LibraryLoader::GetModuleHandleW(
            windows::core::PCWSTR(b"user32.dll\0".as_ptr() as _)
        );
        if let Ok(h) = h_user32 {
            // Try SetProcessDpiAwarenessContext first
            if let Some(func_ptr) = windows::Win32::System::LibraryLoader::GetProcAddress(
                h,
                windows::core::PCSTR(b"SetProcessDpiAwarenessContext\0".as_ptr() as _)
            ) {
                let func: SetDpiFunc = std::mem::transmute(func_ptr);
                if func(-4) != 0 {
                    eprintln!("DPI awareness set to Per-Monitor V2");
                    return;
                }
            }
            // Fallback: SetProcessDPIAware
            if let Some(func_ptr) = windows::Win32::System::LibraryLoader::GetProcAddress(
                h,
                windows::core::PCSTR(b"SetProcessDPIAware\0".as_ptr() as _)
            ) {
                let func: SetDpiAwareFunc = std::mem::transmute(func_ptr);
                func();
                eprintln!("DPI awareness set to System Aware (fallback)");
            }
        }
    }
}
