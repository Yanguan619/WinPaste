import { useEffect, useRef } from "react";

interface UseTagManagerRefreshOptions {
  showTagManager: boolean;
  settingsLoaded: boolean;
  persistentLimitEnabled: boolean;
  persistentLimit: number;
  fetchHistory: (reset?: boolean) => void;
}

export const useTagManagerRefresh = ({
  showTagManager,
  settingsLoaded,
  persistentLimitEnabled,
  persistentLimit,
  fetchHistory
}: UseTagManagerRefreshOptions) => {
  // 首次（settingsLoaded 刚为 true）那一次由 App 的 settingsLoaded 拉取负责，这里只处理后续变化。
  const mounted = useRef(false);

  useEffect(() => {
    if (!settingsLoaded) return;
    if (!mounted.current) {
      mounted.current = true;
      return;
    }
    if (!showTagManager) {
      fetchHistory(true);
    }
  }, [showTagManager, settingsLoaded, persistentLimitEnabled, persistentLimit, fetchHistory]);
};
