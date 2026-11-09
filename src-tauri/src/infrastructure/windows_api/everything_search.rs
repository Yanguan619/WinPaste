//! Everything 搜索模块
//!
//! 通过 Everything 官方命令行工具 `es.exe` 实现文件搜索。
//! 同时保留 IPC 方式作为备用（如果 es.exe 不存在时）。

use std::process::Command;
use std::os::windows::process::CommandExt;
use std::sync::{Mutex, OnceLock};

use crate::error::{AppError, AppResult};

#[cfg(feature = "app-search")]
use super::apps::AppInfo;

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileSearchResult {
    pub name: String,
    pub path: String,
    pub full_path: String,
    pub is_folder: bool,
}

// ============ Everything 进程检测 ============

/// Everything 运行状态：程序启动时检测一次，后续不再变化。
/// 窗口显示/隐藏不会触发重新检测，避免每次产生 tasklist.exe / powershell.exe / reg.exe 子进程。
static EVERYTHING_RUNNING: OnceLock<bool> = OnceLock::new();

static ES_PATH: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static EVERYTHING_PATH: OnceLock<Mutex<Option<String>>> = OnceLock::new();

/// 在启动时调用一次，缓存 Everything 运行状态。
/// 后续 is_everything_running() 直接返回缓存值。
pub fn init_everything_status() {
    EVERYTHING_RUNNING.get_or_init(|| check_everything_running_raw());
}

/// 清除路径缓存（用于运行时 Everything 启动后强制重新扫描目录）
pub fn clear_path_cache() {
    if let Some(lock) = ES_PATH.get() {
        if let Ok(mut guard) = lock.lock() {
            *guard = None;
        }
    }
    if let Some(lock) = EVERYTHING_PATH.get() {
        if let Ok(mut guard) = lock.lock() {
            *guard = None;
        }
    }
}

/// 返回启动时缓存的 Everything 运行状态（仅检测一次，不重复生成子进程）
pub fn is_everything_running() -> bool {
    *EVERYTHING_RUNNING.get_or_init(|| check_everything_running_raw())
}

fn check_everything_running_raw() -> bool {
    if let Ok(output) = Command::new("tasklist")
        .creation_flags(0x08000000)
        .args(["/FI", "IMAGENAME eq Everything.exe"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        return stdout.contains("Everything.exe") && !stdout.contains("No tasks are running");
    }
    false
}

/// 获取 Everything 安装目录
fn get_everything_dir() -> Option<String> {
    {
        let lock = EVERYTHING_PATH.get_or_init(|| Mutex::new(None));
        if let Ok(guard) = lock.lock() {
            if let Some(ref path) = *guard {
                if std::path::Path::new(path).exists() {
                    return Some(path.clone());
                }
            }
        }
    }

    let path = detect_everything_dir();

    if let Ok(mut guard) = EVERYTHING_PATH.get_or_init(|| Mutex::new(None)).lock() {
        *guard = path.clone();
    }

    path
}

fn detect_everything_dir() -> Option<String> {
    // 1. 从 PowerShell Get-Process 路径获取
    if let Ok(output) = Command::new("powershell")
        .creation_flags(0x08000000)
        .args([
            "-NoProfile",
            "-Command",
            "Get-Process Everything -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Path",
        ])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !stdout.is_empty() && std::path::Path::new(&stdout).exists() {
                if let Some(parent) = std::path::Path::new(&stdout).parent() {
                    return Some(parent.to_string_lossy().to_string());
                }
            }
        }
    }

    // 2. 通过 wmic 获取 Everything 可执行文件路径
    if let Ok(output) = Command::new("wmic")
        .creation_flags(0x08000000)
        .args([
            "process",
            "where",
            "name='Everything.exe'",
            "get",
            "ExecutablePath",
            "/format:list",
        ])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if let Some(val) = trimmed.strip_prefix("ExecutablePath=") {
                    let dir = val.trim();
                    if !dir.is_empty() {
                        if let Some(parent) = std::path::Path::new(dir).parent() {
                            return Some(parent.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }

    // 3. 从注册表查找
    if let Ok(output) = Command::new("reg")
        .creation_flags(0x08000000)
        .args([
            "query",
            "HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
            "/s",
            "/f",
            "Everything",
        ])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Some(val) = line.trim().strip_prefix("InstallLocation    REG_SZ    ") {
                    let dir = val.trim();
                    if !dir.is_empty() && std::path::Path::new(dir).exists() {
                        return Some(dir.to_string());
                    }
                }
            }
        }
    }

    None
}

/// 查找 es.exe 路径（带缓存）
fn find_es_path() -> Option<String> {
    {
        let lock = ES_PATH.get_or_init(|| Mutex::new(None));
        if let Ok(guard) = lock.lock() {
            if let Some(ref path) = *guard {
                if std::path::Path::new(path).exists() {
                    return Some(path.clone());
                }
            }
        }
    }

    let path = detect_es_path();

    if let Ok(mut guard) = ES_PATH.get_or_init(|| Mutex::new(None)).lock() {
        *guard = path.clone();
    }

    path
}

fn detect_es_path() -> Option<String> {
    // 0. 从 Everything 安装目录查找
    if let Some(ev_dir) = get_everything_dir() {
        let es_path = std::path::Path::new(&ev_dir).join("es.exe");
        if es_path.exists() {
            return Some(es_path.to_string_lossy().to_string());
        }
    }

    // 1. 从 PATH 查找
    if let Ok(output) = Command::new("where.exe").creation_flags(0x08000000).arg("es").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    let p = if trimmed.ends_with(".exe") {
                        trimmed.to_string()
                    } else {
                        format!("{}.exe", trimmed)
                    };
                    if std::path::Path::new(&p).exists() {
                        return Some(p);
                    }
                }
            }
        }
    }

    // 2. 检查常见安装路径
    let common_paths = [
        "C:\\Program Files\\Everything\\es.exe",
        "C:\\Program Files (x86)\\Everything\\es.exe",
        "D:\\Program Files\\Everything\\es.exe",
        "D:\\Program Files (x86)\\Everything\\es.exe",
        "E:\\Program Files\\Everything\\es.exe",
        "E:\\Program Files (protable)\\Everything\\es.exe",
    ];

    for p in &common_paths {
        if std::path::Path::new(p).exists() {
            return Some(p.to_string());
        }
    }

    None
}

// ============ es.exe 搜索实现 ============

/// 使用 es.exe 搜索文件
fn search_via_es(keyword: &str, max_results: u32) -> AppResult<Vec<String>> {
    let es_path = find_es_path().ok_or_else(|| {
        AppError::Internal("es.exe not found".to_string())
    })?;

    let output = Command::new(&es_path)
        .creation_flags(0x08000000)
        .arg("-n")
        .arg(max_results.to_string())
        .arg("-s")
        .arg("-i")
        .arg(keyword)
        .output()
        .map_err(|e| AppError::Internal(format!("Failed to execute es.exe: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.code() == Some(1) && stderr.is_empty() {
            return Ok(Vec::new());
        }
        return Err(AppError::Internal(format!(
            "es.exe failed: {} (code: {:?})",
            stderr.trim(),
            output.status.code()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let paths: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    Ok(paths)
}

// ============ Everything HTTP 服务器搜索 ============

/// 检测 Everything HTTP 服务器端口（读取 Everything.ini 中的 http_server_port）
fn find_http_port() -> Option<u16> {
    let ev_dir = get_everything_dir()?;
    let ini_path = std::path::Path::new(&ev_dir).join("Everything.ini");
    if !ini_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&ini_path).ok()?;

    let mut port: Option<u16> = None;
    let mut enabled = false;
    let mut allow = true;

    for line in content.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("http_server_port=") {
            if let Ok(p) = val.trim().parse::<u16>() {
                if p > 0 {
                    port = Some(p);
                }
            }
        } else if let Some(val) = line.strip_prefix("http_server_enabled=") {
            enabled = val.trim() == "1";
        } else if let Some(val) = line.strip_prefix("allow_http_server=") {
            allow = val.trim() == "1";
        }
    }

    if !enabled || !allow {
        return None;
    }
    port.or(Some(80))
}

/// 通过 Everything HTTP 服务器搜索
fn search_via_http(keyword: &str, max_results: u32) -> AppResult<Vec<String>> {
    let port = find_http_port().ok_or_else(|| {
        AppError::Internal("HTTP server not enabled".to_string())
    })?;

    let encoded = url_encode(keyword);
    let url = format!(
        "http://localhost:{}/?search={}&count={}",
        port, encoded, max_results
    );

    // 优先用 curl.exe（Windows 10+ 自带），失败则回退 PowerShell
    let output = Command::new("curl.exe")
        .creation_flags(0x08000000)
        .arg("-s")
        .arg("--max-time")
        .arg("5")
        .arg(&url)
        .output()
        .or_else(|_| {
            Command::new("powershell")
                .creation_flags(0x08000000)
                .args([
                    "-NoProfile",
                    "-Command",
                    &format!(
                        "try {{ (Invoke-WebRequest -Uri '{}' -UseBasicParsing -TimeoutSec 5).Content }} catch {{ '' }}",
                        url
                    ),
                ])
                .output()
        })
        .map_err(|e| AppError::Internal(format!("HTTP request failed: {}", e)))?;

    if !output.status.success() {
        return Err(AppError::Internal("HTTP request failed".to_string()));
    }

    let body = String::from_utf8_lossy(&output.stdout);
    parse_http_response(&body)
}

fn url_encode(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' || c == ':' || c == '*' || c == '?' {
            result.push(c);
        } else if c == ' ' {
            result.push('+');
        } else {
            result.push_str(&format!("%{:02X}", c as u32));
        }
    }
    result
}

/// 解析 Everything HTTP 服务器响应（HTML 格式）
/// Everything 将每个完整路径放在 `<a href="/X%3A/.../filename">` 中
fn parse_http_response(body: &str) -> AppResult<Vec<String>> {
    let mut paths = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 查找所有 <a href="/X%3A/..."> 形式的链接
    let mut rest = body;
    while let Some(start) = rest.find("<a href=\"/") {
        rest = &rest[start + 9..];
        if let Some(end) = rest.find('"') {
            let href = &rest[..end];
            rest = &rest[end..];

            // 必须是带盘符的路径: /C%3A/..., /D%3A/...
            if href.len() > 5 && href.as_bytes()[2] == b'%' {
                // 解码 URL 编码
                let decoded = url_decode(href);
                // 去掉开头的 '/'，再转换为 Windows 路径分隔符
                let win_path = decoded.trim_start_matches('/').replace('/', "\\");
                if !seen.contains(&win_path) {
                    seen.insert(win_path.clone());
                    paths.push(win_path);
                }
            }
        }
    }

    Ok(paths)
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_digit(bytes[i + 1]), hex_digit(bytes[i + 2])) {
                result.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            result.push(b' ');
        } else {
            result.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ============ 多后端搜索调度 ============

fn search_with_fallback(keyword: &str, max_results: u32) -> AppResult<Vec<String>> {
    // 1. 优先 HTTP 服务器（兼容性最好，不依赖额外可执行文件）
    if let Ok(paths) = search_via_http(keyword, max_results) {
        if !paths.is_empty() {
            // HTTP 返回可能包含文件+父目录，超出 max_results 时截断
            return Ok(paths.into_iter().take(max_results as usize).collect());
        }
    }

    // 2. 尝试 es.exe
    if let Ok(paths) = search_via_es(keyword, max_results) {
        return Ok(paths);
    }

    // 3. 尝试 IPC 备用
    if let Ok(paths) = ipc_backup::search_via_ipc(keyword, max_results) {
        return Ok(paths);
    }

    Ok(Vec::new())
}

/// 解析 es.exe 返回的路径列表为 FileSearchResult
fn parse_paths_to_results(paths: &[String]) -> Vec<FileSearchResult> {
    paths.iter().map(|full_path| {
        let path_str = full_path.replace('/', "\\");
        let path_obj = std::path::Path::new(&path_str);

        let name = path_obj
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let parent = path_obj
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string();

        // 使用 is_dir() 判断，但如果路径不存在则回退到检查文件扩展名
        let is_folder = path_obj.is_dir() || {
            // 如果没有文件扩展名，很可能是文件夹
            name.is_empty() || !name.contains('.')
        };

        FileSearchResult {
            name,
            path: parent,
            full_path: path_str,
            is_folder,
        }
    }).collect()
}

/// 解析 es.exe 返回的路径列表为 AppInfo
#[cfg(feature = "app-search")]
fn parse_paths_to_apps(paths: &[String]) -> Vec<AppInfo> {
    paths.iter().map(|full_path| {
        let path_str = full_path.replace('/', "\\");
        let path_obj = std::path::Path::new(&path_str);

        let name = path_obj
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        AppInfo {
            name,
            path: path_str,
        }
    }).collect()
}

// ============ 公共 API ============

#[cfg(feature = "app-search")]
/// 使用 Everything 搜索可执行文件
pub fn search_apps_via_everything(keyword: &str, max_results: u32) -> AppResult<Vec<AppInfo>> {
    if !is_everything_running() {
        return Ok(Vec::new());
    }

    let search_query = if keyword.trim().is_empty() {
        "ext:exe".to_string()
    } else {
        format!("{} ext:exe", keyword)
    };

    let paths = search_with_fallback(&search_query, max_results)?;
    Ok(parse_paths_to_apps(&paths))
}

/// 使用 Everything 搜索文件（通用搜索，不限类型）
pub fn search_files(keyword: &str, max_results: u32) -> AppResult<Vec<FileSearchResult>> {
    if !is_everything_running() {
        return Ok(Vec::new());
    }

    let paths = search_with_fallback(keyword, max_results)?;
    Ok(parse_paths_to_results(&paths))
}

// ============ 备用 IPC 实现（当 es.exe 不可用时） ============

#[allow(dead_code)]
mod ipc_backup {
    use super::*;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::DataExchange::COPYDATASTRUCT;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowW,
        RegisterClassExW, SendMessageW, MSG, WM_COPYDATA, WM_QUIT, WNDCLASSEXW,
    };
    use windows::core::{PCWSTR, w};

    static IPC_QUERY_RESULT: OnceLock<Mutex<Option<Vec<u8>>>> = OnceLock::new();

    const EVERYTHING_IPC_WNDCLASS: PCWSTR = w!("EVERYTHING_IPC_WNDCLASS");
    const EVERYTHING_IPC_WM_QUERY: u32 = 0x0040;
    const EVERYTHING_IPC_QUERY2_VERSION: u32 = 2;
    const EVERYTHING_IPC_QUERY2_REQUEST_NAME: u32 = 0x00000001;
    const EVERYTHING_IPC_QUERY2_REQUEST_PATH: u32 = 0x00000002;
    const EVERYTHING_IPC_QUERY2_REQUEST_FULL_PATH_AND_NAME: u32 = 0x00000004;
    const EVERYTHING_IPC_ITEM_FLAG_FOLDER: u32 = 0x00000001;

    #[repr(C, packed)]
    #[derive(Debug, Copy, Clone)]
    struct IpcQuery2 {
        version: u32,
        request_flags: u32,
        search_flags: u32,
        sort_type: u32,
        offset: u32,
        max_results: u32,
    }

    #[repr(C, packed)]
    #[derive(Debug, Copy, Clone)]
    struct IpcList2 {
        num_items: u32,
        tot_items: u32,
        offset: u32,
        request_flags: u32,
        sort_type: u32,
    }

    #[repr(C, packed)]
    #[derive(Debug, Copy, Clone)]
    struct IpcItem2 {
        flags: u32,
        data_offset: u32,
    }

    #[allow(dead_code)]
    pub fn search_via_ipc(keyword: &str, max_results: u32) -> AppResult<Vec<String>> {
        if !is_everything_running_ipc() {
            return Err(AppError::Internal("IPC window not found".to_string()));
        }
        let results = unsafe { run_ipc_query::<FileSearchResult>(keyword, max_results, false)? };
        Ok(results.into_iter().map(|r| r.full_path).collect())
    }
    #[allow(dead_code)]
    pub fn is_everything_running_ipc() -> bool {
        unsafe {
            match FindWindowW(EVERYTHING_IPC_WNDCLASS, PCWSTR::null()) {
                Ok(hwnd) => !hwnd.0.is_null(),
                Err(_) => false,
            }
        }
    }

    #[cfg(feature = "app-search")]
    #[allow(dead_code)]
    pub unsafe fn search_apps_ipc(keyword: &str, max_results: u32) -> AppResult<Vec<AppInfo>> {
        if !is_everything_running_ipc() {
            return Ok(Vec::new());
        }
        let query = if keyword.trim().is_empty() {
            "ext:exe".to_string()
        } else {
            format!("{} ext:exe", keyword)
        };
        run_ipc_query(&query, max_results, true)
    }

    #[allow(dead_code)]
    pub unsafe fn search_files_ipc(keyword: &str, max_results: u32) -> AppResult<Vec<FileSearchResult>> {
        if !is_everything_running_ipc() {
            return Ok(Vec::new());
        }
        run_ipc_query(keyword, max_results, false)
    }

    unsafe fn run_ipc_query<T: IpcParseTarget>(search: &str, max_results: u32, _is_app: bool) -> AppResult<Vec<T>> {
        let everything_hwnd = FindWindowW(EVERYTHING_IPC_WNDCLASS, PCWSTR::null())
            .map_err(|e| AppError::Internal(format!("FindWindowW failed: {}", e)))?;
        if everything_hwnd.0.is_null() {
            return Ok(Vec::new());
        }

        let class_name = w!("WinPasteEverythingReply");
        let mut wnd_class = WNDCLASSEXW::default();
        wnd_class.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        wnd_class.lpfnWndProc = Some(everything_reply_wnd_proc);
        wnd_class.lpszClassName = class_name;

        let h_instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(PCWSTR::null())
            .map_err(|e| AppError::Internal(format!("GetModuleHandleW failed: {}", e)))?;

        RegisterClassExW(&wnd_class);

        let reply_hwnd = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            class_name,
            w!(""),
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(0),
            0, 0, 0, 0,
            Some(HWND::default()),
            None,
            Some(h_instance.into()),
            None,
        )
        .map_err(|e| AppError::Internal(format!("CreateWindowExW failed: {}", e)))?;

        if reply_hwnd.0.is_null() {
            return Err(AppError::Internal("Failed to create reply window".to_string()));
        }

        let query = IpcQuery2 {
            version: EVERYTHING_IPC_QUERY2_VERSION,
            request_flags: EVERYTHING_IPC_QUERY2_REQUEST_NAME
                | EVERYTHING_IPC_QUERY2_REQUEST_PATH
                | EVERYTHING_IPC_QUERY2_REQUEST_FULL_PATH_AND_NAME,
            search_flags: 0,
            sort_type: 14,
            offset: 0,
            max_results,
        };

        let search_wide: Vec<u16> = search
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut query_data = Vec::new();
        query_data.extend_from_slice(std::slice::from_raw_parts(
            &query as *const _ as *const u8,
            std::mem::size_of::<IpcQuery2>(),
        ));
        query_data.extend_from_slice(std::slice::from_raw_parts(
            search_wide.as_ptr() as *const u8,
            search_wide.len() * 2,
        ));

        let copydata = COPYDATASTRUCT {
            dwData: EVERYTHING_IPC_WM_QUERY as usize,
            cbData: query_data.len() as u32,
            lpData: query_data.as_ptr() as *mut std::ffi::c_void,
        };

        let _ = SendMessageW(
            everything_hwnd,
            WM_COPYDATA,
            Some(WPARAM(reply_hwnd.0 as usize)),
            Some(LPARAM(&copydata as *const _ as isize)),
        );

        let result_data = run_message_loop_with_timeout(reply_hwnd, 5000);
        let _ = DestroyWindow(reply_hwnd);

        let Some(data) = result_data else {
            return Ok(Vec::new());
        };

        T::parse(&data)
    }

    trait IpcParseTarget: Sized {
        fn parse(data: &[u8]) -> AppResult<Vec<Self>>;
    }

    #[cfg(feature = "app-search")]
    impl IpcParseTarget for AppInfo {
        fn parse(data: &[u8]) -> AppResult<Vec<AppInfo>> {
            if data.len() < std::mem::size_of::<IpcList2>() {
                return Ok(Vec::new());
            }
            unsafe {
                let list: &IpcList2 = &*(data.as_ptr() as *const IpcList2);
                let num_items = list.num_items as usize;
                if num_items == 0 {
                    return Ok(Vec::new());
                }
                let mut apps = Vec::with_capacity(num_items);
                let mut offset = std::mem::size_of::<IpcList2>();
                for _ in 0..num_items {
                    if offset + std::mem::size_of::<IpcItem2>() > data.len() {
                        break;
                    }
                    let item: &IpcItem2 = &*(data.as_ptr().add(offset) as *const IpcItem2);
                    offset += std::mem::size_of::<IpcItem2>();
                    let mut data_offset = std::mem::size_of::<IpcList2>() + item.data_offset as usize;
                    let name = read_wstring_from_data(data, &mut data_offset)?;
                    let _path = read_wstring_from_data(data, &mut data_offset)?;
                    let full_path = read_wstring_from_data(data, &mut data_offset)?;
                    if !full_path.is_empty() {
                        let app_name = if name.is_empty() {
                            std::path::Path::new(&full_path)
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or(&full_path)
                                .to_string()
                        } else {
                            name
                        };
                        apps.push(AppInfo {
                            name: app_name,
                            path: full_path,
                        });
                    }
                }
                Ok(apps)
            }
        }
    }

    impl IpcParseTarget for FileSearchResult {
        fn parse(data: &[u8]) -> AppResult<Vec<FileSearchResult>> {
            if data.len() < std::mem::size_of::<IpcList2>() {
                return Ok(Vec::new());
            }
            unsafe {
                let list: &IpcList2 = &*(data.as_ptr() as *const IpcList2);
                let num_items = list.num_items as usize;
                if num_items == 0 {
                    return Ok(Vec::new());
                }
                let mut results = Vec::with_capacity(num_items);
                let mut offset = std::mem::size_of::<IpcList2>();
                for _ in 0..num_items {
                    if offset + std::mem::size_of::<IpcItem2>() > data.len() {
                        break;
                    }
                    let item: &IpcItem2 = &*(data.as_ptr().add(offset) as *const IpcItem2);
                    offset += std::mem::size_of::<IpcItem2>();
                    let is_folder = (item.flags & EVERYTHING_IPC_ITEM_FLAG_FOLDER) != 0;
                    let mut data_offset = std::mem::size_of::<IpcList2>() + item.data_offset as usize;
                    let name = read_wstring_from_data(data, &mut data_offset)?;
                    let path = read_wstring_from_data(data, &mut data_offset)?;
                    let full_path = read_wstring_from_data(data, &mut data_offset)?;
                    if !full_path.is_empty() {
                        results.push(FileSearchResult {
                            name,
                            path,
                            full_path,
                            is_folder,
                        });
                    }
                }
                Ok(results)
            }
        }
    }

    unsafe fn run_message_loop_with_timeout(hwnd: HWND, timeout_ms: u32) -> Option<Vec<u8>> {
        let start = std::time::Instant::now();
        let result_storage = IPC_QUERY_RESULT.get_or_init(|| Mutex::new(None));
        *result_storage.lock().unwrap() = None;
        let mut msg = MSG::default();
        loop {
            if let Ok(guard) = result_storage.lock() {
                if guard.is_some() {
                    return guard.clone();
                }
            }
            if start.elapsed().as_millis() as u32 > timeout_ms {
                break;
            }
            let ret = windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
                &mut msg,
                Some(hwnd),
                0,
                0,
                windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
            );
            if ret.as_bool() {
                if msg.message == WM_QUIT {
                    break;
                }
                let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
            } else {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        result_storage.lock().unwrap().clone()
    }

    unsafe extern "system" fn everything_reply_wnd_proc(
        _hwnd: HWND,
        msg: u32,
        _wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_COPYDATA {
            let copydata = lparam.0 as *const COPYDATASTRUCT;
            if !copydata.is_null() {
                let cds = &*copydata;
                let data_slice = std::slice::from_raw_parts(
                    cds.lpData as *const u8,
                    cds.cbData as usize,
                );
                if let Ok(mut guard) = IPC_QUERY_RESULT.get_or_init(|| Mutex::new(None)).lock() {
                    *guard = Some(data_slice.to_vec());
                }
                return LRESULT(1);
            }
        }
        DefWindowProcW(_hwnd, msg, _wparam, lparam)
    }

    unsafe fn read_wstring_from_data(data: &[u8], offset: &mut usize) -> AppResult<String> {
        if *offset + 4 > data.len() {
            return Ok(String::new());
        }
        let len = u32::from_le_bytes([
            data[*offset],
            data[*offset + 1],
            data[*offset + 2],
            data[*offset + 3],
        ]) as usize;
        *offset += 4;
        if len == 0 {
            return Ok(String::new());
        }
        if *offset + len * 2 > data.len() {
            return Ok(String::new());
        }
        let wide_slice = std::slice::from_raw_parts(
            data.as_ptr().add(*offset) as *const u16,
            len,
        );
        *offset += len * 2;
        let actual_len = wide_slice.iter().position(|&c| c == 0).unwrap_or(len);
        Ok(String::from_utf16_lossy(&wide_slice[..actual_len]))
    }
}

// ============ 测试 ============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_es_path() {
        let path = find_es_path();
        println!("es.exe path: {:?}", path);
    }

    #[test]
    fn test_is_everything_running() {
        let running = is_everything_running();
        println!("Everything running: {}", running);
    }

    #[test]
    fn test_search_files_empty_keyword() {
        if !is_everything_running() {
            println!("Skipped: Everything not running");
            return;
        }
        let result = search_files("", 5);
        assert!(result.is_ok());
        let files = result.unwrap();
        println!("Found {} files with empty keyword", files.len());
    }

    #[test]
    fn test_search_files_notepad() {
        if !is_everything_running() {
            println!("Skipped: Everything not running");
            return;
        }
        let result = search_files("notepad", 10);
        assert!(result.is_ok());
        let files = result.unwrap();
        println!("Found {} files for 'notepad'", files.len());
        if !files.is_empty() {
            assert!(!files[0].full_path.is_empty());
            assert!(!files[0].name.is_empty());
        }
    }

    #[test]
    fn test_search_files_extension_syntax() {
        if !is_everything_running() {
            println!("Skipped: Everything not running");
            return;
        }
        let result = search_files("ext:txt", 10);
        assert!(result.is_ok());
        let files = result.unwrap();
        println!("Found {} .txt files", files.len());
    }

    #[cfg(feature = "app-search")]
    #[test]
    fn test_search_apps() {
        if !is_everything_running() {
            println!("Skipped: Everything not running");
            return;
        }
        let result = search_apps_via_everything("", 10);
        assert!(result.is_ok());
        let apps = result.unwrap();
        println!("Found {} apps", apps.len());
        if !apps.is_empty() {
            assert!(!apps[0].name.is_empty());
            assert!(!apps[0].path.is_empty());
        }
    }

    #[cfg(feature = "app-search")]
    #[test]
    fn test_search_apps_with_keyword() {
        if !is_everything_running() {
            println!("Skipped: Everything not running");
            return;
        }
        let result = search_apps_via_everything("note", 10);
        assert!(result.is_ok());
        let apps = result.unwrap();
        println!("Found {} apps for 'note'", apps.len());
    }

    #[test]
    fn test_search_files_result_structure() {
        if !is_everything_running() {
            println!("Skipped: Everything not running");
            return;
        }
        let result = search_files("notepad", 5).unwrap();
        if !result.is_empty() {
            let file = &result[0];
            assert!(!file.full_path.is_empty(), "full_path should not be empty");
            assert!(!file.name.is_empty(), "name should not be empty");
            // full_path should contain the name
            assert!(
                file.full_path.contains(&file.name),
                "full_path '{}' should contain name '{}'",
                file.full_path,
                file.name
            );
        }
    }

    #[test]
    fn test_search_max_results() {
        if !is_everything_running() {
            println!("Skipped: Everything not running");
            return;
        }
        let result = search_files("file", 3);
        assert!(result.is_ok());
        let files = result.unwrap();
        assert!(files.len() <= 3, "Should not exceed max_results");
    }

    #[test]
    fn test_parse_paths_to_results_basic() {
        let paths = vec![
            "C:\\Windows\\System32\\notepad.exe".to_string(),
            "C:\\Users\\test\\Documents".to_string(),
        ];
        let results = parse_paths_to_results(&paths);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "notepad.exe");
        assert_eq!(results[0].path, "C:\\Windows\\System32");
        assert_eq!(results[0].is_folder, false);
    }

    #[cfg(feature = "app-search")]
    #[test]
    fn test_parse_paths_to_apps_basic() {
        let paths = vec![
            "C:\\Windows\\System32\\notepad.exe".to_string(),
        ];
        let apps = parse_paths_to_apps(&paths);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "notepad");
        assert_eq!(apps[0].path, "C:\\Windows\\System32\\notepad.exe");
    }

    #[test]
    fn test_parse_paths_empty() {
        let results = parse_paths_to_results(&[]);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_search_with_special_chars() {
        if !is_everything_running() {
            println!("Skipped: Everything not running");
            return;
        }
        // Everything 支持特殊字符
        let result = search_files("*.txt", 5);
        assert!(result.is_ok());
        let files = result.unwrap();
        println!("Found {} files for '*.txt'", files.len());
    }

    #[test]
    fn test_not_running_returns_empty() {
        // 当 Everything 未运行时应返回空列表
        if is_everything_running() {
            println!("Skipped: Everything is running");
            return;
        }
        let result = search_files("test", 10);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_url_decode() {
        assert_eq!(url_decode("D%3A"), "D:");
        assert_eq!(url_decode("D%3A%2FProgram%20Files"), "D:/Program Files");
        assert_eq!(url_decode("hello+world"), "hello world");
        assert_eq!(url_decode("nothing"), "nothing");
    }

    #[test]
    fn test_parse_http_response() {
        let html = r#"<a href="/D%3A/Program%20Files/test.exe">link</a>"#;
        let paths = parse_http_response(html).unwrap();
        assert_eq!(paths, vec!["D:\\Program Files\\test.exe"]);

        // 多个链接
        let html2 = r#"
            <a href="/D%3A/Program%20Files/a.txt">a</a>
            <a href="/D%3A/Program%20Files/b.txt">b</a>
            <a href="/?search=foo">not a path</a>
        "#;
        let paths2 = parse_http_response(html2).unwrap();
        assert_eq!(paths2.len(), 2);
        assert!(paths2.contains(&"D:\\Program Files\\a.txt".to_string()));
        assert!(paths2.contains(&"D:\\Program Files\\b.txt".to_string()));
    }

    #[test]
    fn test_find_http_port() {
        let port = find_http_port();
        println!("HTTP port: {:?}", port);
        // 仅作打印，无断言
    }

    #[test]
    fn test_search_via_http() {
        if !is_everything_running() {
            println!("Skipped: Everything not running");
            return;
        }
        let result = search_via_http("notepad", 5);
        if let Ok(paths) = result {
            println!("HTTP returned {} paths for 'notepad':", paths.len());
            for p in &paths {
                println!("  {}", p);
            }
        } else {
            println!("HTTP search failed: {:?}", result.err());
        }
    }
}
