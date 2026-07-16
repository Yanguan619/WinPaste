import { useState, useEffect, useRef, useMemo } from 'react';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { listen, emit } from '@tauri-apps/api/event';
import {
    Edit2, Trash2, X, LayoutGrid, List,
    Clock, MousePointer2, Plus, Search, ExternalLink
} from 'lucide-react';
import { getTagColor } from "../../../shared/lib/utils";
import type { ClipboardEntry } from "../../../shared/types";

interface TagManagerProps {
    t: (key: string) => string;
    theme: string;
}

interface TagInfo {
    name: string;
    count: number;
}

export default function TagManager({ t, theme }: TagManagerProps) {
    const [tags, setTags] = useState<TagInfo[]>([]);
    const [tagSearch, setTagSearch] = useState('');
    const [selectedTag, setSelectedTag] = useState<string | null>(null);
    const [tagItems, setTagItems] = useState<ClipboardEntry[]>([]);
    const [tagColors, setTagColors] = useState<Record<string, string>>({});
    const [editingTag, setEditingTag] = useState<string | null>(null);
    const [newTagName, setNewTagName] = useState('');
    const [loading, setLoading] = useState(false);
    const [viewMode, setViewMode] = useState<'list' | 'grid'>('grid');
    const [isDeleting, setIsDeleting] = useState(false);
    const [deleteConfirmation, setDeleteConfirmation] = useState<{ show: boolean, tagName: string | null }>({ show: false, tagName: null });
    const [itemDeleteConfirmation, setItemDeleteConfirmation] = useState<{ show: boolean, id: number | null }>({ show: false, id: null });
    const [sortBy, setSortBy] = useState<'time' | 'count'>('time');
    const [isCreatingItem, setIsCreatingItem] = useState(false);
    const [editingItem, setEditingItem] = useState<{ id: number, content: string } | null>(null);
    const [newItemContent, setNewItemContent] = useState('');

    const selectedTagRef = useRef<string | null>(null);
    const tagScrollRef = useRef<HTMLDivElement>(null);
    useEffect(() => { selectedTagRef.current = selectedTag; }, [selectedTag]);

    // Convert vertical wheel to horizontal scroll for tag chips
    useEffect(() => {
        const el = tagScrollRef.current;
        if (!el) return;
        const handleWheel = (e: WheelEvent) => {
            if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
                e.preventDefault();
                el.scrollLeft += e.deltaY;
            }
        };
        el.addEventListener('wheel', handleWheel, { passive: false });
        return () => el.removeEventListener('wheel', handleWheel);
    }, []);

    useEffect(() => {
        let unlisteners: (() => void)[] = [];
        const setupListeners = async () => {
            const handleUpdate = () => {
                // Don't refresh if we're in the middle of a delete operation
                if (isDeleting) return;
                fetchTags();
                if (selectedTagRef.current) loadTagItems(selectedTagRef.current);
            };
            unlisteners.push(await listen('clipboard-changed', handleUpdate));
            unlisteners.push(await listen('clipboard-updated', handleUpdate));
            unlisteners.push(await listen('clipboard-removed', handleUpdate));
        };
        setupListeners();
        return () => unlisteners.forEach(f => f());
    }, [isDeleting]);

    useEffect(() => { fetchTags(); }, []);

    const fetchTags = async () => {
        try {
            const [tagMap, colors] = await Promise.all([
                invoke<Record<string, number>>('get_all_tags_info'),
                invoke<Record<string, string>>('get_tag_colors')
            ]);

            const tagArray = Object.entries(tagMap).map(([name, count]) => ({ name, count }));
            tagArray.sort((a, b) => b.count - a.count);
            setTags(tagArray);
            setTagColors(colors || {});

            const activeTag = selectedTagRef.current;
            if (tagArray.length === 0) {
                setSelectedTag(null);
                setTagItems([]);
                return;
            }
            if (!activeTag || !tagArray.some(tag => tag.name === activeTag)) {
                loadTagItems(tagArray[0].name);
            }
        } catch (err) { console.error(err); }
    };

    const loadTagItems = async (tagName: string) => {
        setLoading(true);
        setSelectedTag(tagName);
        try {
            const items = await invoke<ClipboardEntry[]>('get_tag_items', { tag: tagName });
            setTagItems(items || []);
        } catch (err) { console.error(err); setTagItems([]); }
        finally { setLoading(false); }
    };

    const createTag = async (rawName: string) => {
        const trimmed = rawName.trim();
        if (!trimmed) return;

        try {
            await invoke('create_new_tag', { tagName: trimmed });
            setNewTagName('');
            setTagSearch('');
            await fetchTags();
            await loadTagItems(trimmed);
        } catch (err) { console.error(err); }
    };

    const handleRenameTag = async (oldName: string) => {
        const trimmed = newTagName.trim();
        if (!trimmed || trimmed === oldName) { setEditingTag(null); return; }

        try {
            await invoke('rename_tag_globally', { oldName, newName: trimmed });
            if (selectedTag === oldName) setSelectedTag(trimmed);
            await fetchTags();
            await loadTagItems(trimmed);
            setEditingTag(null);
            setNewTagName('');
        } catch (err) { console.error(err); }
    };

    const handleDeleteTag = async (tagName: string) => {
        setIsDeleting(true);
        try {
            await invoke('delete_tag_from_all', { tagName });
            await emit('clipboard-changed'); // Notify App.tsx to refresh
            await fetchTags();
        } catch (err) { console.error(err); }
        finally {
            setIsDeleting(false);
        }
    };

    const handleAddManualItem = async () => {
        if (!newItemContent.trim() || !selectedTag) return;
        try {
            await invoke('add_manual_item', {
                content: newItemContent,
                contentType: 'text',
                tags: [selectedTag]
            });
            setNewItemContent('');
            setIsCreatingItem(false);
            await loadTagItems(selectedTag);
        } catch (err) { console.error(err); }
    };

    const handleUpdateItemContent = async () => {
        if (!editingItem || !editingItem.content.trim()) return;
        try {
            await invoke('update_item_content', {
                id: editingItem.id,
                newContent: editingItem.content
            });
            setEditingItem(null);
            if (selectedTag) await loadTagItems(selectedTag);
        } catch (err) { console.error(err); }
    };

    const copyToClipboard = async (id: number, content: string, type: string) => {
        try {
            await invoke('copy_to_clipboard', { content, contentType: type, paste: true, id, deleteAfterUse: false });
        } catch (err) { console.error(err); }
    };

    const filteredTags = useMemo(() => {
        return tags.filter(t => t.name.toLowerCase().includes(tagSearch.toLowerCase()));
    }, [tags, tagSearch]);

    const normalizedTagSearch = tagSearch.trim().toLowerCase();
    const canCreateTag = normalizedTagSearch.length > 0
        && !tags.some(tag => tag.name.toLowerCase() === normalizedTagSearch);

    const sortedItems = [...tagItems].sort((a, b) => {
        if (sortBy === 'count') return (b.use_count || 0) - (a.use_count || 0);
        return b.timestamp - a.timestamp;
    });

    const formatItemDate = (timestamp: number) => {
        const date = new Date(timestamp);
        const year = date.getFullYear();
        const month = String(date.getMonth() + 1).padStart(2, '0');
        const day = String(date.getDate()).padStart(2, '0');
        return `${year}-${month}-${day}`;
    };

    return (
        <div
            className={`themed-tag-manager theme-${theme}`}
            onMouseDown={() => invoke('activate_window_focus').catch(console.error)}
        >
            {/* Tags bar: header + search in one row, then chips */}
            <div className="tag-sidebar">
                <div className="tag-top-bar">
                    <span className="header-label">{t('tags')}</span>
                    <div className="tag-search-box">
                        <Search size={14} className="search-icon-placeholder" />
                        <input
                            placeholder={t('find_or_create')}
                            value={tagSearch}
                            onMouseDown={() => invoke('activate_window_focus').catch(console.error)}
                            onFocus={() => invoke('activate_window_focus').catch(console.error)}
                            onChange={e => setTagSearch(e.target.value)}
                            onKeyDown={async (e) => {
                                if (e.key === 'Enter' && tagSearch.trim()) {
                                    const exactMatch = tags.find(t => t.name.toLowerCase() === normalizedTagSearch);
                                    if (exactMatch) {
                                        loadTagItems(exactMatch.name);
                                    } else {
                                        await createTag(tagSearch);
                                    }
                                }
                            }}
                        />
                        {tagSearch ? (
                            <div className="action-icons">
                                {canCreateTag ? (
                                    <span
                                        title={t('create_new_tag_tooltip')}
                                        className="action-icon create"
                                        onClick={() => createTag(tagSearch)}
                                    >
                                        <Plus size={12} />
                                    </span>
                                ) : null}
                                <X size={12} className="action-icon clear" onClick={() => setTagSearch('')} />
                            </div>
                        ) : null}
                    </div>
                </div>

                <div ref={tagScrollRef} className="tag-scroll custom-scrollbar">
                    {filteredTags.map(tag => (
                        <div
                            key={tag.name}
                            className={`tag-item ${selectedTag === tag.name ? 'active' : ''}`}
                            onClick={() => loadTagItems(tag.name)}
                        >
                            <div className="tag-color-wrapper" onClick={(e) => e.stopPropagation()}>
                                <div
                                    className="tag-color-dot"
                                    style={{ background: tagColors[tag.name] || getTagColor(tag.name, theme) }}
                                    onClick={() => document.getElementById(`color-picker-${tag.name}`)?.click()}
                                />
                                <input
                                    type="color"
                                    id={`color-picker-${tag.name}`}
                                    style={{ display: 'none' }}
                                    value={tagColors[tag.name] || '#888888'} // Approximation if not set, or maybe convert HSL to Hex?
                                    onChange={async (e) => {
                                        const newColor = e.target.value;
                                        setTagColors(prev => ({ ...prev, [tag.name]: newColor }));
                                        await invoke('set_tag_color', { name: tag.name, color: newColor });
                                        await emit('tag-colors-updated');
                                    }}
                                />
                            </div>
                            {editingTag === tag.name ? (
                                <input
                                    className="inline-tag-edit"
                                    value={newTagName}
                                    onMouseDown={() => invoke('activate_window_focus').catch(console.error)}
                                    onFocus={() => invoke('activate_window_focus').catch(console.error)}
                                    onChange={(e) => setNewTagName(e.target.value)}
                                    autoFocus
                                    onKeyDown={async (e) => {
                                        if (e.key === 'Enter') {
                                            await handleRenameTag(tag.name);
                                        } else if (e.key === 'Escape') {
                                            setEditingTag(null);
                                        }
                                    }}
                                    onBlur={() => setEditingTag(null)}
                                    onClick={(e) => e.stopPropagation()}
                                />
                            ) : (
                                <>
                                    <span className="tag-name">{tag.name}</span>
                                    <div className="tag-hover-actions">
                                        <span title="重命名" onClick={(e) => {
                                            e.stopPropagation();
                                            setEditingTag(tag.name);
                                            setNewTagName(tag.name);
                                        }} style={{
                                            display: 'flex',
                                            alignItems: 'center'
                                        }}>
                                            <Edit2 size={12} />
                                        </span>
                                        <span title="删除" onClick={(e) => {
                                            e.stopPropagation();
                                            e.preventDefault();
                                            setDeleteConfirmation({ show: true, tagName: tag.name });
                                        }} style={{ display: 'flex', alignItems: 'center', cursor: 'pointer' }}>
                                            <Trash2 size={12} />
                                        </span>
                                    </div>
                                    <span className="tag-badge">{tag.count}</span>
                                </>
                            )}
                        </div>
                    ))}
                    {filteredTags.length === 0 && !tagSearch.trim() && (
                        <div className="sidebar-status">{t('no_tags')}</div>
                    )}
                    {/* Visual cue for creating new tag when filtering shows no results */}
                    {canCreateTag && filteredTags.length === 0 && (
                        <div className="tag-item create-hint" onClick={() => createTag(tagSearch)}>
                            <div className="tag-color-dot" style={{ border: '1px dashed currentColor', background: 'transparent' }} />
                            <span className="tag-name" style={{ opacity: 0.7 }}>{t('create_tag_hint').replace('{tag}', tagSearch.trim())}</span>
                            <Plus size={10} />
                        </div>
                    )}
                </div>
            </div>

            {/* Right Main Area */}
            <div className="tag-content">
                <div className="content-toolbar">
                    <div className="toolbar-left">
                        <div className="selected-tag-indicator">
                            <span className="breadcrumb-marker">#</span>
                            <span className="breadcrumb-text">{selectedTag || t('tags')}</span>
                        </div>
                        <div className="toolbar-divider" />
                        <div className="sort-group">
                            <button
                                className={`sort-btn ${sortBy === 'time' ? 'active' : ''}`}
                                title={t('sort_time') || '按时间'}
                                onClick={() => setSortBy('time')}
                            >
                                <Clock size={14} />
                            </button>
                            <button
                                className={`sort-btn ${sortBy === 'count' ? 'active' : ''}`}
                                title={t('sort_usage') || '按频率'}
                                onClick={() => setSortBy('count')}
                            >
                                <MousePointer2 size={14} />
                            </button>
                        </div>
                    </div>
                    <div className="toolbar-right">
                        {selectedTag && (
                            <button className="add-item-btn btn-icon" onClick={() => setIsCreatingItem(true)} title={t('add_item')}>
                                <Plus size={14} />
                            </button>
                        )}
                    <div className="view-toggle">
                        <button
                            type="button"
                            className={`toggle-btn btn-icon ${viewMode === 'list' ? 'active' : ''}`}
                            title="列表视图"
                            onClick={() => setViewMode('list')}
                        ><List size={14} /></button>
                        <button
                            type="button"
                            className={`toggle-btn btn-icon ${viewMode === 'grid' ? 'active' : ''}`}
                            title="卡片视图"
                            onClick={() => setViewMode('grid')}
                        ><LayoutGrid size={14} /></button>
                    </div>
                    </div>
                </div>

                <div className="items-area custom-scrollbar">
                    {loading ? <div className="status-msg">{t('processing')}</div> : sortedItems.length === 0 ? (
                        <div className="status-msg">{selectedTag ? t('no_items') : t('select_tag_to_begin')}</div>
                    ) : (
                        <div className={`items-${viewMode}`}>
                            {sortedItems.map(item => (
                                <div key={item.id} className="themed-card" onClick={() => copyToClipboard(item.id, item.content, item.content_type)}>
                                    <div className="card-top-row">
                                        <div className="card-actions-left">
                                            {item.content_type === 'text' || item.content_type === 'code' ? (
                                                <button className="card-action-btn" title="编辑" onClick={(e) => {
                                                    e.stopPropagation();
                                                    setEditingItem({ id: item.id, content: item.content });
                                                }}>
                                                    <Edit2 size={10} />
                                                </button>
                                            ) : null}
                                            <button
                                                className="card-action-btn"
                                                onClick={(e) => {
                                                    e.stopPropagation();
                                                    invoke('open_content', {
                                                        id: item.id,
                                                        content: item.content,
                                                        contentType: item.content_type
                                                    });
                                                }}
                                                title={t('open')}
                                            >
                                                <ExternalLink size={10} />
                                            </button>
                                        </div>
                                        <button className="del-btn" title="删除" onClick={(e) => {
                                            e.stopPropagation();
                                            setItemDeleteConfirmation({ show: true, id: item.id });
                                        }}>
                                            <X size={10} />
                                        </button>
                                    </div>

                                    {item.content_type === 'image' ? (
                                        <div className="card-media">
                                            <img
                                                src={item.content.startsWith('data:') ? item.content : convertFileSrc(item.content)}
                                                alt=""
                                                className="image-preview"
                                                loading="lazy"
                                            />
                                        </div>
                                    ) : (
                                        <div className="card-body-text">{item.preview || item.content}</div>
                                    )}

                                    <div className="card-divider" />
                                    <div className="card-footer">
                                        <span className="meta-time">{formatItemDate(item.timestamp)}</span>
                                        <div className="meta-usage"><MousePointer2 size={8} /> {item.use_count || 0}</div>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            </div>

            {/* Modals for Create (Rename is handled inline now) */}
            {/* Kept minimal if needed for future extensions, but currently inline handles rename */}

            {/* Tag Delete Confirmation Modal */}
            {deleteConfirmation.show && (
                <div className="modal-overlay" onClick={() => setDeleteConfirmation({ show: false, tagName: null })}>
                    <div className={`confirm-dialog tag-manager-dialog theme-${theme}`} onClick={(e) => e.stopPropagation()}>
                        <h3>{t('confirm_delete')}</h3>
                        <p>
                            {t('confirm_delete_tag')}
                            <br />
                            <span className="tag-highlight" style={{ marginTop: '8px', display: 'inline-block' }}>
                                {deleteConfirmation.tagName}
                            </span>
                        </p>
                        <div className="confirm-dialog-buttons">
                            <button className="confirm-dialog-button" onClick={() => setDeleteConfirmation({ show: false, tagName: null })}>
                                {t('cancel')}
                            </button>
                            <button className="confirm-dialog-button primary" onClick={() => {
                                if (deleteConfirmation.tagName) {
                                    handleDeleteTag(deleteConfirmation.tagName);
                                }
                                setDeleteConfirmation({ show: false, tagName: null });
                            }}>
                                {t('delete')}
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Item Delete Confirmation Modal */}
            {itemDeleteConfirmation.show && (
                <div className="modal-overlay" onClick={() => setItemDeleteConfirmation({ show: false, id: null })}>
                    <div className={`confirm-dialog tag-manager-dialog theme-${theme}`} onClick={e => e.stopPropagation()}>
                        <h3>{t('confirm_delete')}</h3>
                        <p>{t('confirm_delete_desc') || "确定要删除这条记录吗？"}</p>
                        <div className="confirm-dialog-buttons">
                            <button className="confirm-dialog-button" onClick={() => setItemDeleteConfirmation({ show: false, id: null })}>
                                {t('cancel')}
                            </button>
                            <button className="confirm-dialog-button primary" onClick={async () => {
                                if (itemDeleteConfirmation.id) {
                                    await invoke('delete_clipboard_entry', { id: itemDeleteConfirmation.id });
                                    loadTagItems(selectedTag!);
                                    emit('clipboard-changed');
                                }
                                setItemDeleteConfirmation({ show: false, id: null });
                            }}>
                                {t('delete')}
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Create Item Modal */}
            {isCreatingItem && (
                <div className="modal-overlay" onClick={() => setIsCreatingItem(false)}>
                    <div className={`confirm-dialog tag-manager-dialog theme-${theme}`} onClick={e => e.stopPropagation()}>
                        <h3>{t('add_item')}</h3>
                        <div className="modal-input-field">
                            <textarea
                                className="tag-manager-textarea"
                                value={newItemContent}
                                onChange={e => setNewItemContent(e.target.value)}
                                placeholder={t('input_content_placeholder')}
                                autoFocus
                            />
                        </div>
                        <div className="confirm-dialog-buttons">
                            <button className="confirm-dialog-button" onClick={() => setIsCreatingItem(false)}>
                                {t('cancel')}
                            </button>
                            <button className="confirm-dialog-button primary" onClick={handleAddManualItem}>
                                {t('confirm')}
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Edit Item Modal */}
            {editingItem && (
                <div className="modal-overlay" onClick={() => setEditingItem(null)}>
                    <div className={`confirm-dialog tag-manager-dialog theme-${theme}`} onClick={e => e.stopPropagation()}>
                        <h3>{t('edit_item')}</h3>
                        <div className="modal-input-field">
                            <textarea
                                className="tag-manager-textarea"
                                value={editingItem.content}
                                onChange={e => setEditingItem({ ...editingItem, content: e.target.value })}
                                autoFocus
                            />
                        </div>
                        <div className="confirm-dialog-buttons">
                            <button className="confirm-dialog-button" onClick={() => setEditingItem(null)}>
                                {t('cancel')}
                            </button>
                            <button className="confirm-dialog-button primary" onClick={handleUpdateItemContent}>
                                {t('save')}
                            </button>
                        </div>
                    </div>
                </div>
            )}
            <style>{`
                .themed-tag-manager {
                    display: flex;
                    flex-direction: column;
                    height: 100%;
                    background: var(--bg-content);
                    font-family: var(--font-main, ui-monospace, monospace);
                    color: var(--text-primary);
                    gap: 10px;
                    padding: 10px;
                }

                .tag-sidebar {
                    flex-shrink: 0;
                    display: flex;
                    flex-direction: column;
                    border: var(--panel-border);
                    border-radius: var(--radius-md);
                    background: var(--bg-panel);
                    box-shadow: 0 2px 8px var(--shadow);
                    overflow: hidden;
                }

                .tag-top-bar {
                    min-height: 36px;
                    padding: 6px 10px;
                    display: flex;
                    align-items: center;
                    gap: 8px;
                    border-bottom: 1px solid var(--panel-divider-color);
                    background: transparent;
                }

                .header-label {
                    font-size: 11px;
                    font-weight: 700;
                    text-transform: uppercase;
                    letter-spacing: 0.5px;
                    color: var(--text-secondary);
                    flex-shrink: 0;
                }

                .tag-search-box {
                    position: relative;
                    flex: 1;
                    min-width: 0;
                    height: 26px;
                    padding: 0 6px;
                    display: flex;
                    align-items: center;
                    border: var(--input-border);
                    border-radius: var(--input-radius);
                    background: var(--bg-input);
                    box-shadow: var(--input-shadow);
                }
                .tag-search-box .search-icon-placeholder {
                    position: absolute;
                    left: 6px;
                    top: 50%;
                    transform: translateY(-50%);
                    color: var(--text-secondary);
                    opacity: 1;
                    pointer-events: none;
                }
                .tag-search-box input {
                    width: 100%;
                    height: 22px;
                    padding: 0 32px 0 18px;
                    border: none;
                    border-radius: 0;
                    background: transparent;
                    box-shadow: none;
                    color: var(--text-primary);
                    font-size: 11px;
                    font-weight: 500;
                    outline: none;
                }
                .tag-search-box input::placeholder { color: var(--text-muted); font-size: 11px; }
                .action-icons { position: absolute; right: 4px; top: 50%; transform: translateY(-50%); display: flex; align-items: center; gap: 2px; }
                .action-icon { width: 14px; height: 14px; display: inline-flex; align-items: center; justify-content: center; border-radius: 999px; color: var(--text-secondary); cursor: pointer; opacity: 1; transition: all 0.16s ease; }
                .action-icon:hover { background: rgba(var(--accent-color-rgb), 0.1); color: var(--text-primary); }
                .action-icon.create { background: rgba(var(--accent-color-rgb), 0.12); color: var(--accent-color); }

                .tag-scroll { flex: 1; min-height: 0; overflow-x: auto; overflow-y: hidden; padding: 6px 10px; white-space: nowrap; scrollbar-width: none; -ms-overflow-style: none; }
                .tag-scroll::-webkit-scrollbar { display: none; }

                .tag-item {
                    display: inline-flex;
                    align-items: center;
                    gap: 6px;
                    height: 28px;
                    margin-right: 6px;
                    padding: 0 10px;
                    border: 1px solid var(--border);
                    border-radius: 999px;
                    background: var(--bg-element);
                    cursor: pointer;
                    white-space: nowrap;
                    transition: all 0.16s ease;
                }
                .tag-item:last-child { margin-right: 0; }
                .tag-item:hover { border-color: rgba(var(--accent-color-rgb), 0.2); }
                .tag-item.active { background: var(--accent-color); border-color: var(--accent-color); color: #ffffff; }
                .tag-item.create-hint { border-style: dashed; border-color: var(--line-soft); color: var(--text-secondary); }
                .tag-item.create-hint:hover { border-style: solid; }

                .tag-color-wrapper { width: 8px; flex-shrink: 0; display: flex; align-items: center; justify-content: center; }
                .tag-color-dot { width: 7px; height: 7px; border-radius: 999px; cursor: pointer; border: none; transition: transform 0.16s ease; box-shadow: inset 0 0 0 1px rgba(255,255,255,0.35), 0 0 0 1px rgba(24,34,53,0.12); }
                .tag-color-dot:hover { transform: scale(1.2); }
                .tag-name { font-size: 12px; font-weight: 500; }
                .inline-tag-edit { width: 100px; height: 22px; padding: 0 6px; border: var(--input-border); border-radius: var(--input-radius); background: var(--bg-input); box-shadow: var(--input-shadow); color: var(--text-primary); font-size: 12px; font-weight: 500; outline: none; }
                .inline-tag-edit:focus { border-color: var(--input-focus-border-color); box-shadow: var(--input-focus-shadow); }
                .tag-hover-actions { display: flex; align-items: center; gap: 2px; color: var(--text-secondary); opacity: 0; pointer-events: none; transition: opacity 0.16s ease; }
                .tag-item:hover .tag-hover-actions, .tag-item.active .tag-hover-actions { opacity: 1; pointer-events: auto; }
                .tag-hover-actions > span { width: 16px; height: 16px; display: inline-flex; align-items: center; justify-content: center; border-radius: 4px; }
                .tag-hover-actions > span:hover { background: rgba(var(--accent-color-rgb), 0.12); color: var(--accent-color); }
                .tag-badge { margin-left: 4px; min-width: 18px; height: 16px; padding: 0 4px; display: inline-flex; align-items: center; justify-content: center; border-radius: 999px; background: rgba(var(--accent-color-rgb), 0.08); color: var(--text-secondary); font-size: 10px; font-weight: 700; }
                .tag-item.active .tag-badge { background: rgba(255,255,255,0.25); color: #ffffff; }
                .sidebar-status { padding: 12px 10px; text-align: center; color: var(--text-secondary); font-size: 11px; }

                .tag-content { flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; border: var(--panel-border); border-radius: var(--radius-md); background: var(--bg-panel); box-shadow: 0 2px 8px var(--shadow); }
                .content-toolbar { min-height: 40px; padding: 6px 12px; display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 8px; border-bottom: 1px solid var(--panel-divider-color); background: transparent; }
                .toolbar-left, .toolbar-right { display: flex; align-items: center; gap: 6px; min-width: 0; }
                .toolbar-left { overflow: hidden; }
                .toolbar-right { flex-shrink: 0; }
                .selected-tag-indicator { display: inline-flex; align-items: center; gap: 4px; min-width: 0; max-width: min(100%, 160px); padding: 4px 8px; border-radius: 999px; background: rgba(var(--accent-color-rgb), 0.08); color: var(--text-primary); font-size: 12px; font-weight: 600; }
                .breadcrumb-marker { color: var(--accent-color); }
                .breadcrumb-text { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
                .toolbar-divider { width: 1px; height: 16px; background: var(--panel-divider-color); }
                .sort-group { display: flex; align-items: center; gap: 4px; }
                .sort-btn, .toggle-btn { border: var(--button-border); background: var(--bg-button); color: var(--text-secondary); box-shadow: var(--button-shadow); }
                .sort-btn { height: 26px; width: 26px; padding: 0; display: inline-flex; align-items: center; justify-content: center; gap: 0; border-radius: var(--button-radius); font-size: 11px; font-weight: 600; cursor: pointer; transition: all 0.18s ease; }
                .sort-btn:hover, .toggle-btn:hover { background: var(--bg-input); border-color: var(--button-hover-border-color); color: var(--text-primary); }
                .sort-btn.active, .toggle-btn.active { background: var(--accent-color); border-color: var(--accent-color); color: #ffffff; box-shadow: var(--button-active-filled-shadow); }
                .view-toggle { display: flex; align-items: center; gap: 2px; padding: 2px; border: var(--button-border); border-radius: calc(var(--button-radius) + 2px); background: var(--bg-input); }
                .toggle-btn { width: 24px; height: 24px; padding: 0; display: inline-flex; align-items: center; justify-content: center; border-radius: var(--button-radius); cursor: pointer; transition: all 0.18s ease; }
                .add-item-btn { height: 26px; width: 26px; padding: 0; gap: 0; font-size: 11px; font-weight: 600; box-shadow: var(--button-shadow); }

                .items-area { flex: 1; min-height: 0; overflow-y: auto; padding: 8px; background: transparent; }
                .status-msg { padding: 16px 8px; text-align: center; color: var(--text-secondary); font-size: 12px; }
                .items-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(130px, 1fr)); gap: 8px; align-content: flex-start; }
                .items-list { display: grid; grid-template-columns: 1fr; gap: 6px; }

                .themed-card { position: relative; min-height: 100px; display: flex; flex-direction: column; padding: 8px; border: var(--card-border); border-radius: var(--card-radius); background: var(--bg-element); box-shadow: var(--card-shadow); cursor: pointer; overflow: hidden; transition: all 0.18s ease; }
                .themed-card:hover { background: var(--data-panel-hover-background); border-color: var(--card-hover-border-color); box-shadow: var(--card-hover-shadow); }
                .items-list .themed-card { min-height: 80px; }
                .card-top-row { position: static; display: flex; align-items: center; justify-content: space-between; gap: 4px; margin-bottom: 4px; opacity: 1; }
                .card-actions-left { display: flex; align-items: center; gap: 3px; }
                .card-action-btn, .del-btn { width: 20px; height: 20px; padding: 0; display: inline-flex; align-items: center; justify-content: center; border: var(--button-border); border-radius: var(--button-radius); background: transparent; color: var(--text-secondary); box-shadow: none; opacity: 0.8; cursor: pointer; transition: all 0.16s ease; }
                .card-action-btn:hover, .del-btn:hover { background: var(--bg-input); border-color: var(--button-hover-border-color); color: var(--accent-color); }
                .del-btn:hover { color: #ff4d4f; }
                .card-media { flex: 1; min-height: 60px; display: flex; align-items: center; justify-content: center; overflow: hidden; border-radius: var(--data-panel-radius); background: var(--bg-input); }
                .card-media img { max-width: 100%; max-height: 90px; object-fit: contain; }
                .card-body-text { flex: 1; color: var(--text-primary); font-size: 11px; font-weight: 500; line-height: 1.45; word-break: break-word; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 3; overflow: hidden; min-height: 0; }
                .items-list .card-body-text { -webkit-line-clamp: 2; }
                .card-divider { height: 1px; margin: 6px 0 4px; background: var(--panel-divider-color); }
                .card-footer { margin-top: auto; display: flex; align-items: center; justify-content: space-between; gap: 4px; color: var(--text-secondary); font-size: 10px; font-weight: 600; }
                .meta-usage { display: inline-flex; align-items: center; gap: 3px; }

                .modal-overlay { position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.4); backdrop-filter: blur(4px); display: flex; align-items: center; justify-content: center; z-index: 2000; }
                .modal-overlay .confirm-dialog { background: var(--bg-panel); padding: 16px; border: var(--modal-border); box-shadow: var(--modal-shadow); border-radius: var(--modal-radius); width: min(360px, 100%); max-width: 90%; animation: modal-pop 0.2s cubic-bezier(0.34, 1.56, 0.64, 1); }
                @keyframes modal-pop { 0% { transform: scale(0.95); opacity: 0; } 100% { transform: scale(1); opacity: 1; } }
                .modal-overlay .confirm-dialog h3 { margin: 0 0 10px; padding: 0; background: transparent; color: var(--text-primary); font-size: 14px; font-weight: 600; text-transform: none; }
                .modal-overlay .confirm-dialog p { margin: 8px 0 16px; color: var(--text-secondary); font-size: 12px; line-height: 1.5; }
                .modal-overlay .confirm-dialog-buttons { display: flex; justify-content: flex-end; gap: 6px; }
                .modal-overlay .confirm-dialog-button { padding: 6px 12px; font-size: 12px; font-weight: 600; cursor: pointer; background: var(--bg-button); border: var(--dialog-button-border); color: var(--text-primary); box-shadow: var(--dialog-button-shadow); transition: all 0.18s ease; border-radius: var(--dialog-button-radius); }
                .modal-overlay .confirm-dialog-button:hover { background: var(--bg-input); box-shadow: var(--dialog-button-hover-shadow); }
                .modal-overlay .confirm-dialog-button.primary { background: var(--accent-color); color: #ffffff; border-color: var(--accent-color); }
                .modal-overlay .confirm-dialog-button.primary:hover { background: var(--accent-hover); }

                .tag-manager-textarea { width: 100%; min-height: 100px; margin-bottom: 12px; padding: 8px; border: var(--input-border); border-radius: var(--input-radius); background: var(--bg-input); box-shadow: var(--input-shadow); color: var(--text-primary); font-family: inherit; font-size: 12px; line-height: 1.5; outline: none; resize: vertical; }
                .tag-manager-textarea:focus { border-color: var(--input-focus-border-color); box-shadow: var(--input-focus-shadow); }

                .custom-scrollbar::-webkit-scrollbar { width: var(--scrollbar-size-thin); }
                .custom-scrollbar::-webkit-scrollbar-thumb { background: var(--scrollbar-thumb-color); border-radius: var(--scrollbar-radius); }
            `}</style>
        </div >
    );
}
