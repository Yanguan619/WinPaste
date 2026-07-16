/// Drag-and-drop support for the clipboard manager.
use windows::Win32::Foundation::HANDLE;
use windows::Win32::UI::Shell::{DragQueryFileW, DragFinish};

use crate::infrastructure::repository::clipboard_repo::ClipboardRepository;
use crate::{error, info};

pub unsafe fn handle_drop_files(h_drop: usize) -> Vec<String> {
    let mut paths = Vec::new();
    let h = HANDLE(h_drop as *mut _);

    let count = DragQueryFileW(Some(h), 0xFFFF, std::ptr::null_mut(), 0);
    let count = count.min(100);

    for i in 0..count {
        let len = DragQueryFileW(Some(h), i, std::ptr::null_mut(), 0);
        if len == 0 { continue; }

        let mut buffer = vec![0u16; (len + 1) as usize];
        let result = DragQueryFileW(Some(h), i, buffer.as_mut_ptr() as *mut u16, len + 1);

        if result > 0 {
            let path = String::from_utf16_lossy(&buffer[..result as usize]);
            if !path.is_empty() {
                paths.push(path);
            }
        }
    }

    DragFinish(Some(h));
    info!("Dropped {} files", paths.len());
    paths
}

pub fn save_dropped_files(paths: &[String]) {
    if paths.is_empty() { return; }

    let content = paths.join("\n");
    let preview = if paths.len() == 1 {
        std::path::Path::new(&paths[0])
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| paths[0].clone())
    } else {
        format!("{} files", paths.len())
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let entry = crate::domain::ClipboardEntry {
        id: 0,
        content_type: "file".to_string(),
        content,
        html_content: None,
        source_app: "Drag & Drop".to_string(),
        source_app_path: None,
        timestamp: now,
        preview,
        is_pinned: false,
        tags: vec![],
        use_count: 1,
        is_external: true,
        pinned_order: 0,
        file_preview_exists: true,
    };

    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = crate::infrastructure::repository::clipboard_repo::SqliteClipboardRepository::new(
            ctx.db_conn.clone(),
        );
        match repo.save(&entry, None) {
            Ok(id) => {
                let mut entry = entry;
                entry.id = id;
                crate::CLIPBOARD_ENTRIES.lock().unwrap().insert(0, entry);
                info!("Saved {} dropped files", paths.len());
            }
            Err(e) => {
                error!("Failed to save dropped files: {}", e);
            }
        }
    }
}

pub fn save_dropped_url(url: &str) {
    if url.is_empty() { return; }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let entry = crate::domain::ClipboardEntry {
        id: 0,
        content_type: "text".to_string(),
        content: url.to_string(),
        html_content: None,
        source_app: "Drag & Drop".to_string(),
        source_app_path: None,
        timestamp: now,
        preview: url.to_string(),
        is_pinned: false,
        tags: vec![],
        use_count: 1,
        is_external: false,
        pinned_order: 0,
        file_preview_exists: false,
    };

    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = crate::infrastructure::repository::clipboard_repo::SqliteClipboardRepository::new(
            ctx.db_conn.clone(),
        );
        match repo.save(&entry, None) {
            Ok(id) => {
                let mut entry = entry;
                entry.id = id;
                crate::CLIPBOARD_ENTRIES.lock().unwrap().insert(0, entry);
                info!("Saved dropped URL: {}", url);
            }
            Err(e) => {
                error!("Failed to save dropped URL: {}", e);
            }
        }
    }
}
