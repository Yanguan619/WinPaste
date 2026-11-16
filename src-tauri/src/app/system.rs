use std::sync::atomic::Ordering;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::DefSubclassProc;
use tauri::Manager;
use crate::global_state::TASKBAR_CREATED_MSG;

/// 获取硬件机器码（基于硬件唯一标识）
/// 返回格式: 8字符的十六进制字符串 (例如: "ef785433")
pub fn get_machine_id() -> String {
    use sha2::{Sha256, Digest};
    
    match machine_uid::get() {
        Ok(machine_uid) => {
            let mut hasher = Sha256::new();
            hasher.update(machine_uid.as_bytes());
            let result = hasher.finalize();
            let hex = format!("{:x}", result);
            hex.chars().take(8).collect()
        }
        Err(e) => {
            crate::info!("[WARN] Failed to get machine UID: {}. Using fallback.", e);
            let mut hasher = Sha256::new();
            if let Ok(computer_name) = std::env::var("COMPUTERNAME") {
                hasher.update(computer_name.as_bytes());
            }
            if let Ok(username) = std::env::var("USERNAME") {
                hasher.update(username.as_bytes());
            }
            let result = hasher.finalize();
            let hex = format!("{:x}", result);
            hex.chars().take(8).collect()
        }
    }
}

pub fn build_anon_id(machine_id: &str) -> String {
    machine_id.to_string()
}

pub fn is_legacy_placeholder_anon_id(id: &str) -> bool {
    id.contains("-0000-0000-0000-000000000000")
}

pub fn normalize_anon_id(id: &str) -> Option<String> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(short) = trimmed.split('-').next() {
        if (short.len() == 8 || short.len() == 9) && short.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(short.to_string());
        }
    }
    None
}

pub fn is_same_device_id(id1: &str, id2: &str) -> bool {
    let n1 = normalize_anon_id(id1);
    let n2 = normalize_anon_id(id2);
    n1.is_some() && n1 == n2
}

// —— 面板合成表面失效的信号 ——
// 这些消息号/参数值只在 Windows 上用到。手写常量是为了不额外引入
// Win32_System_Power feature（PBT_* 定义在其中）。
#[cfg(target_os = "windows")]
const WM_POWERBROADCAST: u32 = 0x0218;
#[cfg(target_os = "windows")]
const WM_DWMCOMPOSITIONCHANGED: u32 = 0x031E;
#[cfg(target_os = "windows")]
const WM_DISPLAYCHANGE: u32 = 0x007E;
#[cfg(target_os = "windows")]
const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
#[cfg(target_os = "windows")]
const PBT_APMRESUMEAUTOMATIC: usize = 0x0012;
#[cfg(target_os = "windows")]
const PBT_APMRESUMESUSPEND: usize = 0x0007;
#[cfg(target_os = "windows")]
const WTS_SESSION_UNLOCK: usize = 0x0008;
/// WTSRegisterSessionNotification 唯一合法的标志值：NOTIFY_FOR_THIS_SESSION。
#[cfg(target_os = "windows")]
const NOTIFY_FOR_THIS_SESSION: u32 = 0;

// wtsapi32!WTSRegisterSessionNotification —— 不注册就收不到 WM_WTSSESSION_CHANGE。
// 会话通知是"按窗口"注册的，窗口重建后必须重新注册，否则解锁唤醒不再上报。
#[cfg(target_os = "windows")]
#[link(name = "wtsapi32")]
extern "system" {
    fn WTSRegisterSessionNotification(hwnd: HWND, dwflags: u32) -> i32;
}

/// Apply the message hooks the panel depends on to a window: the TaskbarCreated
/// subclass (explorer restart) plus WTS session notifications (lock/unlock wake
/// signals). Used both at startup and when the main window is rebuilt in
/// lightweight mode (a fresh HWND needs re-registering).
#[cfg(target_os = "windows")]
pub fn install_window_notifications(window: &tauri::WebviewWindow) {
    use windows::Win32::UI::Shell::SetWindowSubclass;
    if let Ok(hwnd) = window.hwnd() {
        let hwnd = HWND(hwnd.0);
        unsafe {
            let _ = SetWindowSubclass(hwnd, Some(tray_subclass_proc), 1337, 0);
            // 失败只是收不到解锁事件，其它恢复手段不受影响。
            let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
        }
    }
}

/// Window subclass procedure: taskbar recreation (explorer restart) + wake signals
#[cfg(target_os = "windows")]
pub unsafe extern "system" fn tray_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    // 唤醒/合成器重置：窗口层(IsWindowVisible、DWMWA_CLOAKED)完全正常，健康自检
    // 也照常回执，但 WebView2 的 DirectComposition 表面已经作废 —— 全透明窗口上
    // 的一块空表面在 DWM 里就是纯黑。这种"进程活、JS 活、表面死"无法就地修补，
    // 只能整窗重建，所以这里只打标记（真正的动作在
    // window_manager::mark_panel_surface_stale 里异步执行）。
    match msg {
        WM_POWERBROADCAST
            if wparam.0 == PBT_APMRESUMEAUTOMATIC || wparam.0 == PBT_APMRESUMESUSPEND =>
        {
            crate::app::window_manager::mark_panel_surface_stale("system resume");
        }
        WM_DWMCOMPOSITIONCHANGED => {
            crate::app::window_manager::mark_panel_surface_stale("DWM composition reset");
        }
        WM_DISPLAYCHANGE => {
            crate::app::window_manager::mark_panel_surface_stale("display topology change");
        }
        WM_WTSSESSION_CHANGE if wparam.0 == WTS_SESSION_UNLOCK => {
            crate::app::window_manager::mark_panel_surface_stale("session unlock");
        }
        _ => {}
    }

    let taskbar_msg = TASKBAR_CREATED_MSG.load(Ordering::Relaxed);
    if msg != 0 && msg == taskbar_msg {
        if let Some(app_handle) = crate::GLOBAL_APP_HANDLE.get() {
            let handle = app_handle.clone();
                std::thread::Builder::new()
                    .name("tray-explorer-restart".to_string())
                    .stack_size(1_048_576)
                    .spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(1500));
                        if let Some(settings) = handle.try_state::<crate::app_state::SettingsState>() {
                            if settings.hide_tray_icon.load(Ordering::Relaxed) {
                                if let Some(tray) = handle.tray_by_id("main_tray") {
                                    let _ = tray.set_visible(false);
                                    crate::info!(">>> [TRAY] Explorer restart detected, re-hiding tray icon per user setting.");
                                }
                            }
                        }
                    }).expect("spawn tray-explorer-restart thread");
        }
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}
