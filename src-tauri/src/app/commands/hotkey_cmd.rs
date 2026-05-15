use tauri::{AppHandle, Manager};
use crate::app_state::SettingsState;
use crate::error::{AppResult, AppError};
use crate::global_state::HOTKEY_STRING;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

fn normalize_shortcut_string(hk: &str) -> String {
    hk.replace("Win", "Super")
        .replace("Shift+!", "Shift+1")
        .replace("Shift+@", "Shift+2")
        .replace("Shift+#", "Shift+3")
        .replace("Shift+$", "Shift+4")
        .replace("Shift+%", "Shift+5")
        .replace("Shift+^", "Shift+6")
        .replace("Shift+&", "Shift+7")
        .replace("Shift+*", "Shift+8")
        .replace("Shift+(", "Shift+9")
        .replace("Shift+)", "Shift+0")
}

#[tauri::command]
pub fn register_hotkey(app_handle: AppHandle, hotkey: String) -> AppResult<()> {
    crate::info!(">>> [DEBUG] Registering hotkey: {}", hotkey);
    {
        let mut guard = HOTKEY_STRING.lock().unwrap();
        *guard = hotkey.clone();
    }

    if let Some(settings) = app_handle.try_state::<SettingsState>() {
        let mut guard = settings.main_hotkey.lock().unwrap();
        *guard = hotkey.clone();
    }
    
    let _ = app_handle.global_shortcut().unregister_all();
    
    if !hotkey.is_empty() {
        let normalized = normalize_shortcut_string(&hotkey);
        crate::info!(">>> [DEBUG] Normalized main hotkey: {}", normalized);
        if hotkey.eq_ignore_ascii_case("MouseMiddle") || hotkey.eq_ignore_ascii_case("MButton") {
            // Mouse middle handled in hooks
        } else {
            match normalized.parse::<Shortcut>() {
                Ok(shortcut) => {
                    let res = app_handle.global_shortcut().register(shortcut);
                    crate::info!(">>> [DEBUG] Register main hotkey result: {:?}", res);
                },
                Err(e) => {
                    crate::info!(">>> [DEBUG] Parse main hotkey failed: {:?}", e);
                }
            }
        }
    }
    
    // Check and register other hotkeys
    let register_other = |val: String, name: &str| {
        if !val.is_empty() {
            let norm = normalize_shortcut_string(&val);
            if let Ok(shortcut) = norm.parse::<Shortcut>() {
                let res = app_handle.global_shortcut().register(shortcut);
                crate::info!(">>> [DEBUG] Register {} result: {:?}", name, res);
            }
        }
    };

    let settings = app_handle.state::<SettingsState>();
    register_other(settings.sequential_paste_hotkey.lock().unwrap().clone(), "sequential");
    register_other(settings.rich_paste_hotkey.lock().unwrap().clone(), "rich");
    register_other(settings.search_hotkey.lock().unwrap().clone(), "search");
    
    Ok(())
}

#[tauri::command]
pub fn test_hotkey_available(app_handle: AppHandle, hotkey: String) -> AppResult<bool> {
    crate::info!(">>> [DEBUG] Testing hotkey available: {}", hotkey);
    if hotkey.is_empty() || hotkey.eq_ignore_ascii_case("MouseMiddle") || hotkey.eq_ignore_ascii_case("MButton") {
        return Ok(true);
    }
    
    let normalized = normalize_shortcut_string(&hotkey);
    crate::info!(">>> [DEBUG] Testing normalized: {}", normalized);
    let shortcut = match normalized.parse::<Shortcut>() {
        Ok(s) => s,
        Err(e) => {
            crate::info!(">>> [DEBUG] Test parse failed: {:?}", e);
            return Err(AppError::Validation("快捷键格式无效".to_string()));
        }
    };
    
    match app_handle.global_shortcut().register(shortcut.clone()) {
        Ok(_) => {
            let _ = app_handle.global_shortcut().unregister(shortcut);
            Ok(true)
        },
        Err(e) => {
            let err_str = format!("{:?}", e);
            crate::info!(">>> [DEBUG] Test register failed: {:?}", err_str);
            let user_msg = if err_str.contains("AlreadyRegistered") {
                "该快捷键已被其他程序占用".to_string()
            } else {
                "快捷键不可用".to_string()
            };
            Err(AppError::Internal(user_msg))
        }
    }
}
