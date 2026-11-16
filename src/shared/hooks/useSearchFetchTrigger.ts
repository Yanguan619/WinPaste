import { useEffect, useRef } from "react";

interface UseSearchFetchTriggerOptions {
  debouncedSearch: string;
  isComposing: boolean;
  typeFilter?: string | null;
  fetchHistory: (reset?: boolean) => void;
}

export const useSearchFetchTrigger = ({
  debouncedSearch,
  isComposing,
  typeFilter,
  fetchHistory
}: UseSearchFetchTriggerOptions) => {
  // 挂载那次由 App 的首屏拉取负责；这里只管后续变化，否则一次开面板会重复拉 2 次全表。
  const mounted = useRef(false);
  const typeMounted = useRef(false);

  useEffect(() => {
    if (!mounted.current) {
      mounted.current = true;
      return;
    }
    if (!isComposing) {
      fetchHistory(true);
    }
  }, [debouncedSearch, isComposing, fetchHistory]);

  useEffect(() => {
    if (!typeMounted.current) {
      typeMounted.current = true;
      return;
    }
    fetchHistory(true);
  }, [typeFilter, fetchHistory]);
};
