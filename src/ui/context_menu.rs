/// Win32 context menu (right-click popup menu) for clipboard items.
use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, TrackPopupMenu, TPM_RETURNCMD, TPM_TOPALIGN, TPM_LEFTALIGN,
    AppendMenuW, MF_STRING, MF_SEPARATOR, MF_GRAYED,
};

use crate::error;
use crate::infrastructure::repository::clipboard_repo::{ClipboardRepository, SqliteClipboardRepository};

/// Context menu command IDs
const CMD_COPY: u32 = 1001;
const CMD_PASTE: u32 = 1002;
const CMD_PIN: u32 = 1003;
const CMD_UNPIN: u32 = 1004;
const CMD_DELETE: u32 = 1005;
const CMD_OPEN: u32 = 1006;
const CMD_CREATE_STICKY: u32 = 1007;

/// Show a context menu at the given screen position for the specified entry.
/// Returns the selected action as a string.
pub fn show_context_menu(entry_id: i64, is_pinned: bool, content_type: &str, screen_x: i32, screen_y: i32) -> Option<String> {
    unsafe {
        let hmenu = CreatePopupMenu();
        if hmenu.is_err() {
            error!("Failed to create popup menu");
            return None;
        }
        let hmenu = hmenu.unwrap();

        let copy_text = "\u{2398} 复制\0";
        let paste_text = "\u{2328} 粘贴\0";
        let pin_text = if is_pinned { "\u{2606} 取消置顶\0" } else { "\u{2605} 置顶\0" };
        let delete_text = "\u{1F5D1} 删除\0";
        let open_text = "\u{1F4C2} 打开\0";
        let sticky_text = "\u{1F4DD} 创建贴图\0";

        // Add menu items
        let _ = AppendMenuW(hmenu, MF_STRING, CMD_COPY as usize, windows::core::PCWSTR(copy_text.as_ptr() as _));
        let _ = AppendMenuW(hmenu, MF_STRING, CMD_PASTE as usize, windows::core::PCWSTR(paste_text.as_ptr() as _));
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(hmenu, MF_STRING, CMD_PIN as usize, windows::core::PCWSTR(pin_text.as_ptr() as _));
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, None);

        // Open only for file type
        if content_type == "file" {
            let _ = AppendMenuW(hmenu, MF_STRING, CMD_OPEN as usize, windows::core::PCWSTR(open_text.as_ptr() as _));
        } else {
            let _ = AppendMenuW(hmenu, MF_STRING | MF_GRAYED, CMD_OPEN as usize, windows::core::PCWSTR(open_text.as_ptr() as _));
        }

        let _ = AppendMenuW(hmenu, MF_STRING, CMD_CREATE_STICKY as usize, windows::core::PCWSTR(sticky_text.as_ptr() as _));
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(hmenu, MF_STRING, CMD_DELETE as usize, windows::core::PCWSTR(delete_text.as_ptr() as _));

        // Show the menu
        let pt = POINT { x: screen_x, y: screen_y };
        let cmd = TrackPopupMenu(
            hmenu,
            TPM_RETURNCMD | TPM_TOPALIGN | TPM_LEFTALIGN,
            pt.x,
            pt.y,
            Some(0),
            HWND(std::ptr::null_mut()),
            Some(std::ptr::null()),
        );

        // Process the selected command
        match cmd.0 as u32 {
            CMD_COPY => {
                // Copy entry content to clipboard
                copy_entry_content(entry_id);
                Some("copy".to_string())
            }
            CMD_PASTE => {
                // Paste the entry
                paste_entry_by_id(entry_id);
                Some("paste".to_string())
            }
            CMD_PIN | CMD_UNPIN => {
                // Toggle pin
                toggle_pin_entry(entry_id);
                Some("pin".to_string())
            }
            CMD_DELETE => {
                // Delete the entry
                delete_entry(entry_id);
                Some("delete".to_string())
            }
            CMD_OPEN => {
                // Open file with default app
                open_entry_file(entry_id);
                Some("open".to_string())
            }
            CMD_CREATE_STICKY => {
                // Create sticky note from entry
                create_sticky_from_entry(entry_id);
                Some("sticky".to_string())
            }
            _ => None
        }
    }
}

/// Copy entry content to clipboard.
fn copy_entry_content(entry_id: i64) {
    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = SqliteClipboardRepository::new(ctx.db_conn.clone());
        if let Ok(Some(entry)) = repo.get_entry_by_id(entry_id) {
            drop(ctx);
            let _ = crate::services::clipboard_ops::copy_text(&entry.content);
            crate::ui::main_window::show_toast("已复制到剪贴板");
        }
    }
}

/// Paste entry by ID.
fn paste_entry_by_id(entry_id: i64) {
    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = SqliteClipboardRepository::new(ctx.db_conn.clone());
        if let Ok(Some(entry)) = repo.get_entry_by_id(entry_id) {
            drop(ctx);
            let _ = crate::services::clipboard_ops::paste_entry(&entry);
        }
    }
}

/// Toggle pin state of an entry.
fn toggle_pin_entry(entry_id: i64) {
    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = SqliteClipboardRepository::new(ctx.db_conn.clone());
        // Get current pin state
        if let Ok(Some(entry)) = repo.get_entry_by_id(entry_id) {
            let new_pinned = !entry.is_pinned;
            drop(ctx);
            let _ = repo.toggle_pin(entry_id, new_pinned);
            crate::ui::main_window::show_toast(if new_pinned { "已置顶" } else { "已取消置顶" });
            // Trigger UI refresh
            crate::state::global_state::SELECTED_INDEX.store(
                crate::state::global_state::SELECTED_INDEX.load(Ordering::Relaxed),
                Ordering::Relaxed,
            );
        }
    }
}

/// Delete an entry.
fn delete_entry(entry_id: i64) {
    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = SqliteClipboardRepository::new(ctx.db_conn.clone());
        let data_dir = ctx.data_dir.clone();
        drop(ctx);
        let data_dir_path = Some(std::path::Path::new(&data_dir));
        if let Err(e) = repo.delete(entry_id, data_dir_path) {
            error!("Failed to delete entry: {}", e);
        } else {
            // Remove from UI cache
            let mut entries = crate::CLIPBOARD_ENTRIES.lock().unwrap();
            entries.retain(|e| e.id != entry_id);
            crate::ui::main_window::show_toast("已删除");
        }
    }
}

/// Open file entry with default application.
fn open_entry_file(entry_id: i64) {
    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = SqliteClipboardRepository::new(ctx.db_conn.clone());
        if let Ok(Some(entry)) = repo.get_entry_by_id(entry_id) {
            drop(ctx);
            if entry.content_type == "file" {
                let first_file = entry.content.lines().next().unwrap_or("");
                if !first_file.is_empty() {
                    let _ = std::process::Command::new("explorer")
                        .arg(first_file)
                        .spawn();
                }
            }
        }
    }
}

/// Create a sticky note from an entry.
fn create_sticky_from_entry(entry_id: i64) {
    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = SqliteClipboardRepository::new(ctx.db_conn.clone());
        if let Ok(Some(entry)) = repo.get_entry_by_id(entry_id) {
            drop(ctx);
            crate::app::sticky_manager::create_sticky_from_clipboard(&entry.content);
            crate::ui::main_window::show_toast("已创建贴图");
        }
    }
}
