import { useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Dispatch, SetStateAction } from "react";
import type { DefaultAppsMap, InstalledAppOption } from "../../features/app/types";

interface UseAppBootstrapOptions {
  setDataPath: Dispatch<SetStateAction<string>>;
  setInstalledApps: Dispatch<SetStateAction<InstalledAppOption[]>>;
  setAutoStart: Dispatch<SetStateAction<boolean>>;
  setAutoStartAdmin: Dispatch<SetStateAction<boolean>>;
  setWinClipboardDisabled: Dispatch<SetStateAction<boolean>>;
  setDefaultApps: Dispatch<SetStateAction<DefaultAppsMap>>;
  setAppSearchEnabled: Dispatch<SetStateAction<boolean>>;
}

export const useAppBootstrap = ({
  setDataPath,
  setInstalledApps,
  setAutoStart,
  setAutoStartAdmin,
  setWinClipboardDisabled,
  setAppSearchEnabled
}: UseAppBootstrapOptions) => {
  /**
   * 检测 Everything 状态并扫描已安装应用（仅启动时执行一次）。
   * Rust 端已在启动时缓存检测结果，后续调用直接返回缓存值。
   */
  const refreshAppSearchStatus = useCallback(async () => {
    try {
      const enabled = await invoke<boolean>("refresh_app_search_status");
      setAppSearchEnabled(enabled);
      if (enabled) {
        // 重新扫描已安装应用
        const apps = await invoke<{ name: string; path: string }[]>("scan_installed_apps").catch(() => []);
        if (apps) {
          setInstalledApps(apps.map(app => ({ label: app.name, value: app.path })));
        }
      }
    } catch (err) {
      console.error("Failed to refresh app search status:", err);
    }
  }, [setAppSearchEnabled, setInstalledApps]);

  useEffect(() => {
    let disposed = false;

    // 后端在面板显示后会 ping 一次;渲染进程已死时不会回执,
    // 连续两次缺失即触发 webview 重载自愈(见 window_manager.rs)。
    let unlistenHealth: (() => void) | null = null;
    listen<number>("panel-health-ping", (event) => {
      invoke("panel_health_ack", { token: event.payload }).catch(() => {});
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenHealth = unlisten;
    }).catch(() => {});

    invoke<string>("get_data_path").then((path) => {
      if (!disposed) setDataPath(path);
    }).catch(console.error);

    // 初始检查
    refreshAppSearchStatus();

    invoke<boolean>("is_autostart_enabled").then((enabled) => {
      if (!disposed) setAutoStart(enabled);
    }).catch(console.error);

    invoke<boolean>("is_autostart_admin_enabled").then((enabled) => {
      if (!disposed) setAutoStartAdmin(enabled);
    }).catch(console.error);

    invoke<boolean>("get_windows_clipboard_history").then((enabled) => {
      if (!disposed) setWinClipboardDisabled(!enabled);
    }).catch(console.error);

    return () => {
      disposed = true;
      unlistenHealth?.();
    };
  }, [
    setDataPath,
    setAutoStart,
    setAutoStartAdmin,
    setWinClipboardDisabled,
    refreshAppSearchStatus
  ]);

  return { refreshAppSearchStatus };
};
