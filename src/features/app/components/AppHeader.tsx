import { useCallback } from "react";
import type { RefObject } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  ChevronLeft,
  Clipboard,
  FileSearch,
  Grid3X3,
  Pin,
  PinOff,
  Search,
  Settings as SettingsIcon,
  Tag,
  X
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getTagColor } from "../../../shared/lib/utils";
import { useHistoryStore } from "../../../shared/store/historyStore";
import { useUIStore } from "../../../shared/store/uiStore";

interface AppHeaderProps {
  t: (key: string) => string;
  searchInputRef: RefObject<HTMLInputElement | null>;
  allTags: string[];
}

const AppHeader = ({
  t,
  searchInputRef,
  allTags,
}: AppHeaderProps) => {
  const {
    showSettings,
    setShowSettings,
    showTagManager,
    setShowTagManager,
    tagManagerEnabled,
    isWindowPinned,
    setIsWindowPinned,
    showSearchBox,
    appSearchEnabled,
  } = useUIStore();

  const {
    search,
    setSearch,
    setIsComposing,
    showTagFilter,
    setShowTagFilter,
    searchIsFocused,
    setSearchIsFocused,
    setEditingTagsId,
    typeFilter,
    setTypeFilter,
    searchMode,
    setSearchMode
  } = useHistoryStore();

  const getTypeName = (type: string) => {
    switch (type) {
      case "code": return t('type_code');
      case "link":
      case "url": return t('type_url');
      case "file": return t('type_file');
      case "image": return t('type_image');
      case "video": return t('type_video');
      case "rich_text": return t('type_rich_text');
      default: return t('type_text') || 'Text';
    }
  };

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (!(e.target as HTMLElement).closest('button, input, select, textarea')) {
      getCurrentWindow().startDragging().catch(console.error);
    }
  }, []);

  return (
  <header onMouseDown={handleMouseDown}>
    <div className="header-top">
      <div className="header-leading">
        {(showSettings || showTagManager) && (
          <button className="btn-icon" onClick={() => {
            if (showTagManager) setShowTagManager(false);
            else setShowSettings(false);
          }}>
            <ChevronLeft size={18} />
          </button>
        )}
        <div className="header-drag-region" style={{ flex: 1, alignSelf: 'stretch' }}>
          <span className="header-title">
            {showTagManager && tagManagerEnabled
                ? (t('tag_manager') || '标签管理')
                : showSettings
                  ? t('settings')
                  : ''}
          </span>
        </div>
      </div>
      <div style={{ display: 'flex', gap: '4px' }}>
        <button
          className={`btn-icon ${isWindowPinned ? 'active' : ''}`}
          title={t('pin')}
          onClick={() => {
            const newVal = !isWindowPinned;
            setIsWindowPinned(newVal);
            invoke("set_window_pinned", { pinned: newVal }).catch(console.error);
          }}
        >
          {isWindowPinned ? <PinOff size={16} /> : <Pin size={16} />}
        </button>

        {!showSettings && !showTagManager && (
          <>
            {tagManagerEnabled && (
              <button className="btn-icon" title={t('tag_manager') || '标签管理'} onClick={() => setShowTagManager(true)}>
                <Tag size={16} />
              </button>
            )}
            <button className="btn-icon" title={t('settings')} onClick={() => setShowSettings(true)}>
              <SettingsIcon size={16} />
            </button>
          </>
        )}
        <button className="btn-icon" title={t('hide')} onClick={async () => {
          invoke("hide_window_cmd").catch(console.error);
        }}>
          <X size={16} />
        </button>
      </div>
    </div>

    {!showSettings && !showTagManager && (
      <AnimatePresence>
        {(showSearchBox || search.trim().length > 0) && (
          <motion.div
            initial={{ height: 0, opacity: 0, overflow: 'hidden' }}
            animate={{
              height: "auto",
              opacity: 1,
              transitionEnd: { overflow: "visible" }
            }}
            exit={{ height: 0, opacity: 0, overflow: 'hidden' }}
            transition={{ duration: 0.2, ease: "easeOut" }}
            style={{ flexShrink: 0 }}
          >
            <div className="search-container">
              <div style={{ position: 'relative' }}>
                {searchMode === 'apps' ? <Grid3X3 size={14} className="search-icon" />
                  : searchMode === 'files' ? <FileSearch size={14} className="search-icon" />
                  : <Search size={14} className="search-icon" />}
                <input
                  ref={searchInputRef}
                  type="text"
                  className={`search-input ${showTagFilter && allTags.length > 0 && search.trim() === "" ? 'dropdown-open' : ''}`}
                  placeholder={searchMode === 'apps' ? t('search_placeholder_apps') : searchMode === 'files' ? t('search_placeholder_files') : t('search_placeholder')}
                  value={search}
                  onCompositionStart={() => setIsComposing(true)}
                  onCompositionEnd={(e) => {
                    setIsComposing(false);
                    setSearch((e.target as HTMLInputElement).value);
                  }}
                  onChange={(e) => {
                    setSearch(e.target.value);
                  }}
                  onMouseDown={() => {
                    invoke("activate_window_focus").catch(console.error);
                  }}
                  onClick={() => { setShowTagFilter(true); setEditingTagsId(null); }}
                  onFocus={() => {
                    invoke("activate_window_focus").catch(console.error);
                    invoke("set_search_focused", { focused: true }).catch(console.error);
                    setShowTagFilter(true);
                    setSearchIsFocused(true);
                    setEditingTagsId(null);
                  }}
                  onBlur={() => {
                    invoke("set_search_focused", { focused: false }).catch(console.error);
                    setTimeout(() => {
                      setShowTagFilter(false);
                      setSearchIsFocused(false);
                    }, 200);
                  }}
                />
                {showTagFilter && searchIsFocused && allTags.length > 0 && search.trim() === "" && (
                  <motion.div
                    className="tags-dropdown"
                    initial={{ opacity: 0, y: -10 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -10 }}
                    transition={{ duration: 0.15, delay: 0.15, ease: "easeOut" }}
                  >
                    <div className="tags-label">{t('tags') || "Tags"}</div>
                    <div className="tags-list">
                      {allTags.map(tag => (
                        <span
                          className="tag-chip"
                          key={tag}
                          onMouseDown={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            setSearch("tag:" + tag);
                            setShowTagFilter(false);
                          }}
                          data-tag={tag}
                          style={{ background: getTagColor(tag, "fluent") }}
                        >
                          {tag}
                        </span>
                      ))}
                    </div>
                  </motion.div>
                )}
              </div>
              <div
                className="hide-scrollbar"
                style={{
                  display: 'flex',
                  gap: '6px',
                  padding: '8px 0 0 0',
                  overflowX: 'auto',
                  scrollbarWidth: 'none',
                  msOverflowStyle: 'none',
                  flexWrap: 'nowrap'
                }}
                onWheel={(e) => {
                  if (e.deltaY !== 0) {
                    e.currentTarget.scrollLeft += e.deltaY;
                  }
                }}
              >
                <button
                  className={`btn-icon ${searchMode === 'clipboard' ? 'active' : ''}`}
                  onClick={() => {
                    setSearchMode('clipboard');
                    setTypeFilter(null);
                  }}
                  style={{
                    width: 'auto',
                    padding: '4px 8px',
                    fontSize: '11px',
                    borderRadius: '4px',
                    whiteSpace: 'nowrap',
                    flexShrink: 0,
                    opacity: searchMode === 'clipboard' ? 1 : 0.7
                  }}
                  title={t('clipboard') || '剪贴板'}
                >
                  <Clipboard size={12} style={{ marginRight: '4px', display: 'inline' }} />
                  {t('clipboard') || '剪贴板'}
                </button>
                {appSearchEnabled && (
                  <>
                    <button
                      className={`btn-icon ${searchMode === 'apps' ? 'active' : ''}`}
                      onClick={() => {
                        setSearchMode('apps');
                        setTypeFilter(null);
                      }}
                      style={{
                        width: 'auto',
                        padding: '4px 8px',
                        fontSize: '11px',
                        borderRadius: '4px',
                        whiteSpace: 'nowrap',
                        flexShrink: 0,
                        opacity: searchMode === 'apps' ? 1 : 0.7
                      }}
                      title={t('apps') || '应用'}
                    >
                      <Grid3X3 size={12} style={{ marginRight: '4px', display: 'inline' }} />
                      {t('apps') || '应用'}
                    </button>
                    <button
                      className={`btn-icon ${searchMode === 'files' ? 'active' : ''}`}
                      onClick={() => {
                        setSearchMode('files');
                        setTypeFilter(null);
                      }}
                      style={{
                        width: 'auto',
                        padding: '4px 8px',
                        fontSize: '11px',
                        borderRadius: '4px',
                        whiteSpace: 'nowrap',
                        flexShrink: 0,
                        opacity: searchMode === 'files' ? 1 : 0.7
                      }}
                      title={t('files') || '文件'}
                    >
                      <FileSearch size={12} style={{ marginRight: '4px', display: 'inline' }} />
                      {t('files') || '文件'}
                    </button>
                  </>
                )}
                {searchMode === 'clipboard' && ['text', 'image', 'file', 'url', 'code', 'video', 'rich_text'].map(t => (
                  <button
                    key={t}
                    className={`btn-icon ${typeFilter === t ? 'active' : ''}`}
                    onClick={() => setTypeFilter(typeFilter === t ? null : t)}
                    style={{
                      width: 'auto',
                      padding: '4px 8px',
                      fontSize: '11px',
                      borderRadius: '4px',
                      whiteSpace: 'nowrap',
                      flexShrink: 0,
                      opacity: typeFilter === t ? 1 : 0.7
                    }}
                    title={getTypeName(t)}
                  >
                    {getTypeName(t)}
                  </button>
                ))}
              </div>

            </div>
          </motion.div>
        )}
      </AnimatePresence>
    )}
  </header>
);
};

export default AppHeader;
