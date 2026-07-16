/// Paste queue for sequential pasting of multiple items.
/// Supports Ctrl+Shift+0-9 for quick paste of top 10 items.
use std::collections::VecDeque;
use crate::infrastructure::repository::clipboard_repo::{ClipboardRepository, SqliteClipboardRepository};
use std::sync::atomic::Ordering;
use std::sync::Mutex;

static PASTE_QUEUE: std::sync::OnceLock<Mutex<VecDeque<i64>>> = std::sync::OnceLock::new();

fn queue() -> &'static Mutex<VecDeque<i64>> {
    PASTE_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Add an entry ID to the paste queue.
pub fn enqueue(entry_id: i64) {
    queue().lock().unwrap().push_back(entry_id);
}

/// Get the next entry ID from the paste queue.
pub fn dequeue() -> Option<i64> {
    queue().lock().unwrap().pop_front()
}

/// Peek at the next entry ID without removing it.
pub fn peek() -> Option<i64> {
    queue().lock().unwrap().front().copied()
}

/// Clear the paste queue.
pub fn clear() {
    queue().lock().unwrap().clear();
}

/// Get the current queue length.
pub fn len() -> usize {
    queue().lock().unwrap().len()
}

/// Check if the queue is empty.
pub fn is_empty() -> bool {
    queue().lock().unwrap().is_empty()
}

/// Get all entry IDs in the queue.
pub fn get_all() -> Vec<i64> {
    queue().lock().unwrap().iter().copied().collect()
}

/// Paste the entry at the given index from the clipboard list (0-based).
/// Used by Ctrl+Shift+0-9 quick paste.
pub fn quick_paste(index: usize) {
    let entries = crate::CLIPBOARD_ENTRIES.lock().unwrap();
    if index >= entries.len() {
        return;
    }
    let entry_id = entries[index].id;
    drop(entries);

    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = SqliteClipboardRepository::new(ctx.db_conn.clone());
        let delete_after = ctx.settings.delete_after_paste.load(Ordering::Relaxed);
        let move_to_top = ctx.settings.move_to_top_after_paste.load(Ordering::Relaxed);
        let data_dir = ctx.data_dir.clone();
        drop(ctx);

        if let Ok(Some(entry)) = repo.get_entry_by_id(entry_id) {
            let _ = crate::services::clipboard_ops::paste_entry(&entry);

            // Handle delete after paste if enabled
            if delete_after {
                let data_dir_path = Some(std::path::Path::new(&data_dir));
                let _ = repo.delete(entry_id, data_dir_path);
                let mut entries = crate::CLIPBOARD_ENTRIES.lock().unwrap();
                entries.retain(|e| e.id != entry_id);
            }

            // Handle move to top after paste if enabled
            if move_to_top {
                let now_ms = chrono::Utc::now().timestamp_millis();
                let _ = repo.touch_entry(entry_id, now_ms);
            }

            crate::ui::main_window::show_toast(&format!("已粘贴第 {} 项", index + 1));
        }
    }
}

/// Paste the next item in the sequential queue.
pub fn paste_next_in_queue() -> bool {
    let entry_id = match dequeue() {
        Some(id) => id,
        None => return false,
    };

    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = SqliteClipboardRepository::new(ctx.db_conn.clone());
        let delete_after = ctx.settings.delete_after_paste.load(Ordering::Relaxed);
        let data_dir = ctx.data_dir.clone();
        drop(ctx);

        if let Ok(Some(entry)) = repo.get_entry_by_id(entry_id) {
            let _ = crate::services::clipboard_ops::paste_entry(&entry);

            // Handle delete after paste
            if delete_after {
                let data_dir_path = Some(std::path::Path::new(&data_dir));
                let _ = repo.delete(entry_id, data_dir_path);
                let mut entries = crate::CLIPBOARD_ENTRIES.lock().unwrap();
                entries.retain(|e| e.id != entry_id);
            }

            let remaining = len();
            if remaining > 0 {
                crate::ui::main_window::show_toast(&format!("队列剩余 {} 项", remaining));
            }
            return true;
        }
    }
    false
}
