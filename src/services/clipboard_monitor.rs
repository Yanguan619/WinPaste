use std::hash::{Hash, Hasher};
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc, Mutex};

use base64::Engine;
use rusqlite::Connection;

use crate::domain::ClipboardEntry;
use crate::infrastructure::repository::clipboard_repo::{
    ClipboardRepository, SqliteClipboardRepository,
};
use crate::infrastructure::windows_api::window_tracker::get_clipboard_source_app_info;
use crate::state::global_state::*;

pub enum MonitorEvent {
    ClipboardUpdated(ClipboardEntry),
}

/// Start the clipboard monitoring pipeline.
///
/// 1. Creates a `SqliteClipboardRepository` over the given DB connection.
/// 2. Starts the Win32 event-driven clipboard listener.
/// 3. On each clipboard change: reads content, deduplicates via
///    `LAST_APP_SET_HASH`, saves to the repository, and sends a
///    `MonitorEvent` on the returned channel.
pub fn start_monitor(conn: Arc<Mutex<Connection>>) -> mpsc::Receiver<MonitorEvent> {
    let (tx, rx) = mpsc::channel();

    let repo = Arc::new(SqliteClipboardRepository::new(conn));

    let callback = {
        let tx = tx.clone();
        let repo = repo.clone();

        Arc::new(move || {
            // Pause check
            if CLIPBOARD_MONITOR_PAUSED.load(Ordering::Relaxed) {
                return;
            }

            crate::info!("monitor: callback fired");

            // Read clipboard content
            let (content, content_type, html_content) = match read_clipboard_content() {
                Some(result) => result,
                None => {
                    crate::info!("monitor: read_clipboard_content returned None");
                    return;
                }
            };

            // Echo prevention: skip our own clipboard writes
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            content.hash(&mut hasher);
            let content_hash_val = hasher.finish();

            if content_hash_val == LAST_APP_SET_HASH.load(Ordering::SeqCst)
                || content_hash_val == LAST_APP_SET_HASH_ALT.load(Ordering::SeqCst)
            {
                crate::info!(
                    "monitor: skipping our own clipboard write (hash={})",
                    content_hash_val
                );
                return;
            }

            // Resolve source application
            let source_info = get_clipboard_source_app_info();

            // Build ClipboardEntry
            let now_ms = chrono::Utc::now().timestamp_millis();
            let preview: String = if content_type == "text" || content_type == "rich_text" {
                let truncated: String = content.chars().take(100).collect();
                truncated.replace('\r', "").replace('\n', " ")
            } else {
                String::new()
            };

            // Apply privacy protection if enabled
            let content_for_save = if crate::services::privacy::maybe_encrypt_content(&content, &content_type).is_some() {
                // Content has sensitive data - encrypt it
                if let Some(encrypted) = crate::database::encryption::encrypt_value(&content) {
                    encrypted
                } else {
                    content.clone()
                }
            } else {
                content.clone()
            };

            let entry = ClipboardEntry {
                id: 0,
                content_type,
                content: content_for_save,
                html_content,
                source_app: source_info.app_name,
                source_app_path: source_info.process_path,
                timestamp: now_ms,
                preview,
                is_pinned: false,
                tags: Vec::new(),
                use_count: 1,
                is_external: false,
                pinned_order: 0,
                file_preview_exists: false,
            };

            // Save to repository (no data_dir → skip file attachments for now)
            match repo.save(&entry, None) {
                Ok(saved_id) => {
                    let mut saved = entry;
                    saved.id = saved_id;
                    let _ = tx.send(MonitorEvent::ClipboardUpdated(saved));
                }
                Err(e) => {
                    crate::error!("Failed to save clipboard entry: {}", e);
                }
            }
        })
    };

    crate::services::clipboard_listener::listen_clipboard(callback);

    rx
}

/// Try to read clipboard content. Returns `(content, content_type, html)`.
///
/// Priority: image > file > rich_text > text.
/// Returns `None` when clipboard is empty or unreadable.
fn read_clipboard_content() -> Option<(String, String, Option<String>)> {
    unsafe {
        // Priority 1: Image (CF_DIB/CF_DIBV5)
        if let Some(img) = crate::infrastructure::windows_api::win_clipboard::get_clipboard_image()
        {
            
            let img_rgba =
                image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes)?;
            let mut png_buf: Vec<u8> = Vec::new();
            let _ = img_rgba.write_to(
                &mut std::io::Cursor::new(&mut png_buf),
                image::ImageFormat::Png,
            );
            let b64 = base64::engine::general_purpose::STANDARD.encode(&png_buf);
            let data_url = format!("data:image/png;base64,{}", b64);
            return Some((data_url, "image".to_string(), None));
        }

        // Priority 2: File (CF_HDROP)
        if let Some(files) =
            crate::infrastructure::windows_api::win_clipboard::get_clipboard_files()
        {
            let content = files.join("\n");
            return Some((content, "file".to_string(), None));
        }

        // Priority 3: Rich text (CF_HTML)
        let html = crate::infrastructure::windows_api::win_clipboard::get_clipboard_html()
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty());

        // Priority 4: Text (CF_UNICODETEXT)
        if let Some(text) = crate::infrastructure::windows_api::win_clipboard::get_clipboard_text()
        {
            if text.is_empty() {
                return None;
            }
            if let Some(ref html_val) = html {
                let clean_html = html_val
                    .replace("<html>", "")
                    .replace("</html>", "")
                    .replace("<body>", "")
                    .replace("</body>", "")
                    .trim()
                    .to_string();
                if !clean_html.is_empty() && !clean_html.eq_ignore_ascii_case(text.trim()) {
                    return Some((text, "rich_text".to_string(), html));
                }
            }
            // Detect URLs
            let trimmed = text.trim();
            if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
                && !trimmed.contains('\n')
                && trimmed.len() < 2048
            {
                return Some((text, "url".to_string(), None));
            }
            return Some((text, "text".to_string(), None));
        }

        None
    }
}
