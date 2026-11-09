import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { RefObject, ReactNode } from "react";
import { motion, Reorder, useDragControls, AnimatePresence } from "framer-motion";
import type { DragControls } from "framer-motion";
import { ArrowUp, Clipboard, FileSearch, FileText, Folder, Grid3X3, Search, X } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import type { StickyEntry } from "../../../shared/types/sticky";
import { StickyManager } from "../../sticky/StickyManager";
import type { SettingsPanelProps } from "../../settings/components/SettingsPanel";
import { VirtualList } from "../../clipboard/components/VirtualClipboardList";
import type { ClipboardEntry } from "../../../shared/types";
import type { VirtualClipboardListHandle } from "../../clipboard/types";
import { useSettingsStore } from "../../../shared/store/settingsStore";
import { useHistoryStore } from "../../../shared/store/historyStore";
import { useUIStore } from "../../../shared/store/uiStore";

export interface AppListItem {
  name: string;
  path: string;
  icon?: string;
}

export interface FileSearchItem {
  name: string;
  path: string;
  full_path: string;
  is_folder: boolean;
}

// Settings and tag manager are heavy (react-select, tag UI) and only rendered
// when the user opens them — keep them out of the startup chunk. Both read CSS
// variables set by the app shell, so they render fine after the chunk loads.
const SettingsPanel = lazy(() => import("../../settings/components/SettingsPanel"));
const TagManager = lazy(() => import("../../tag/components/TagManager"));

type RenderItem = (
  item: ClipboardEntry,
  index: number,
  dragControls?: DragControls,
  disableLayout?: boolean
) => ReactNode;

interface AppMainContentProps {
  t: (key: string) => string;
  settingsPanelProps: SettingsPanelProps;
  filteredHistory: ClipboardEntry[];
  pinnedItems: ClipboardEntry[];
  unpinnedItems: ClipboardEntry[];
  virtualListRef: RefObject<VirtualClipboardListHandle | null>;
  handlePinnedReorder: (newOrderIds: number[]) => void;
  renderItemContent: RenderItem;
  loadMoreHistory: () => void;
  handleListScroll: (offset: number) => void;
  showScrollTop: boolean;
  onScrollTop: () => void;
  stickyEntries?: StickyEntry[];
  onStickyRemoved?: () => void;
  stickyEnabled?: boolean;
  searchMode: 'clipboard' | 'apps' | 'files';
  filteredApps: AppListItem[];
  filteredFiles: FileSearchItem[];
  onLaunchApp: (path: string) => void;
  onOpenFile: (path: string) => void;
  onLaunchBrowserSearch: (query: string) => void;
  filesLoading: boolean;
  appsLoading?: boolean;
  onAppsListScroll: (offset: number) => void;
  onFilesListScroll: (offset: number) => void;
  onAppsScrollTop: () => void;
  onFilesScrollTop: () => void;
  showAppsScrollTop: boolean;
  showFilesScrollTop: boolean;
  itemCount?: number;
}

const SortableItem = ({
  item,
  index,
  renderItem,
  isFirst,
  onDragStart,
  onDragEnd
}: {
  item: ClipboardEntry;
  index: number;
  renderItem: RenderItem;
  isFirst?: boolean;
  onDragStart?: () => void;
  onDragEnd?: () => void;
}) => {
  const controls = useDragControls();
  return (
    <Reorder.Item
      value={item.id}
      dragListener={false}
      dragControls={controls}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      className={isFirst ? "first-virtual-item" : undefined}
      style={{
        listStyle: "none",
        overflow: "visible",
        paddingTop: isFirst ? "4px" : undefined,
        paddingBottom: "4px", /* match unpinned spacing roughly */
      }}
    >
      {renderItem(item, index, controls, true)}
    </Reorder.Item>
  );
};

const AppMainContent = ({
  t,
  settingsPanelProps,
  filteredHistory,
  pinnedItems,
  unpinnedItems,
  virtualListRef,
  handlePinnedReorder,
  renderItemContent,
  loadMoreHistory,
  handleListScroll,
  showScrollTop,
  onScrollTop,
  stickyEntries,
  onStickyRemoved,
  stickyEnabled,
  searchMode,
  filteredApps,
  filteredFiles,
  onLaunchApp,
  onOpenFile,
  onLaunchBrowserSearch,
  filesLoading,
  appsLoading,
  onAppsListScroll,
  onFilesListScroll,
  onAppsScrollTop,
  onFilesScrollTop,
  showAppsScrollTop,
  showFilesScrollTop,
  itemCount
}: AppMainContentProps) => {
  const {
    showSettings,
    showTagManager,
    tagManagerEnabled,
    appSearchEnabled,
  } = useUIStore();

  const {
    compactMode,
    pinnedCollapsed,
    settingsLoaded,
    settingsLoadError,
  } = useSettingsStore();

  const {
    search,
    selectedIndex,
    isKeyboardMode,
    hasMore,
    isLoadingMore
  } = useHistoryStore();

  const appsListRef = useRef<VirtualClipboardListHandle | null>(null);
  const filesListRef = useRef<VirtualClipboardListHandle | null>(null);

  const [pinnedOrderIds, setPinnedOrderIds] = useState<number[]>(
    () => pinnedItems.map((item) => item.id)
  );
  const pinnedOrderRef = useRef<number[]>(pinnedItems.map((item) => item.id));
  const [isDraggingPinned, setIsDraggingPinned] = useState(false);
  const [isPinnedExpanded, setIsPinnedExpanded] = useState(false);
  const [isStickyExpanded, setIsStickyExpanded] = useState(false);

  const hasStickies = stickyEnabled && stickyEntries && stickyEntries.length > 0;

  const scrollAppsToTop = useCallback(() => {
    appsListRef.current?.scrollToTop();
    onAppsScrollTop();
  }, [onAppsScrollTop]);

  const scrollFilesToTop = useCallback(() => {
    filesListRef.current?.scrollToTop();
    onFilesScrollTop();
  }, [onFilesScrollTop]);

  // 面板关闭时重置折叠状态，每次唤出都是折叠的
  useEffect(() => {
    const unlisten = listen("window-hidden", () => {
      setIsPinnedExpanded(false);
    });
    return () => { unlisten.then(fn => fn()).catch(() => {}); };
  }, []);

  useEffect(() => {
    if (isDraggingPinned) return;
    const next = pinnedItems.map((item) => item.id);
    setPinnedOrderIds(next);
    pinnedOrderRef.current = next;
  }, [pinnedItems, isDraggingPinned]);

  const orderedPinnedItems = useMemo(() => {
    if (pinnedItems.length === 0) return [];
    const map = new Map<number, ClipboardEntry>();
    pinnedItems.forEach((item) => map.set(item.id, item));

    const ordered: ClipboardEntry[] = [];
    const seen = new Set<number>();

    pinnedOrderIds.forEach((id) => {
      const item = map.get(id);
      if (!item) return;
      ordered.push(item);
      seen.add(id);
    });

    pinnedItems.forEach((item) => {
      if (!seen.has(item.id)) {
        ordered.push(item);
      }
    });

    return ordered;
  }, [pinnedItems, pinnedOrderIds]);

  const orderedPinnedIds = useMemo(
    () => orderedPinnedItems.map((item) => item.id),
    [orderedPinnedItems]
  );

  const handlePinnedIdsReorder = useCallback((nextIds: number[]) => {
    setPinnedOrderIds(nextIds);
    pinnedOrderRef.current = nextIds;
  }, []);

  const handlePinnedDragStart = useCallback(() => {
    setIsDraggingPinned(true);
  }, []);

  const handlePinnedDragEnd = useCallback(() => {
    setIsDraggingPinned(false);
    const finalIds = pinnedOrderRef.current;
    const currentIds = pinnedItems.map((item) => item.id);
    if (
      finalIds.length === currentIds.length &&
      finalIds.every((id, idx) => id === currentIds[idx])
    ) {
      return;
    }
    handlePinnedReorder(finalIds);
  }, [handlePinnedReorder, pinnedItems]);

  const transitionConfig = {
    initial: { opacity: 0, scale: 0.98 },
    animate: { opacity: 1, scale: 1 },
    exit: { opacity: 0, scale: 0.98 },
    transition: { duration: 0.15, ease: "easeOut" as const }
  };

  return (
    <AnimatePresence mode="popLayout">
      {showTagManager && tagManagerEnabled ? (
        <motion.div
          key="tag-manager"
          {...transitionConfig}
          style={{ height: "100%" }}
        >
          <Suspense fallback={null}>
            <TagManager t={t} theme="fluent" />
          </Suspense>
        </motion.div>
      ) : showSettings ? (
        <motion.div
          key="settings-panel"
          {...transitionConfig}
          className="settings-view"
          style={{ display: "flex", flexDirection: "column", gap: "12px" }}
        >
          {settingsLoaded ? (
            <Suspense fallback={null}>
              <SettingsPanel {...settingsPanelProps} />
            </Suspense>
          ) : (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                height: "100%",
                minHeight: 180,
                color: "var(--text-secondary)",
                fontSize: 12,
                textAlign: "center",
                padding: "0 16px",
              }}
            >
              {settingsLoadError ? t("settings_load_failed") : t("settings_loading")}
            </div>
          )}
        </motion.div>
      ) : searchMode === 'apps' && appSearchEnabled ? (
        <motion.div
          key="apps-list"
          {...transitionConfig}
          className="apps-list-container"
        >
          {appsLoading ? (
            <div className="empty-state">
              <Grid3X3 size={40} opacity={0.2} style={{ marginBottom: "12px" }} />
              <p>加载中...</p>
            </div>
          ) : filteredApps.length === 0 ? (
            <div className="empty-state">
              <Grid3X3 size={40} opacity={0.2} style={{ marginBottom: "12px" }} />
              <p>{search ? '没有找到应用' : '输入关键字搜索应用'}</p>
              {search && (
                <button className="browser-search-btn" onClick={() => onLaunchBrowserSearch(search)}>
                  <Search size={14} style={{ marginRight: '6px' }} />
                  在浏览器中搜索 "{search}"
                </button>
              )}
            </div>
          ) : (
            <>
              <div style={{ flex: 1, minHeight: 0 }}>
                <VirtualList<AppListItem>
                  listRef={appsListRef}
                  items={filteredApps}
                  compactMode={false}
                  onScroll={onAppsListScroll}
                  hasMore={false}
                  isLoading={false}
                  renderItem={(app, _index) => (
                    <div
                      className={`list-item app-item ${isKeyboardMode && _index === selectedIndex ? "selected" : ""}`}
                      onClick={() => onLaunchApp(app.path)}
                    >
                      <div className="app-icon-wrapper">
                        {app.icon ? (
                          <img src={app.icon} alt={app.name} className="app-icon" loading="lazy" />
                        ) : (
                          <Grid3X3 size={20} />
                        )}
                      </div>
                      <div className="app-info">
                        <div className="app-name">{app.name}</div>
                        <div className="app-path">{app.path}</div>
                      </div>
                    </div>
                  )}
                />
              </div>
              {showAppsScrollTop && (
                <button
                  type="button"
                  className="btn-icon scroll-top-button"
                  onClick={scrollAppsToTop}
                  aria-label={t("scroll_to_top")}
                  title={t("scroll_to_top")}
                >
                  <ArrowUp size={16} />
                </button>
              )}
              {itemCount !== undefined && (
                <div className="list-status-bar">{itemCount} 项</div>
              )}
            </>
          )}
        </motion.div>
      ) : searchMode === 'files' && appSearchEnabled ? (
        <motion.div
          key="files-list"
          {...transitionConfig}
          className="apps-list-container"
        >
          {filesLoading ? (
            <div className="empty-state">
              <FileSearch size={40} opacity={0.2} style={{ marginBottom: "12px" }} />
              <p>加载中...</p>
            </div>
          ) : filteredFiles.length === 0 ? (
            <div className="empty-state">
              <FileText size={40} opacity={0.2} style={{ marginBottom: "12px" }} />
              <p>{search ? '没有找到文件' : '输入关键字搜索文件'}</p>
              {search && (
                <button className="browser-search-btn" onClick={() => onLaunchBrowserSearch(search)}>
                  <Search size={14} style={{ marginRight: '6px' }} />
                  在浏览器中搜索 "{search}"
                </button>
              )}
            </div>
          ) : (
            <>
              <div style={{ flex: 1, minHeight: 0 }}>
                <VirtualList<FileSearchItem>
                  listRef={filesListRef}
                  items={filteredFiles}
                  compactMode={false}
                  onScroll={onFilesListScroll}
                  hasMore={false}
                  isLoading={false}
                  renderItem={(file, _index) => (
                    <div
                      className={`list-item app-item ${isKeyboardMode && _index === selectedIndex ? "selected" : ""}`}
                      onClick={() => onOpenFile(file.full_path)}
                    >
                      <div className="app-icon-wrapper">
                        {file.is_folder ? (
                          <Folder size={20} />
                        ) : (
                          <FileText size={20} />
                        )}
                      </div>
                      <div className="app-info">
                        <div className="app-name">{file.name}</div>
                        <div className="app-path">{file.path}</div>
                      </div>
                    </div>
                  )}
                />
              </div>
              {showFilesScrollTop && (
                <button
                  type="button"
                  className="btn-icon scroll-top-button"
                  onClick={scrollFilesToTop}
                  aria-label={t("scroll_to_top")}
                  title={t("scroll_to_top")}
                >
                  <ArrowUp size={16} />
                </button>
              )}
              {itemCount !== undefined && (
                <div className="list-status-bar">{itemCount} 项</div>
              )}
            </>
          )}
        </motion.div>
      ) : filteredHistory.length === 0 ? (
        <motion.div
          key="empty-state"
          {...transitionConfig}
          className="empty-state"
        >
          <Clipboard size={40} opacity={0.2} style={{ marginBottom: "12px" }} />
          {search ? (
            <>
              <p>{t("no_records")}</p>
              <button className="browser-search-btn" onClick={() => onLaunchBrowserSearch(search)}>
                <Search size={14} style={{ marginRight: '6px' }} />
                在浏览器中搜索 "{search}"
              </button>
            </>
          ) : (
            <>
              <p
                style={{
                  fontSize: "15px",
                  fontWeight: "bold",
                  color: "var(--text-primary)",
                  marginBottom: "4px"
                }}
              >
                {t("empty_title")}
              </p>
              <p style={{ fontSize: "12px", opacity: 0.6 }}>{t("empty_desc")}</p>
            </>
          )}
        </motion.div>
      ) : (
        <motion.div
          key="main-list"
          {...transitionConfig}
          className="history-list-container"
        >
          <div style={{ flex: 1, minHeight: 0 }}>
            <VirtualList
              listRef={virtualListRef}
              items={unpinnedItems}
              compactMode={compactMode}
              header={
                <div style={{ marginBottom: (hasStickies || pinnedItems.length > 0) ? 8 : 0 }}>
                  {hasStickies && (
                    <div style={{ marginBottom: pinnedItems.length > 0 ? 6 : 0 }}>
                      <div
                        onClick={() => setIsStickyExpanded(!isStickyExpanded)}
                        style={{ padding: "6px 12px", fontSize: "11px", fontWeight: "bold", color: "var(--text-secondary)", cursor: "pointer", display: "flex", alignItems: "center", gap: "6px", userSelect: "none",'marginBottom': '10px' }}
                      >
                        <span style={{ transform: isStickyExpanded ? 'rotate(90deg)' : 'rotate(0deg)', transition: 'transform 0.2s', display: 'inline-block' }}>▶</span>
                        {t('sticky_title') || '贴图记录'} ({stickyEntries!.length})
                      </div>
                      {isStickyExpanded && (
                        <div style={{ padding: "0 0 4px 0", display: "flex", flexDirection: "column", gap: "4px" }}>
                          {stickyEntries!.map((entry) => (
                            <div key={entry.id} style={{ width: "calc(100% - 12px)", display: "flex", alignItems: "center", justifyContent: "space-between", padding: "6px 10px", borderRadius: 6, background: "var(--bg-element)", fontSize: 12, color: "var(--text-secondary)" }}>
                              <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                                [{entry.content_type === "image" ? "图片" : "文字"}] {entry.content_type === "image" ? "" : (entry.content?.substring(0, 40) + (entry.content?.length > 40 ? "..." : ""))}
                              </span>
                              <button
                                className="btn-icon"
                                onClick={async (e) => {
                                  e.stopPropagation();
                                  await StickyManager.closeSticky(entry.id);
                                  onStickyRemoved?.();
                                }}
                                title="取消贴图"
                                style={{ width: 24, height: 24, minWidth: 24 }}
                              ><X size={14} /></button>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                  {pinnedItems.length > 0 ? (
                  pinnedCollapsed ? (
                    <div style={{ marginBottom: "8px" }}>
                      <div
                        onClick={() => setIsPinnedExpanded(!isPinnedExpanded)}
                        style={{
                          padding: "6px 12px",
                          fontSize: "11px",
                          fontWeight: "bold",
                          color: "var(--text-secondary)",
                          cursor: "pointer",
                          display: "flex",
                          alignItems: "center",
                          gap: "6px",
                          userSelect: "none"
                        }}
                      >
                        <span style={{ transform: isPinnedExpanded ? 'rotate(90deg)' : 'rotate(0deg)', transition: 'transform 0.2s', display: 'inline-block' }}>▶</span>
                        {t('pinned_items_title') || '置顶记录'} ({pinnedItems.length})
                      </div>
                      {isPinnedExpanded && (
                        <Reorder.Group
                          axis="y"
                          values={orderedPinnedIds}
                          onReorder={handlePinnedIdsReorder}
                          className={isDraggingPinned ? "pinned-reorder dragging" : "pinned-reorder"}
                          style={{ listStyle: "none", padding: 0 }}
                        >
                          {orderedPinnedItems.map((item, index) => (
                            <SortableItem
                              key={item.id}
                              item={item}
                              index={index}
                              renderItem={renderItemContent}
                              isFirst={index === 0}
                              onDragStart={handlePinnedDragStart}
                              onDragEnd={handlePinnedDragEnd}
                            />
                          ))}
                        </Reorder.Group>
                      )}
                    </div>
                  ) : (
                    <Reorder.Group
                      axis="y"
                      values={orderedPinnedIds}
                      onReorder={handlePinnedIdsReorder}
                      className={isDraggingPinned ? "pinned-reorder dragging" : "pinned-reorder"}
                      style={{ listStyle: "none", padding: 0 }}
                    >
                      {orderedPinnedItems.map((item, index) => (
                        <SortableItem
                          key={item.id}
                          item={item}
                          index={index}
                          renderItem={renderItemContent}
                          isFirst={index === 0}
                          onDragStart={handlePinnedDragStart}
                          onDragEnd={handlePinnedDragEnd}
                        />
                      ))}
                    </Reorder.Group>
                  )
                ) : null}
                </div>
              }
              renderItem={(item, index, isFirst?: boolean) => {
                const absoluteIndex = pinnedItems.length + index;
                const el = renderItemContent(item, absoluteIndex, undefined, true);
                if (isFirst && (pinnedItems.length === 0 || pinnedCollapsed)) {
                  return (
                    <div className="first-virtual-item" style={{ height: "100%" }}>
                      {el}
                    </div>
                  );
                }
                return el;
              }}
              onLoadMore={loadMoreHistory}
              onScroll={handleListScroll}
              hasMore={hasMore}
              isLoading={isLoadingMore}
            />
          </div>
          {showScrollTop && (
            <button
              type="button"
              className="btn-icon scroll-top-button"
              onClick={onScrollTop}
              aria-label={t("scroll_to_top")}
              title={t("scroll_to_top")}
            >
              <ArrowUp size={16} />
            </button>
          )}
          {itemCount !== undefined && (
            <div className="list-status-bar">{itemCount} 项</div>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
};

export default AppMainContent;
