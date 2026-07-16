use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;

use crate::domain::ClipboardEntry;

pub struct DisplayMonitor {
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub x: i32,
    pub y: i32,
}

pub struct SettingsState {
    pub auto_start: AtomicBool,
    pub deduplicate: AtomicBool,
    pub persistent: AtomicBool,
    pub persistent_limit_enabled: AtomicBool,
    pub persistent_limit: AtomicI32,
    pub theme: Mutex<String>,
    pub color_mode: Mutex<String>,
    pub capture_files: AtomicBool,
    pub capture_rich_text: AtomicBool,
    pub rich_text_snapshot_preview: AtomicBool,
    pub silent_start: AtomicBool,
    pub delete_after_paste: AtomicBool,
    pub move_to_top_after_paste: AtomicBool,
    pub privacy_protection: AtomicBool,
    pub privacy_protection_kinds: Mutex<Vec<String>>,
    pub privacy_protection_custom_rules: Mutex<String>,
    pub sequential_mode: AtomicBool,
    pub sequential_paste_hotkey: Mutex<String>,
    pub rich_paste_hotkey: Mutex<String>,
    pub search_hotkey: Mutex<String>,
    pub sound_enabled: AtomicBool,
    pub sound_paste_enabled: AtomicBool,
    pub hide_tray_icon: AtomicBool,
    pub edge_docking: AtomicBool,
    pub follow_mouse: AtomicBool,
    pub follow_caret: AtomicBool,
    pub arrow_key_selection: AtomicBool,
    pub main_hotkey: Mutex<String>,
    pub quick_paste_enabled: AtomicBool,
    pub sticky_enabled: AtomicBool,
    pub compact_mode: AtomicBool,
    pub show_app_border: AtomicBool,
    pub show_source_app_icon: AtomicBool,
    pub vibrancy_enabled: AtomicBool,
    pub use_win_v_shortcut: AtomicBool,
    pub auto_hide_tags: AtomicBool,
    pub pinned_collapsed: AtomicBool,
    pub paste_method: Mutex<String>,
    pub clipboard_item_font_size: Mutex<f64>,
    pub clipboard_tag_font_size: Mutex<f64>,
    pub monitors: Mutex<Vec<DisplayMonitor>>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            auto_start: AtomicBool::new(true),
            deduplicate: AtomicBool::new(true),
            persistent: AtomicBool::new(false),
            persistent_limit_enabled: AtomicBool::new(true),
            persistent_limit: AtomicI32::new(500),
            theme: Mutex::new("fluent".to_string()),
            color_mode: Mutex::new("dark".to_string()),
            capture_files: AtomicBool::new(false),
            capture_rich_text: AtomicBool::new(false),
            rich_text_snapshot_preview: AtomicBool::new(false),
            silent_start: AtomicBool::new(true),
            delete_after_paste: AtomicBool::new(false),
            move_to_top_after_paste: AtomicBool::new(true),
            privacy_protection: AtomicBool::new(true),
            privacy_protection_kinds: Mutex::new(vec![
                "phone".to_string(),
                "idcard".to_string(),
                "email".to_string(),
                "secret".to_string(),
            ]),
            privacy_protection_custom_rules: Mutex::new(String::new()),
            sequential_mode: AtomicBool::new(false),
            sequential_paste_hotkey: Mutex::new("Alt+V".to_string()),
            rich_paste_hotkey: Mutex::new("Ctrl+Shift+Z".to_string()),
            search_hotkey: Mutex::new(String::new()),
            sound_enabled: AtomicBool::new(false),
            sound_paste_enabled: AtomicBool::new(true),
            hide_tray_icon: AtomicBool::new(false),
            edge_docking: AtomicBool::new(false),
            follow_mouse: AtomicBool::new(true),
            follow_caret: AtomicBool::new(false),
            arrow_key_selection: AtomicBool::new(true),
            main_hotkey: Mutex::new("Alt+C".to_string()),
            quick_paste_enabled: AtomicBool::new(true),
            sticky_enabled: AtomicBool::new(false),
            compact_mode: AtomicBool::new(false),
            show_app_border: AtomicBool::new(true),
            show_source_app_icon: AtomicBool::new(true),
            vibrancy_enabled: AtomicBool::new(false),
            use_win_v_shortcut: AtomicBool::new(false),
            auto_hide_tags: AtomicBool::new(false),
            pinned_collapsed: AtomicBool::new(false),
            paste_method: Mutex::new("shift_insert".to_string()),
            clipboard_item_font_size: Mutex::new(14.0),
            clipboard_tag_font_size: Mutex::new(10.0),
            monitors: Mutex::new(Vec::new()),
        }
    }
}

impl SettingsState {
    /// Create a snapshot copy from a reference to another SettingsState.
    pub fn from_ref(other: &SettingsState) -> Self {
        let s = Self::default();
        s.auto_start.store(other.auto_start.load(Ordering::Relaxed), Ordering::Relaxed);
        s.deduplicate.store(other.deduplicate.load(Ordering::Relaxed), Ordering::Relaxed);
        s.persistent.store(other.persistent.load(Ordering::Relaxed), Ordering::Relaxed);
        s.persistent_limit_enabled.store(other.persistent_limit_enabled.load(Ordering::Relaxed), Ordering::Relaxed);
        s.persistent_limit.store(other.persistent_limit.load(Ordering::Relaxed), Ordering::Relaxed);
        s.capture_files.store(other.capture_files.load(Ordering::Relaxed), Ordering::Relaxed);
        s.capture_rich_text.store(other.capture_rich_text.load(Ordering::Relaxed), Ordering::Relaxed);
        s.rich_text_snapshot_preview.store(other.rich_text_snapshot_preview.load(Ordering::Relaxed), Ordering::Relaxed);
        s.silent_start.store(other.silent_start.load(Ordering::Relaxed), Ordering::Relaxed);
        s.delete_after_paste.store(other.delete_after_paste.load(Ordering::Relaxed), Ordering::Relaxed);
        s.move_to_top_after_paste.store(other.move_to_top_after_paste.load(Ordering::Relaxed), Ordering::Relaxed);
        s.privacy_protection.store(other.privacy_protection.load(Ordering::Relaxed), Ordering::Relaxed);
        s.sequential_mode.store(other.sequential_mode.load(Ordering::Relaxed), Ordering::Relaxed);
        s.sound_enabled.store(other.sound_enabled.load(Ordering::Relaxed), Ordering::Relaxed);
        s.sound_paste_enabled.store(other.sound_paste_enabled.load(Ordering::Relaxed), Ordering::Relaxed);
        s.hide_tray_icon.store(other.hide_tray_icon.load(Ordering::Relaxed), Ordering::Relaxed);
        s.edge_docking.store(other.edge_docking.load(Ordering::Relaxed), Ordering::Relaxed);
        s.follow_mouse.store(other.follow_mouse.load(Ordering::Relaxed), Ordering::Relaxed);
        s.follow_caret.store(other.follow_caret.load(Ordering::Relaxed), Ordering::Relaxed);
        s.arrow_key_selection.store(other.arrow_key_selection.load(Ordering::Relaxed), Ordering::Relaxed);
        s.quick_paste_enabled.store(other.quick_paste_enabled.load(Ordering::Relaxed), Ordering::Relaxed);
        s.sticky_enabled.store(other.sticky_enabled.load(Ordering::Relaxed), Ordering::Relaxed);
        s.compact_mode.store(other.compact_mode.load(Ordering::Relaxed), Ordering::Relaxed);
        s.show_app_border.store(other.show_app_border.load(Ordering::Relaxed), Ordering::Relaxed);
        s.show_source_app_icon.store(other.show_source_app_icon.load(Ordering::Relaxed), Ordering::Relaxed);
        s.vibrancy_enabled.store(other.vibrancy_enabled.load(Ordering::Relaxed), Ordering::Relaxed);
        s.use_win_v_shortcut.store(other.use_win_v_shortcut.load(Ordering::Relaxed), Ordering::Relaxed);
        s.auto_hide_tags.store(other.auto_hide_tags.load(Ordering::Relaxed), Ordering::Relaxed);
        s.pinned_collapsed.store(other.pinned_collapsed.load(Ordering::Relaxed), Ordering::Relaxed);
        *s.theme.lock().unwrap() = other.theme.lock().unwrap().clone();
        *s.color_mode.lock().unwrap() = other.color_mode.lock().unwrap().clone();
        *s.main_hotkey.lock().unwrap() = other.main_hotkey.lock().unwrap().clone();
        *s.sequential_paste_hotkey.lock().unwrap() = other.sequential_paste_hotkey.lock().unwrap().clone();
        *s.rich_paste_hotkey.lock().unwrap() = other.rich_paste_hotkey.lock().unwrap().clone();
        *s.search_hotkey.lock().unwrap() = other.search_hotkey.lock().unwrap().clone();
        *s.paste_method.lock().unwrap() = other.paste_method.lock().unwrap().clone();
        *s.privacy_protection_kinds.lock().unwrap() = other.privacy_protection_kinds.lock().unwrap().clone();
        *s.privacy_protection_custom_rules.lock().unwrap() = other.privacy_protection_custom_rules.lock().unwrap().clone();
        *s.clipboard_item_font_size.lock().unwrap() = *other.clipboard_item_font_size.lock().unwrap();
        *s.clipboard_tag_font_size.lock().unwrap() = *other.clipboard_tag_font_size.lock().unwrap();
        s
    }
}

#[derive(Default)]
pub struct PasteQueueState {
    pub items: VecDeque<i64>,
    pub last_action_was_paste: bool,
    pub last_pasted_content: Option<String>,
}

pub struct PasteQueue(pub Mutex<PasteQueueState>);

pub struct SessionHistory(pub Mutex<VecDeque<ClipboardEntry>>);

pub struct AppDataDir(pub Mutex<std::path::PathBuf>);

/// Load all settings from the database into the given SettingsState.
pub fn load_settings_from_db(settings: &SettingsState, db_conn: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>) {
    use crate::infrastructure::repository::settings_repo::{SqliteSettingsRepository, SettingsRepository};
    let repo = SqliteSettingsRepository::new(db_conn.clone());
    if let Ok(all) = repo.get_all() {
        let get = |key: &str| -> Option<String> { all.get(key).cloned() };
        let get_bool = |key: &str| -> bool {
            get(key).map(|v| v == "true").unwrap_or(false)
        };
        let get_i32 = |key: &str| -> i32 {
            get(key).and_then(|v| v.parse().ok()).unwrap_or(0)
        };

        settings.auto_start.store(get_bool("app.autostart"), Ordering::Relaxed);
        settings.deduplicate.store(get_bool("app.deduplicate"), Ordering::Relaxed);
        settings.persistent.store(get_bool("app.persistent"), Ordering::Relaxed);
        settings.persistent_limit_enabled.store(get_bool("app.persistent_limit_enabled"), Ordering::Relaxed);
        settings.persistent_limit.store(get_i32("app.persistent_limit").max(50), Ordering::Relaxed);
        settings.capture_files.store(get_bool("app.capture_files"), Ordering::Relaxed);
        settings.capture_rich_text.store(get_bool("app.capture_rich_text"), Ordering::Relaxed);
        settings.rich_text_snapshot_preview.store(get_bool("app.rich_text_snapshot_preview"), Ordering::Relaxed);
        settings.silent_start.store(get_bool("app.silent_start"), Ordering::Relaxed);
        settings.delete_after_paste.store(get_bool("app.delete_after_paste"), Ordering::Relaxed);
        settings.move_to_top_after_paste.store(get_bool("app.move_to_top_after_paste"), Ordering::Relaxed);
        settings.privacy_protection.store(get_bool("app.privacy_protection"), Ordering::Relaxed);
        settings.sequential_mode.store(get_bool("app.sequential_mode"), Ordering::Relaxed);
        settings.sound_enabled.store(get_bool("app.sound_enabled"), Ordering::Relaxed);
        settings.sound_paste_enabled.store(get_bool("app.sound_paste_enabled"), Ordering::Relaxed);
        settings.hide_tray_icon.store(get_bool("app.hide_tray_icon"), Ordering::Relaxed);
        settings.edge_docking.store(get_bool("app.edge_docking"), Ordering::Relaxed);
        settings.follow_mouse.store(get_bool("app.follow_mouse"), Ordering::Relaxed);
        settings.follow_caret.store(get("app.follow_caret").map(|v| v == "true").unwrap_or(false), Ordering::Relaxed);
        settings.arrow_key_selection.store(get_bool("app.arrow_key_selection"), Ordering::Relaxed);
        settings.quick_paste_enabled.store(get_bool("app.quick_paste_enabled"), Ordering::Relaxed);
        settings.sticky_enabled.store(get_bool("app.sticky_enabled"), Ordering::Relaxed);
        settings.compact_mode.store(get_bool("app.compact_mode"), Ordering::Relaxed);
        settings.show_app_border.store(get_bool("app.show_app_border"), Ordering::Relaxed);
        settings.show_source_app_icon.store(get("app.show_source_app_icon").map(|v| v == "true").unwrap_or(true), Ordering::Relaxed);
        settings.vibrancy_enabled.store(get_bool("app.vibrancy_enabled"), Ordering::Relaxed);
        settings.use_win_v_shortcut.store(get_bool("app.use_win_v_shortcut"), Ordering::Relaxed);
        settings.auto_hide_tags.store(get("app.auto_hide_tags").map(|v| v == "true").unwrap_or(false), Ordering::Relaxed);
        settings.pinned_collapsed.store(get("app.pinned_collapsed").map(|v| v == "true").unwrap_or(false), Ordering::Relaxed);

        if let Some(v) = get("app.theme") { *settings.theme.lock().unwrap() = v; }
        if let Some(v) = get("app.color_mode") { *settings.color_mode.lock().unwrap() = v; }
        if let Some(v) = get("app.hotkey") { *settings.main_hotkey.lock().unwrap() = v; }
        if let Some(v) = get("app.sequential_hotkey") { *settings.sequential_paste_hotkey.lock().unwrap() = v; }
        if let Some(v) = get("app.rich_paste_hotkey") { *settings.rich_paste_hotkey.lock().unwrap() = v; }
        if let Some(v) = get("app.search_hotkey") { *settings.search_hotkey.lock().unwrap() = v; }
        if let Some(v) = get("app.paste_method") { *settings.paste_method.lock().unwrap() = v; }
        if let Some(v) = get("app.privacy_protection_kinds") {
            *settings.privacy_protection_kinds.lock().unwrap() =
                v.split(',').map(|s| s.to_string()).filter(|s| !s.is_empty()).collect();
        }
        if let Some(v) = get("app.privacy_protection_custom_rules") {
            *settings.privacy_protection_custom_rules.lock().unwrap() = v;
        }
        if let Some(v) = get("app.clipboard_item_font_size") {
            if let Ok(f) = v.parse::<f64>() { *settings.clipboard_item_font_size.lock().unwrap() = f; }
        }
        if let Some(v) = get("app.clipboard_tag_font_size") {
            if let Ok(f) = v.parse::<f64>() { *settings.clipboard_tag_font_size.lock().unwrap() = f; }
        }
    }
}
