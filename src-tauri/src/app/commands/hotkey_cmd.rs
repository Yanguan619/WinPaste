use tauri::{AppHandle, Manager};
use crate::app_state::SettingsState;
use crate::error::{AppResult, AppError};
use crate::global_state::HOTKEY_STRING;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::app::setup::normalize_shortcut_string;

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
                    match app_handle.global_shortcut().register(shortcut) {
                        Ok(_) => {},
                        Err(e) => {
                            let err_str = format!("{:?}", e);
                            crate::error!(">>> [DEBUG] Failed to register main hotkey: {}", err_str);
                            return Err(AppError::Internal(format!("快捷键注册失败: {}", err_str)));
                        }
                    }
                },
                Err(e) => {
                    crate::error!(">>> [DEBUG] Parse main hotkey failed: {:?}", e);
                    return Err(AppError::Validation(format!("快捷键格式无效: {:?}", e)));
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
    let seq_mode = settings.sequential_mode.load(std::sync::atomic::Ordering::Relaxed);
    if seq_mode {
        register_other(settings.sequential_paste_hotkey.lock().unwrap().clone(), "sequential");
    }
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

    // 测试前先注销所有已有快捷键，防止旧快捷键干扰新快捷键的注册测试
    // (管理员权限下 global-shortcut 插件可能因内部实现限制导致冲突)
    let _ = app_handle.global_shortcut().unregister_all();

    match app_handle.global_shortcut().register(shortcut.clone()) {
        Ok(_) => {
            let _ = app_handle.global_shortcut().unregister(shortcut);
            // 重新注册之前的主快捷键
            let current = HOTKEY_STRING.lock().unwrap().clone();
            if !current.is_empty() {
                let cur_norm = normalize_shortcut_string(&current);
                if let Ok(cur_sc) = cur_norm.parse::<Shortcut>() {
                    let _ = app_handle.global_shortcut().register(cur_sc);
                }
            }
            Ok(true)
        },
        Err(e) => {
            let err_str = format!("{:?}", e);
            crate::error!(">>> [DEBUG] Test register failed: {:?}", err_str);
            // 测试失败也要恢复之前的快捷键
            let current = HOTKEY_STRING.lock().unwrap().clone();
            if !current.is_empty() {
                let cur_norm = normalize_shortcut_string(&current);
                if let Ok(cur_sc) = cur_norm.parse::<Shortcut>() {
                    let _ = app_handle.global_shortcut().register(cur_sc);
                }
            }
            let user_msg = if err_str.contains("AlreadyRegistered") {
                "该快捷键已被其他程序占用".to_string()
            } else {
                "快捷键不可用".to_string()
            };
            Err(AppError::Internal(user_msg))
        }
    }
}
