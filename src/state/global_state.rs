use std::sync::atomic::AtomicPtr;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, AtomicUsize};

/// Main window raw handle (HWND as usize, 0 = unset).
pub static MAIN_WINDOW_HANDLE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

pub static HOOK_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static HOOK_MOUSE_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

#[derive(Clone, Debug)]
pub struct HookHotkey {
    pub vk: u32,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
}

pub static TARGET_HOTKEY: std::sync::Mutex<Option<HookHotkey>> = std::sync::Mutex::new(None);

pub static IS_RECORDING: AtomicBool = AtomicBool::new(false);
pub static IGNORE_BLUR: AtomicBool = AtomicBool::new(false);
pub static WINDOW_PINNED: AtomicBool = AtomicBool::new(false);
pub static CLIPBOARD_MONITOR_PAUSED: AtomicBool = AtomicBool::new(false);
pub static LAST_ACTIVE_HWND: AtomicUsize = AtomicUsize::new(0);
pub static LAST_APP_SET_HASH: AtomicU64 = AtomicU64::new(0);
pub static LAST_APP_SET_HASH_ALT: AtomicU64 = AtomicU64::new(0);
pub static LAST_APP_SET_TIMESTAMP: AtomicU64 = AtomicU64::new(0);
pub static LAST_TOGGLE_TIMESTAMP: AtomicU64 = AtomicU64::new(0);
pub static LAST_SHOW_TIMESTAMP: AtomicU64 = AtomicU64::new(0);
pub static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
pub static TASKBAR_CREATED_MSG: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DockPosition {
    None,
    Top,
    Left,
    Right,
}

pub static CURRENT_DOCK: AtomicI32 = AtomicI32::new(0); // 0: None, 1: Top, 2: Left, 3: Right
pub static IS_HIDDEN: AtomicBool = AtomicBool::new(false);
pub static IS_MOUSE_BUTTON_DOWN: AtomicBool = AtomicBool::new(false);
pub static NAVIGATION_ENABLED: AtomicBool = AtomicBool::new(false);
pub static NAVIGATION_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);
pub static IS_MAIN_WINDOW_FOCUSED: AtomicBool = AtomicBool::new(false);
pub static IS_SEARCH_FOCUSED: AtomicBool = AtomicBool::new(false);
pub static LAST_GLOBAL_HOTKEY_TIMESTAMP: AtomicU64 = AtomicU64::new(0);

// ── Phase 4: Theme, compact mode, edge docking ──────────────────────────

/// Current theme: 0 = light, 1 = dark, 2 = system.
pub static CURRENT_THEME: AtomicI32 = AtomicI32::new(0);

/// Whether compact mode is active.
pub static IS_COMPACT: AtomicBool = AtomicBool::new(false);

/// Whether edge docking is enabled.
pub static EDGE_DOCKING_ENABLED: AtomicBool = AtomicBool::new(false);

/// Dock edge: 0 = none, 1 = left, 2 = right, 3 = top, 4 = bottom.
pub static DOCK_EDGE: AtomicI32 = AtomicI32::new(0);

// ── Keyboard navigation ─────────────────────────────────────────────────

/// Currently selected item index in the clipboard list (-1 = none).
pub static SELECTED_INDEX: AtomicI32 = AtomicI32::new(-1);

/// Currently hovered item index in the clipboard list (-1 = none).
pub static HOVERED_INDEX: AtomicI32 = AtomicI32::new(-1);

/// Total number of items in the filtered list (set by UI on each render).
pub static LIST_ITEM_COUNT: AtomicI32 = AtomicI32::new(0);

// ── Toast / Dialog state ────────────────────────────────────────────────

/// Toast message (auto-clears after timeout). Empty = no toast.
pub static TOAST_MESSAGE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
/// Toast timestamp (when it was shown, for auto-hide).
pub static TOAST_TIMESTAMP: AtomicU64 = AtomicU64::new(0);
/// Confirm dialog state: (message, confirmed).
pub static CONFIRM_DIALOG: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Database connection — separate from APP_CTX to avoid lock contention with the monitor thread.
pub static DB_CONN: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>> =
    std::sync::OnceLock::new();

/// Shutdown signal — when true, all background threads should exit their loops.
pub static SHUTDOWN: AtomicBool = AtomicBool::new(false);
