import { useEffect, useState, useRef, useCallback } from "react";
import { getCurrentWindow, PhysicalSize, PhysicalPosition } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { ClipboardPaste, Pin, PinOff, X } from "lucide-react";
import type { StickyEntry } from "../../../shared/types/sticky";
import { StickyManager } from "../StickyManager";

interface ContextMenuState {
    visible: boolean;
    x: number;
    y: number;
    maxHeight: number;
}

function toImageSrc(content: string): string {
    if (content.startsWith("data:")) return content;
    if (content.startsWith("http://") || content.startsWith("https://")) return content;
    if (content.match(/^[A-Za-z]:[\\/]/)) {
        return `https://asset.localhost/${encodeURIComponent(content)}`;
    }
    return content;
}

export default function StickyWindow() {
    const params = new URLSearchParams(window.location.search);
    const idParam = params.get("id");
    const stickyId = idParam ? parseInt(idParam, 10) : null;

    const [entry, setEntry] = useState<StickyEntry | null>(null);
    const [isAlwaysOnTop, setIsAlwaysOnTop] = useState(false);
    const [contextMenu, setContextMenu] = useState<ContextMenuState>({ visible: false, x: 0, y: 0, maxHeight: 200 });
    const contextMenuRef = useRef<HTMLDivElement | null>(null);
    const containerRef = useRef<HTMLDivElement | null>(null);
    const zoomScaleRef = useRef(1);
    const [showToolbar, setShowToolbar] = useState(false);
    const [pasteFeedback, setPasteFeedback] = useState(false);
    const [saveError, setSaveError] = useState(false);
    const saveErrorTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    // Text stickies are directly editable: the draft mirrors entry.content
    // and is flushed to the DB on blur / Ctrl+Enter.
    const [draftContent, setDraftContent] = useState("");
    const toolbarTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
    const moveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const resizeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    // Latest DB values, read inside the debounced handlers without re-binding
    // the native event listeners on every save.
    const entryRef = useRef<StickyEntry | null>(null);
    useEffect(() => { entryRef.current = entry; }, [entry]);

    // Apply body styling on mount — transparent, allow shadow overflow
    useEffect(() => {
        const body = document.body;
        body.style.margin = "0";
        body.style.padding = "0";
        body.style.background = "transparent";
        body.style.overflow = "visible";
        body.style.borderRadius = "0";
        body.classList.add("sticky-window", "theme-fluent");

        // Read theme from settings (key is "app.color_mode" in the HashMap)
        invoke<Record<string, string>>("get_settings")
            .then((settings) => {
                const mode = settings["app.color_mode"] || "dark";
                document.body.classList.add(mode === "light" ? "light-mode" : "dark-mode");
            })
            .catch(() => document.body.classList.add("dark-mode"));

        const root = document.getElementById("root");
        if (root) {
            root.style.background = "transparent";
            root.style.overflow = "visible";
            root.style.borderRadius = "0";
            root.style.display = "flex";
            root.style.width = "100vw";
            root.style.height = "100vh";
        }

        return () => {
            document.body.classList.remove("sticky-window", "theme-fluent", "light-mode", "dark-mode");
        };
    }, []);

    // Listen for real-time theme changes from main app
    useEffect(() => {
        const unlisten = listen<{ colorMode: string }>("theme-changed", (event) => {
            const { colorMode } = event.payload;
            document.body.classList.remove("light-mode", "dark-mode");
            document.body.classList.add(colorMode === "light" ? "light-mode" : "dark-mode");
        });
        return () => { unlisten.then((u) => u()); };
    }, []);

    // Load sticky data
    useEffect(() => {
        if (stickyId === null || isNaN(stickyId)) return;
        invoke("get_sticky", { id: stickyId })
            .then((data: any) => {
                if (data) {
                    setEntry(data);
                    setDraftContent(data.content);
                    setIsAlwaysOnTop(data.always_on_top);
                } else {
                    // Entry was deleted (e.g. clear-all left a stale window) — close self
                    emit("sticky-closed", { id: stickyId }).catch(() => {});
                    getCurrentWindow().close().catch(() => {});
                }
            })
            .catch(console.error);
    }, [stickyId]);

    // Resize window to fit content after entry loads
    useEffect(() => {
        if (!entry) return;

        const resize = async () => {
            try {
                const win = getCurrentWindow();
                const screenW = window.screen.availWidth || window.screen.width;
                const screenH = window.screen.availHeight || window.screen.height;
                const maxW = Math.floor(screenW / 2);
                const maxH = Math.floor(screenH * 0.8);
                const isImage = entry.content_type === "image";

                if (isImage) {
                    const img = new Image();
                    img.onload = async () => {
                        const scale = Math.min(maxW / img.naturalWidth, maxH / img.naturalHeight, 1);
                        const w = Math.max(200, Math.round(img.naturalWidth * scale) + 24);
                        const h = Math.max(160, Math.round(img.naturalHeight * scale) + 52);
                        await win.setSize(new PhysicalSize(w, h));
                    };
                    img.onerror = async () => {
                        await win.setSize(new PhysicalSize(400, 300));
                    };
                    img.src = toImageSrc(entry.content);
                } else {
                    // Measure natural width of longest line (nowrap)
                    const lines = entry.content.split("\n");
                    const longestLine = lines.reduce((a, b) => (a.length > b.length ? a : b), "");

                    const measurer = document.createElement("div");
                    measurer.style.cssText = "position:absolute;visibility:hidden;white-space:nowrap;font-size:14px;line-height:1.6;font-family:inherit;";
                    measurer.textContent = longestLine || entry.content;
                    document.body.appendChild(measurer);
                    const naturalW = measurer.scrollWidth;
                    document.body.removeChild(measurer);

                    const contentW = naturalW + 28;
                    const w = Math.max(200, Math.min(contentW, maxW));

                    // Measure actual height at chosen width
                    const heightMeasurer = document.createElement("div");
                    heightMeasurer.style.cssText = `position:absolute;visibility:hidden;font-size:14px;line-height:1.6;font-family:inherit;width:${w - 28}px;white-space:pre-wrap;word-break:break-word;`;
                    heightMeasurer.textContent = entry.content;
                    document.body.appendChild(heightMeasurer);
                    const contentH = heightMeasurer.scrollHeight;
                    document.body.removeChild(heightMeasurer);

                    const h = Math.max(120, contentH + 52);

                    await win.setSize(new PhysicalSize(w, h));
                }
            } catch (e) {
                console.error("Failed to auto-resize sticky:", e);
            }
        };

        resize();
    }, [entry]);

    // Persist window position/size via native events (debounced) — reliable
    // even when the mouse is released outside the window, unlike onMouseUp.
    useEffect(() => {
        if (stickyId === null || isNaN(stickyId)) return;
        const win = getCurrentWindow();
        const unlistenMoved = win.onMoved((event) => {
            if (moveTimerRef.current) clearTimeout(moveTimerRef.current);
            moveTimerRef.current = setTimeout(() => {
                // Programmatic setPosition (restore/create) also fires
                // onMoved — skip the write when the position already equals
                // what we have in the DB.
                const cur = entryRef.current;
                if (cur && Math.round(cur.x) === Math.round(event.payload.x) && Math.round(cur.y) === Math.round(event.payload.y)) return;
                StickyManager.updatePosition(stickyId, event.payload.x, event.payload.y);
            }, 300);
        });
        const unlistenResized = win.onResized((event) => {
            if (resizeTimerRef.current) clearTimeout(resizeTimerRef.current);
            resizeTimerRef.current = setTimeout(() => {
                const cur = entryRef.current;
                if (cur && Math.round(cur.width) === Math.round(event.payload.width) && Math.round(cur.height) === Math.round(event.payload.height)) return;
                StickyManager.updateSize(stickyId, event.payload.width, event.payload.height);
            }, 300);
        });
        return () => {
            unlistenMoved.then((u) => u());
            unlistenResized.then((u) => u());
            if (moveTimerRef.current) clearTimeout(moveTimerRef.current);
            if (resizeTimerRef.current) clearTimeout(resizeTimerRef.current);
        };
    }, [stickyId]);

    // Flush the editable draft to the DB when it changed. UI state and the
    // "sticky-updated" event are only updated after a successful write — on
    // failure the draft stays in the textarea and a short error hint appears.
    const saveDraft = useCallback(async () => {
        if (stickyId === null || isNaN(stickyId)) return;
        if (!entry || entry.content === draftContent) return; // nothing changed
        try {
            await StickyManager.updateContent(stickyId, draftContent);
            setEntry((prev) => (prev ? { ...prev, content: draftContent } : prev));
            // Notify the main app so the sticky panel shows the updated content
            emit("sticky-updated", { id: stickyId }).catch(() => {});
        } catch (err) {
            console.error("Save sticky edit failed:", err);
            setSaveError(true);
            if (saveErrorTimerRef.current) clearTimeout(saveErrorTimerRef.current);
            saveErrorTimerRef.current = setTimeout(() => setSaveError(false), 3000);
        }
    }, [stickyId, entry, draftContent]);

    // Paste: copy the current (possibly just-edited) content to clipboard,
    // persist any pending edit, then hide sticky and simulate paste keystroke
    const handlePaste = useCallback(async () => {
        if (!entry || stickyId === null || isNaN(stickyId)) return;
        try {
            // First copy content to system clipboard
            await invoke("copy_to_clipboard", {
                content: draftContent,
                contentType: entry.content_type,
                paste: false,
                id: 0,
                deleteAfterUse: false,
                pasteWithFormat: false,
                moveToTop: false,
            });
            await saveDraft();
            // Then hide sticky, paste, and show again
            await invoke("paste_sticky_content", { id: stickyId });
            setPasteFeedback(true);
            setTimeout(() => setPasteFeedback(false), 300);
        } catch (err) {
            console.error("Paste failed:", err);
        }
    }, [entry, draftContent, saveDraft, stickyId]);

    const handleOpen = useCallback(async () => {
        if (!entry) return;
        try {
            await invoke("open_content", {
                content: entry.content,
                contentType: entry.content_type,
                id: 0,
            });
        } catch (err) {
            console.error("Open failed:", err);
        }
    }, [entry]);

    const toggleAlwaysOnTop = useCallback(async () => {
        if (stickyId === null || isNaN(stickyId)) return;
        const newValue = !isAlwaysOnTop;
        setIsAlwaysOnTop(newValue);
        await getCurrentWindow().setAlwaysOnTop(newValue);
        await StickyManager.updateAlwaysOnTop(stickyId, newValue);
    }, [stickyId, isAlwaysOnTop]);

    const handleClose = useCallback(async () => {
        if (stickyId !== null && !isNaN(stickyId)) {
            // Delete first, then notify the main app so its list refresh
            // does not race with (and show) a stale entry
            try { await invoke("delete_sticky", { id: stickyId }); } catch (_) {}
            emit("sticky-closed", { id: stickyId }).catch(() => {});
            try { await invoke("close_sticky_window", { id: stickyId }); } catch (_) {}
        }
        try { await getCurrentWindow().close(); } catch (_) {}
    }, [stickyId]);

    const handleContextMenu = useCallback((e: React.MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        const menuWidth = 160;
        const vw = document.documentElement.clientWidth;
        const vh = document.documentElement.clientHeight;
        const menuHeight = 130;
        const x = Math.max(0, Math.min(e.clientX, vw - menuWidth));
        const y = Math.max(0, Math.min(e.clientY, vh - menuHeight));
        const maxHeight = vh - 8;
        setContextMenu({ visible: true, x, y, maxHeight });
    }, []);

    useEffect(() => {
        if (!contextMenu.visible) return;
        const handleClick = (e: MouseEvent) => {
            if (contextMenuRef.current && !contextMenuRef.current.contains(e.target as Node)) {
                setContextMenu({ visible: false, x: 0, y: 0, maxHeight: 200 });
            }
        };
        document.addEventListener("mousedown", handleClick);
        return () => document.removeEventListener("mousedown", handleClick);
    }, [contextMenu.visible]);

    // Ctrl+滚轮缩放窗口，缩放中心为光标坐标
    useEffect(() => {
        const container = containerRef.current;
        if (!container) return;

        const handleWheel = async (e: WheelEvent) => {
            if (!e.ctrlKey) return;
            e.preventDefault();
            e.stopPropagation();

            const MIN_SCALE = 0.5;
            const MAX_SCALE = 3.0;
            const STEP = 0.08;

            const oldScale = zoomScaleRef.current;
            const delta = e.deltaY > 0 ? -STEP : STEP;
            const newScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, oldScale + delta));
            if (Math.abs(newScale - oldScale) < 0.001) return;
            zoomScaleRef.current = newScale;

            const win = getCurrentWindow();
            const oldSize = await win.outerSize().catch(() => null);
            const oldPos = await win.outerPosition().catch(() => null);
            if (!oldSize || !oldPos) return;

            const newW = Math.round(oldSize.width * (newScale / oldScale));
            const newH = Math.round(oldSize.height * (newScale / oldScale));

            // 光标屏幕坐标
            const cursorScreenX = e.screenX;
            const cursorScreenY = e.screenY;
            // 光标在窗口内的相对比例
            const ratioX = (cursorScreenX - oldPos.x) / oldSize.width;
            const ratioY = (cursorScreenY - oldPos.y) / oldSize.height;

            const newPosX = Math.round(cursorScreenX - ratioX * newW);
            const newPosY = Math.round(cursorScreenY - ratioY * newH);

            try {
                await win.setSize(new PhysicalSize(newW, newH));
                await win.setPosition(new PhysicalPosition(newPosX, newPosY));
            } catch (err) {
                console.error("Sticky zoom failed:", err);
            }
        };

        container.addEventListener("wheel", handleWheel, { passive: false });
        return () => container.removeEventListener("wheel", handleWheel);
    }, [entry]);

    const handleMouseEnter = useCallback(() => {
        if (toolbarTimer.current) clearTimeout(toolbarTimer.current);
        setShowToolbar(true);
    }, []);

    const handleMouseLeave = useCallback(() => {
        toolbarTimer.current = setTimeout(() => setShowToolbar(false), 600);
    }, []);

    const handleTextKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
        if (e.key === "Enter" && e.ctrlKey) {
            e.preventDefault();
            saveDraft();
        }
    }, [saveDraft]);

    if (!entry) {
        return (
            <div style={{
                width: "100vw", height: "100vh",
                display: "flex", alignItems: "center", justifyContent: "center",
                background: "transparent",
            }}>
                <div style={{
                    width: 32, height: 32,
                    border: "3px solid rgba(128,128,128,0.2)",
                    borderTopColor: "var(--accent-color, #0078d4)",
                    borderRadius: "0%",
                    animation: "spin 0.8s linear infinite",
                }} />
                <style>{`@keyframes spin { to { transform: rotate(360deg); } }`}</style>
            </div>
        );
    }

    const isImage = entry.content_type === "image";
    const btnBase = {
        width: 24, height: 24,
        border: "none", borderRadius: 4,
        background: "var(--bg-button, rgba(128,128,128,0.1))",
        color: "var(--text-secondary, #666)",
        cursor: "pointer",
        display: "flex", alignItems: "center", justifyContent: "center",
        fontSize: 14,
        lineHeight: 1,
    };

    return (
        <div
            ref={containerRef}
            className="sticky-container"
            onContextMenu={handleContextMenu}
            onMouseEnter={handleMouseEnter}
            onMouseLeave={handleMouseLeave}
            onMouseDown={(e) => {
                if (e.button === 0 && e.target === e.currentTarget) {
                    getCurrentWindow().startDragging().catch(() => {});
                }
            }}
            style={{
                width: "100vw",
                height: "100vh",
                margin: 0,
                padding: 0,
                overflow: "visible",
                display: "flex",
                flexDirection: "column",
                borderRadius: 0,
                boxShadow: "0 4px 24px rgba(0,0,0,0.30)",
                border: "1px solid var(--line-soft, rgba(128,128,128,0.15))",
                backgroundColor: "var(--bg-window, #ffffff)",
            }}
        >
            <div
                style={{
                    position: "absolute",
                    top: 0, left: 0, right: 0, height: 36,
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "flex-end",
                    padding: "4px 6px",
                    gap: 4,
                    opacity: showToolbar ? 1 : 0,
                    transition: "opacity 0.2s",
                    background: showToolbar
                        ? "linear-gradient(to bottom, rgba(0,0,0,0.06), transparent)"
                        : "transparent",
                    zIndex: 10,
                }}
                onMouseDown={(e) => {
                    if (e.button === 0 && e.target === e.currentTarget) {
                        getCurrentWindow().startDragging().catch(() => {});
                    }
                }}
            >
                <button className="sticky-toolbar-btn" onMouseDown={(e) => e.stopPropagation()} onClick={(e) => { e.stopPropagation(); handlePaste(); }} title="粘贴到光标处" style={{ ...btnBase, color: pasteFeedback ? "var(--accent-color, #0078d4)" : btnBase.color }}><ClipboardPaste size={14} /></button>
                <button className="sticky-toolbar-btn" onMouseDown={(e) => e.stopPropagation()} onClick={(e) => { e.stopPropagation(); toggleAlwaysOnTop(); }} title={isAlwaysOnTop ? "取消置顶" : "置顶"} style={{ ...btnBase, background: isAlwaysOnTop ? "var(--accent-color, #0078d4)" : btnBase.background, color: isAlwaysOnTop ? "#fff" : btnBase.color }}>{isAlwaysOnTop ? <PinOff size={14} /> : <Pin size={14} />}</button>
                <button className="sticky-toolbar-btn" onMouseDown={(e) => e.stopPropagation()} onClick={(e) => { e.stopPropagation(); handleClose(); }} title="关闭" style={btnBase}><X size={14} /></button>
            </div>

            <div
                className="sticky-content"
                style={{
                    flex: 1, overflow: "hidden",
                    padding: "40px 12px 12px 12px",
                    display: "flex",
                    alignItems: isImage ? "center" : "flex-start",
                    justifyContent: isImage ? "center" : "flex-start",
                }}
                onDoubleClick={() => (isImage ? handleOpen() : undefined)}
            >
                {isImage ? (
                    <img src={toImageSrc(entry.content)} alt="Sticky" style={{ maxWidth: "100%", maxHeight: "100%", objectFit: "contain", borderRadius: 0, userSelect: "none", pointerEvents: "none" }} draggable={false} />
                ) : (
                    // Text stickies are directly editable — changes are persisted on blur / Ctrl+Enter
                    <textarea
                        value={draftContent}
                        onChange={(e) => setDraftContent(e.target.value)}
                        onKeyDown={handleTextKeyDown}
                        onBlur={saveDraft}
                        spellCheck={false}
                        style={{
                            flex: 1,
                            width: "100%",
                            height: "100%",
                            border: "none",
                            outline: "none",
                            resize: "none",
                            background: "transparent",
                            overflow: "auto",
                            whiteSpace: "pre-wrap",
                            wordBreak: "break-word",
                            fontSize: 14,
                            lineHeight: 1.6,
                            fontFamily: "inherit",
                            color: "var(--text-primary, #1a1a1a)",
                            padding: 0,
                        }}
                    />
                )}
            </div>

            {/* 右键菜单 — Windows 11 风格 */}
            {contextMenu.visible && (
                <div
                    ref={contextMenuRef}
                    className="sticky-context-menu"
                    style={{
                        position: "fixed",
                        left: contextMenu.x,
                        top: contextMenu.y,
                        zIndex: 9999,
                        minWidth: 160,
                        background: "rgba(242, 242, 242, 0.99)",
                        backdropFilter: "blur(20px) saturate(180%)",
                        WebkitBackdropFilter: "blur(20px) saturate(180%)",
                        border: "1px solid rgba(0, 0, 0, 0.08)",
                        borderRadius: 2,
                        boxShadow: "0 8px 32px rgba(0,0,0,0.12), 0 2px 8px rgba(0,0,0,0.06)",
                        padding: "0px",
                        overflowY: "auto",
                        maxHeight: contextMenu.maxHeight,
                    }}
                >
                    {[
                        { icon: "📋", label: "粘贴", action: handlePaste, color: "var(--text-primary, #1a1a1a)" },
                        { icon: isAlwaysOnTop ? "📍" : "📌", label: isAlwaysOnTop ? "取消置顶" : "置顶", action: toggleAlwaysOnTop, color: "var(--text-primary, #1a1a1a)" },
                    ].map((item) => (
                        <div
                            key={item.label}
                            onMouseDown={(e) => e.stopPropagation()}
                            onClick={() => { setContextMenu({ visible: false, x: 0, y: 0, maxHeight: 200 }); item.action(); }}
                            className="sticky-context-menu-item"
                            style={{ padding: "7px 12px", fontSize: 13, cursor: "pointer", display: "flex", alignItems: "center", gap: 10, color: item.color, borderRadius: 4, margin: "0 2px" }}
                        >
                            <span style={{ width: 20, textAlign: "center", fontSize: 14, flexShrink: 0 }}>{item.icon}</span>
                            <span>{item.label}</span>
                        </div>
                    ))}
                    <div style={{ height: 1, background: "rgba(0, 0, 0, 0.08)", margin: "4px 8px" }} />
                    <div
                        onMouseDown={(e) => e.stopPropagation()}
                        onClick={() => { setContextMenu({ visible: false, x: 0, y: 0, maxHeight: 200 }); handleClose(); }}
                        className="sticky-context-menu-item"
                        style={{ padding: "7px 12px", fontSize: 13, cursor: "pointer", display: "flex", alignItems: "center", gap: 10, color: "var(--text-secondary, #5d5d5d)", borderRadius: 4, margin: "0 2px" }}
                    >
                        <span style={{ width: 20, textAlign: "center", fontSize: 14, flexShrink: 0 }}>✕</span>
                        <span>关闭</span>
                    </div>
                </div>
            )}

            {saveError && (
                <div style={{
                    position: "absolute",
                    bottom: 10, left: 12, right: 12,
                    padding: "6px 10px",
                    borderRadius: 6,
                    fontSize: 12,
                    background: "rgba(200, 60, 60, 0.12)",
                    border: "1px solid rgba(200, 60, 60, 0.35)",
                    color: "var(--text-error, #c0392b)",
                    textAlign: "center",
                }}>
                    保存失败，内容未写入
                </div>
            )}
        </div>
    );
}
