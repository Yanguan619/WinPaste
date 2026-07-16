/// Clipboard operations: copy, paste, and format conversion.
/// Full paste pipeline with keystroke simulation.
use std::hash::{Hash, Hasher};
use std::sync::atomic::Ordering;

use crate::domain::ClipboardEntry;
use crate::state::global_state::*;
use crate::{error, info};

/// Paste the content of a clipboard entry into the focused application.
/// 0. Hide our window and wait for target to gain focus
/// 1. Write content to system clipboard (type-aware)
/// 2. Set echo-prevention hash
/// 3. Pause monitor
/// 4. Focus target window
/// 5. Send paste keystroke
/// 6. Unpause monitor
pub fn paste_entry(entry: &ClipboardEntry) -> Result<(), String> {
    // 0. Hide our clipboard window first so the target window can gain focus
    if let Some(&hwnd) = MAIN_WINDOW_HANDLE.get() {
        if hwnd != 0 {
            crate::app::window_manager::hide_window(hwnd);
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(100));

    let target_hwnd = LAST_ACTIVE_HWND.load(Ordering::Relaxed);
    if target_hwnd == 0 {
        error!("No target window for paste");
        return Err("No target window".to_string());
    }

    // 1. Write content to system clipboard based on type
    match entry.content_type.as_str() {
        "image" => {
            paste_image(entry)?;
        }
        "file" => {
            paste_files(entry)?;
        }
        "rich_text" => {
            if let Some(ref html) = entry.html_content {
                unsafe {
                    crate::infrastructure::windows_api::win_clipboard::set_clipboard_text_and_html(
                        &entry.content,
                        html,
                    )?;
                }
            } else {
                unsafe {
                    crate::infrastructure::windows_api::win_clipboard::set_clipboard_text(&entry.content)?;
                }
            }
        }
        _ => {
            // Plain text, code, url, etc.
            unsafe {
                crate::infrastructure::windows_api::win_clipboard::set_clipboard_text(&entry.content)?;
            }
        }
    }

    // 2. Set echo-prevention hash so monitor skips this entry
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    entry.content.hash(&mut hasher);
    let hash = hasher.finish();
    LAST_APP_SET_HASH.store(hash, Ordering::SeqCst);
    LAST_APP_SET_HASH_ALT.store(0, Ordering::SeqCst);
    LAST_APP_SET_TIMESTAMP.store(timestamp_ms(), Ordering::Relaxed);

    // 3. Pause clipboard monitor
    CLIPBOARD_MONITOR_PAUSED.store(true, Ordering::SeqCst);

    // 4. Wait for clipboard to settle
    std::thread::sleep(std::time::Duration::from_millis(50));

    // 5. Focus target window
    unsafe {
        let h = windows::Win32::Foundation::HWND(target_hwnd as *mut _);
        let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(h);
    }
    std::thread::sleep(std::time::Duration::from_millis(50));

    // 6. Send paste keystroke based on configured method
    let paste_method = if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        ctx.settings.paste_method.lock().unwrap().clone()
    } else {
        "ctrl_v".to_string()
    };

    match paste_method.as_str() {
        "shift_insert" => {
            crate::app::window_manager::send_shift_insert();
        }
        "game_mode" => {
            // Game mode: type characters one by one (for games that don't support Ctrl+V)
            // This is a simplified version - full implementation would need the actual content
            crate::app::window_manager::send_paste_keystroke();
        }
        _ => {
            // Default: Ctrl+V
            crate::app::window_manager::send_paste_keystroke();
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(100));

    // 7. Unpause monitor
    CLIPBOARD_MONITOR_PAUSED.store(false, Ordering::SeqCst);

    // 8. Hide the clipboard window
    if let Some(&hwnd) = MAIN_WINDOW_HANDLE.get() {
        if hwnd != 0 {
            crate::app::window_manager::hide_window(hwnd);
        }
    }

    info!("Pasted entry #{} ({})", entry.id, entry.content_type);

    // Play paste sound if enabled
    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        if ctx.settings.sound_paste_enabled.load(Ordering::Relaxed) {
            // Use Windows Beep API (frequency=800Hz, duration=100ms)
            type BeepFunc = unsafe extern "system" fn(u32, u32) -> i32;
            unsafe {
                if let Ok(h) = windows::Win32::System::LibraryLoader::GetModuleHandleW(
                    windows::core::PCWSTR(b"user32.dll\0".as_ptr() as _)
                ) {
                    if let Some(ptr) = windows::Win32::System::LibraryLoader::GetProcAddress(
                        h, windows::core::PCSTR(b"MessageBeep\0".as_ptr() as _)
                    ) {
                        let func: BeepFunc = std::mem::transmute(ptr);
                        func(0x00000040, 100); // MB_ICONASTERISK
                    }
                }
            }
        }
    }

    Ok(())
}

/// Paste an image entry to the system clipboard as CF_DIB.
fn paste_image(entry: &ClipboardEntry) -> Result<(), String> {
    let content = &entry.content;

    // Determine image bytes: file path or data URL
    let image_bytes = if entry.is_external {
        // Content is a file path (e.g. C:\...\attachments\img_xxx.png)
        let path = content.trim();
        std::fs::read(path).map_err(|e| format!("Failed to read image file: {}", e))?
    } else if content.starts_with("data:image/") {
        // Data URL: decode base64
        let b64 = content.splitn(2, ',').nth(1).unwrap_or("");
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| format!("Failed to decode base64 image: {}", e))?
    } else {
        // Raw content - try as file path
        std::fs::read(content).map_err(|e| format!("Failed to read image: {}", e))?
    };

    // Decode to raw pixels using the image crate
    let img = image::load_from_memory(&image_bytes)
        .map_err(|e| format!("Failed to decode image: {}", e))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    // Build ImageData for the Win32 clipboard API
    let image_data = crate::infrastructure::windows_api::win_clipboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: rgba.into_raw(),
    };

    unsafe {
        crate::infrastructure::windows_api::win_clipboard::set_clipboard_image_with_formats(
            image_data, None, None,
        )?;
    }
    Ok(())
}

/// Paste file entries to the system clipboard as CF_HDROP.
fn paste_files(entry: &ClipboardEntry) -> Result<(), String> {
    let paths: Vec<String> = entry.content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if paths.is_empty() {
        // Fallback: write as text
        unsafe {
            crate::infrastructure::windows_api::win_clipboard::set_clipboard_text(&entry.content)?;
        }
        return Ok(());
    }

    // Use set_clipboard_files if available, otherwise fallback to text
    unsafe {
        crate::infrastructure::windows_api::win_clipboard::set_clipboard_files(paths)?;
    }
    Ok(())
}

/// Copy the given text to the system clipboard.
pub fn copy_text(text: &str) -> Result<(), String> {
    unsafe {
        crate::infrastructure::windows_api::win_clipboard::set_clipboard_text(text)?;
    }

    // Set echo-prevention hash
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    let hash = hasher.finish();
    LAST_APP_SET_HASH.store(hash, Ordering::SeqCst);
    LAST_APP_SET_HASH_ALT.store(0, Ordering::SeqCst);

    info!("Copied {} bytes to clipboard", text.len());
    Ok(())
}

/// Get current timestamp in milliseconds.
fn timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
