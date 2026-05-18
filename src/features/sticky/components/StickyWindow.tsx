import { useEffect, useState, useRef, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import type { StickyEntry } from "../../../shared/types/sticky";
import { StickyManager } from "../StickyManager";

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
    const [showToolbar, setShowToolbar] = useState(false);
    const [pasteFeedback, setPasteFeedback] = useState(false);
    const toolbarTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

    // Apply body styling on mount — transparent, rounded, no overflow
    useEffect(() => {
        const body = document.body;
        body.style.margin = "0";
        body.style.padding = "0";
        body.style.background = "transparent";
        body.style.overflow = "hidden";
        body.style.borderRadius = "12px";
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
            root.style.overflow = "hidden";
            root.style.borderRadius = "12px";
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
                    setIsAlwaysOnTop(data.always_on_top);
                }
            })
            .catch(console.error);
    }, [stickyId]);

    // Paste: copy to clipboard first, then hide sticky and simulate paste keystroke
    const handlePaste = useCallback(async () => {
        if (!entry || stickyId === null || isNaN(stickyId)) return;
        try {
            // First copy content to system clipboard
            await invoke("copy_to_clipboard", {
                content: entry.content,
                contentType: entry.content_type,
                paste: false,
                id: 0,
                deleteAfterUse: false,
                pasteWithFormat: false,
                moveToTop: false,
            });
            // Then hide sticky, paste, and show again
            await invoke("paste_sticky_content", { id: stickyId });
            setPasteFeedback(true);
            setTimeout(() => setPasteFeedback(false), 300);
        } catch (err) {
            console.error("Paste failed:", err);
        }
    }, [entry, stickyId]);

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
            // Notify main app so the sticky panel refreshes
            emit("sticky-closed", { id: stickyId }).catch(() => {});
            try { await invoke("delete_sticky", { id: stickyId }); } catch (_) {}
        }
        try { await getCurrentWindow().close(); } catch (_) {}
        if (stickyId !== null && !isNaN(stickyId)) {
            try { await invoke("close_sticky_window", { id: stickyId }); } catch (_) {}
        }
    }, [stickyId]);

    const handleMouseEnter = useCallback(() => {
        if (toolbarTimer.current) clearTimeout(toolbarTimer.current);
        setShowToolbar(true);
    }, []);

    const handleMouseLeave = useCallback(() => {
        toolbarTimer.current = setTimeout(() => setShowToolbar(false), 600);
    }, []);

    const handleDragEnd = useCallback(async () => {
        if (stickyId === null || isNaN(stickyId)) return;
        try {
            const pos = await getCurrentWindow().outerPosition();
            await StickyManager.updatePosition(stickyId, pos.x, pos.y);
        } catch {
            // Window may have been closed; ignore
        }
    }, [stickyId]);

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
                    borderRadius: "50%",
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
            className="sticky-container"
            onMouseEnter={handleMouseEnter}
            onMouseLeave={handleMouseLeave}
            style={{
                width: "100vw",
                height: "100vh",
                margin: 0,
                padding: 0,
                overflow: "hidden",
                display: "flex",
                flexDirection: "column",
                borderRadius: 12,
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
                <button className="sticky-toolbar-btn" onMouseDown={(e) => e.stopPropagation()} onClick={(e) => { e.stopPropagation(); handlePaste(); }} title="粘贴到光标处" style={{ ...btnBase, color: pasteFeedback ? "var(--accent-color, #0078d4)" : btnBase.color }}>📋</button>
                <button className="sticky-toolbar-btn" onMouseDown={(e) => e.stopPropagation()} onClick={(e) => { e.stopPropagation(); toggleAlwaysOnTop(); }} title={isAlwaysOnTop ? "取消置顶" : "置顶"} style={{ ...btnBase, background: isAlwaysOnTop ? "var(--accent-color, #0078d4)" : btnBase.background, color: isAlwaysOnTop ? "#fff" : btnBase.color }}>📌</button>
                <button className="sticky-toolbar-btn" onMouseDown={(e) => e.stopPropagation()} onClick={(e) => { e.stopPropagation(); handleClose(); }} title="关闭" style={{ ...btnBase, fontSize: 16 }}>✕</button>
            </div>

            <div
                style={{
                    flex: 1, overflow: "auto",
                    padding: "40px 12px 12px 12px",
                    display: "flex",
                    alignItems: isImage ? "center" : "flex-start",
                    justifyContent: isImage ? "center" : "flex-start",
                }}
                onMouseUp={handleDragEnd}
                onDoubleClick={() => handleOpen()}
            >
                {isImage ? (
                    <img src={toImageSrc(entry.content)} alt="Sticky" style={{ maxWidth: "100%", maxHeight: "100%", objectFit: "contain", borderRadius: 6, userSelect: "none", pointerEvents: "none" }} draggable={false} />
                ) : (
                    <div style={{ whiteSpace: "pre-wrap", wordBreak: "break-word", fontSize: 14, lineHeight: 1.6, color: "var(--text-primary, #1a1a1a)", userSelect: "text", width: "100%" }}>{entry.content}</div>
                )}
            </div>
        </div>
    );
}
