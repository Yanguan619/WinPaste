use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc, Mutex};

use crate::database;
use crate::services::clipboard_monitor;
use crate::services::clipboard_monitor::MonitorEvent;
use crate::state::app_state::SettingsState;
use crate::state::global_state::*;
use crate::{error, info};

/// Application initialization state.
pub struct AppContext {
    pub data_dir: PathBuf,
    pub db_conn: Arc<Mutex<rusqlite::Connection>>,
    pub monitor_rx: mpsc::Receiver<MonitorEvent>,
    pub settings: Arc<SettingsState>,
}

/// Initialize the application: data directories, logger, database, clipboard monitor.
pub fn init() -> AppContext {
    // 1. Determine data directory
    let data_dir = get_data_dir();

    // 2. Initialize logger
    let log_path = data_dir.join("clipboard.log");
    crate::logger::init(log_path);
    info!("Logger initialized");

    // 3. Ensure data directory exists
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        error!("Failed to create data directory: {}", e);
    }

    // 4. Initialize database
    let db_path = data_dir.join("clipboard.db");
    let db_path_str = db_path.to_string_lossy().to_string();
    let db = match database::init_db(&db_path_str) {
        Ok(conn) => {
            info!("Database initialized at: {}", db_path_str);
            conn
        }
        Err(e) => {
            error!("Failed to initialize database: {}", e);
            panic!("Database initialization failed: {}", e);
        }
    };
    let db_conn = Arc::new(Mutex::new(db));

    // Store db_conn in global for settings reads (avoids locking APP_CTX)
    let _ = crate::state::global_state::DB_CONN.set(db_conn.clone());

    // 5. Load settings from DB into runtime state
    let settings = Arc::new(SettingsState::default());
    crate::state::app_state::load_settings_from_db(&settings, &db_conn);
    info!("Settings loaded from database");

    // 6. Start clipboard monitor
    let my_hwnd = MAIN_WINDOW_HANDLE.get().copied().unwrap_or(0);
    LAST_ACTIVE_HWND.store(my_hwnd, Ordering::SeqCst);
    let monitor_rx = clipboard_monitor::start_monitor(db_conn.clone());
    info!("Clipboard monitor started in background thread");

    // 7. Start window tracking
    crate::infrastructure::windows_api::window_tracker::start_window_tracking();
    info!("Window tracking started");

    // 8. Apply startup settings to global state
    apply_settings_to_globals(&settings);

    info!("App initialized");
    AppContext {
        data_dir,
        db_conn,
        monitor_rx,
        settings,
    }
}

/// Apply loaded settings to global atomic state.
fn apply_settings_to_globals(settings: &SettingsState) {
    WINDOW_PINNED.store(false, Ordering::Relaxed);
    EDGE_DOCKING_ENABLED.store(settings.edge_docking.load(Ordering::Relaxed), Ordering::Relaxed);
    IS_COMPACT.store(settings.compact_mode.load(Ordering::Relaxed), Ordering::Relaxed);
    let theme_idx = match settings.color_mode.lock().unwrap().as_str() {
        "light" => 0,
        "dark" => 1,
        _ => 2,
    };
    CURRENT_THEME.store(theme_idx, Ordering::Relaxed);
}

/// Get the application data directory.
/// Uses separate directory from WinPaste to avoid database lock conflicts.
fn get_data_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .unwrap_or_else(|_| {
            let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
            format!("{}\\AppData\\Roaming", home)
        });
    PathBuf::from(base).join("com.clipboard.app")
}
