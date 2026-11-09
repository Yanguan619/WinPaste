import { useEffect, useMemo, useState, useRef, useCallback } from "react";
import { emitTo, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize, currentMonitor } from "@tauri-apps/api/window";
import {
    FileText,
    Image as ImageIcon,
    Link as LinkIcon,
    Code,
    File,
    Video,
    AppWindow,
    Clock,
    Copy,
    ExternalLink,
    X
} from "lucide-react";
import HtmlContent from "../../../shared/components/HtmlContent";
import { getConciseTime } from "../../../shared/lib/utils";
import type { Locale } from "../../../shared/types";
import { toTauriLocalImageSrc } from "../../../shared/lib/localImageSrc";
import { getRichTextSnapshotDataUrl } from "../../../shared/lib/richTextSnapshot";

type PreviewPayload = {
    contentType: string;
    content: string;
    preview?: string;
    htmlContent?: string;
    sourceApp?: string;
    timestamp?: number;
    language?: Locale;
    theme?: string;
    colorMode?: "light" | "dark";
    richTextSnapshotPreview?: boolean;
    clipboardItemFontSize?: number;
    clipboardTagFontSize?: number;
};

type ContextMenuState = {
    visible: boolean;
    x: number;
    y: number;
    maxHeight: number;
};

const RICH_IMAGE_FALLBACK_PREFIX = "<!--WINPASTE_RICH_IMAGE:";
const RICH_IMAGE_FALLBACK_SUFFIX = "-->";

const extractRichImageFallback = (html?: string): { cleanHtml?: string; imagePayload?: string } => {
    if (!html) return {};
    const start = html.lastIndexOf(RICH_IMAGE_FALLBACK_PREFIX);
    if (start < 0) return { cleanHtml: html };

    const markerStart = start + RICH_IMAGE_FALLBACK_PREFIX.length;
    const endRel = html.slice(markerStart).indexOf(RICH_IMAGE_FALLBACK_SUFFIX);
    if (endRel < 0) return { cleanHtml: html };

    const markerEnd = markerStart + endRel;
    const payload = html.slice(markerStart, markerEnd).trim();
    const cleanHtml = `${html.slice(0, start)}${html.slice(markerEnd + RICH_IMAGE_FALLBACK_SUFFIX.length)}`.trim();
    return {
        cleanHtml: cleanHtml || html,
        imagePayload: payload || undefined
    };
};

const resolveRichImageSrc = (payload: string): string | null => {
    const value = payload.trim();
    if (!value) return null;
    if (value.startsWith("data:image/")) return value;
    if (/^https?:\/\/asset\.localhost\//i.test(value)) return value;
    return toTauriLocalImageSrc(value);
};

const RICH_PREVIEW_DEBUG = import.meta.env.DEV;
const richPreviewFailureLog = (stage: string, detail?: Record<string, unknown>) => {
    if (!RICH_PREVIEW_DEBUG) return;
    console.warn("[RichTextPreview][CompactWindow]", stage, detail || {});
};

const getIcon = (type: string) => {
    switch (type) {
        case "text": return <FileText size={14} />;
        case "image": return <ImageIcon size={14} />;
        case "url": return <LinkIcon size={14} />;
        case "code": return <Code size={14} />;
        case "file": return <File size={14} />;
        case "video": return <Video size={14} />;
        default: return <FileText size={14} />;
    }
};

const seekVideoPreviewFrame = (video: HTMLVideoElement | null) => {
    if (!video) return;
    const duration = video.duration;
    if (!Number.isFinite(duration) || duration <= 0) return;
    const maxSeek = Math.max(duration - 0.05, 0);
    if (maxSeek <= 0) return;
    const preferred = Math.min(duration * 0.1, 2);
    const target = Math.min(Math.max(preferred, 0.1), maxSeek);
    if (target <= 0) return;
    try {
        video.currentTime = target;
    } catch {
        // Ignore seek errors; fallback will just show the first frame.
    }
};

const applyTheme = (payload: PreviewPayload) => {
    const root = document.documentElement;
    const body = document.body;

    root.classList.add("theme-fluent");
    body.classList.add("theme-fluent");

    if (payload.colorMode === "light") {
        root.classList.add("light-mode");
        body.classList.add("light-mode");
        root.classList.remove("dark-mode");
        body.classList.remove("dark-mode");
    } else {
        root.classList.add("dark-mode");
        body.classList.add("dark-mode");
        root.classList.remove("light-mode");
        body.classList.remove("light-mode");
    }

    body.classList.add("compact-preview");

    if (payload.clipboardItemFontSize) {
        root.style.setProperty("--clipboard-item-font-size", `${payload.clipboardItemFontSize}px`);
    }
    if (payload.clipboardTagFontSize) {
        root.style.setProperty("--clipboard-tag-font-size", `${payload.clipboardTagFontSize}px`);
    }
};

const CompactPreviewWindow = () => {
    const [payload, setPayload] = useState<PreviewPayload | null>(null);
    const [snapshotFailed, setSnapshotFailed] = useState(false);
    const [richImageFallbackFailed, setRichImageFallbackFailed] = useState(false);
    const [animating, setAnimating] = useState(false);
    const [contextMenu, setContextMenu] = useState<ContextMenuState>({ visible: false, x: 0, y: 0, maxHeight: 200 });
    const contextMenuRef = useRef<HTMLDivElement | null>(null);
    const richSnapshotImgRef = useRef<HTMLImageElement | null>(null);
    const richSnapshotFallbackTimerRef = useRef<number | null>(null);
    const containerRef = useRef<HTMLDivElement | null>(null);
    const metaRef = useRef<HTMLDivElement | null>(null);
    const contentRef = useRef<HTMLDivElement | null>(null);
    const dividerRef = useRef<HTMLDivElement | null>(null);
    const requestResizeRef = useRef<() => void>(() => {});
    const previewBoundsRef = useRef({ width: 560, height: 560, mediaWidth: 520, mediaHeight: 360 });
    const lastSentSizeRef = useRef<{ width: number; height: number } | null>(null);
    const richImageFallbackSrc = useMemo(() => {
        if (!payload || payload.contentType !== "rich_text" || !payload.htmlContent) return null;
        const { imagePayload } = extractRichImageFallback(payload.htmlContent);
        if (!imagePayload) return null;
        const src = resolveRichImageSrc(imagePayload);
        if (!src) return null;
        return src;
    }, [payload]);
    const richTextSnapshotSrc = useMemo(() => {
        if (!payload || payload.contentType !== "rich_text" || !payload.htmlContent) return null;
        if (!payload.richTextSnapshotPreview) return null;
        const { cleanHtml } = extractRichImageFallback(payload.htmlContent);
        const htmlForSnapshot = cleanHtml || payload.htmlContent;
        return getRichTextSnapshotDataUrl(htmlForSnapshot, {
            width: 560,
            maxHeight: 1200
        });
    }, [payload]);
    const effectiveRichTextSnapshotSrc = snapshotFailed ? null : richTextSnapshotSrc;
    const effectiveRichImageFallbackSrc = richImageFallbackFailed ? null : richImageFallbackSrc;
    const useSnapshotPreviewImage = !effectiveRichImageFallbackSrc && !!effectiveRichTextSnapshotSrc;

    useEffect(() => {
        getCurrentWindow().setAlwaysOnTop(true).catch(console.error);
        (async () => {
            try {
                const monitor = await currentMonitor();
                const monitorWidth = monitor?.size.width ?? 1280;
                const monitorHeight = monitor?.size.height ?? 720;
                const maxWidth = Math.max(320, Math.floor(monitorWidth * 0.5));
                const maxHeight = Math.max(240, Math.min(560, Math.floor(monitorHeight * 0.6)));
                const mediaMaxWidth = Math.max(260, Math.min(520, maxWidth));
                const mediaMaxHeight = Math.max(200, Math.min(360, maxHeight - 120));
                const minWidth = Math.max(300, Math.min(380, Math.floor(maxWidth * 0.62)));

                previewBoundsRef.current = {
                    width: maxWidth,
                    height: maxHeight,
                    mediaWidth: mediaMaxWidth,
                    mediaHeight: mediaMaxHeight
                };

                const root = document.documentElement;
                root.style.setProperty("--preview-max-width", `${maxWidth}px`);
                root.style.setProperty("--preview-max-height", `${maxHeight}px`);
                root.style.setProperty("--preview-media-max-width", `${mediaMaxWidth}px`);
                root.style.setProperty("--preview-media-max-height", `${mediaMaxHeight}px`);
                root.style.setProperty("--preview-min-width", `${minWidth}px`);

                await getCurrentWindow().setSize(new LogicalSize(maxWidth, maxHeight));
            } catch (err) {
                console.error("Failed to initialize preview bounds:", err);
            } finally {
                requestResizeRef.current?.();
            }
        })();
        const unlisten = listen<PreviewPayload>("compact-preview-update", (event) => {
            setPayload(event.payload);
            applyTheme(event.payload);
        });
        emitTo("main", "compact-preview-mounted", true).catch(console.error);
        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    useEffect(() => {
        if (!payload) return;
        let raf = 0;
        let timerA: number | null = null;
        let timerB: number | null = null;
        let timerC: number | null = null;
        const updateSize = () => {
            raf = window.requestAnimationFrame(() => {
                const container = containerRef.current;
                if (!container) return;

                const bounds = previewBoundsRef.current;
                const maxWidth = bounds.width;
                const maxHeight = bounds.height;
                const minWidth =
                    payload.contentType === "image" || payload.contentType === "video"
                        ? 260
                        : Math.min(bounds.width, 320);

                const measuredWidth = Math.max(container.offsetWidth, container.scrollWidth);
                const measuredHeight = Math.max(container.offsetHeight, container.scrollHeight);
                const width = Math.min(Math.max(Math.ceil(measuredWidth), minWidth), maxWidth);
                const height = Math.min(Math.max(Math.ceil(measuredHeight), 140), maxHeight);
                if (width < 40 || height < 40) return;

                const last = lastSentSizeRef.current;
                if (last && Math.abs(last.width - width) <= 1 && Math.abs(last.height - height) <= 1) return;

                lastSentSizeRef.current = { width, height };
                emitTo("main", "compact-preview-resize", { width, height }).catch(console.error);
                getCurrentWindow().setSize(new LogicalSize(width, height)).catch(console.error);
            });
        };

        requestResizeRef.current = updateSize;
        updateSize();
        const observer = new ResizeObserver(updateSize);
        if (containerRef.current) observer.observe(containerRef.current);
        if (contentRef.current) observer.observe(contentRef.current);
        if (metaRef.current) observer.observe(metaRef.current);
        if (dividerRef.current) observer.observe(dividerRef.current);

        timerA = window.setTimeout(updateSize, 50);
        timerB = window.setTimeout(updateSize, 180);
        timerC = window.setTimeout(updateSize, 420);

        return () => {
            if (raf) cancelAnimationFrame(raf);
            if (timerA) window.clearTimeout(timerA);
            if (timerB) window.clearTimeout(timerB);
            if (timerC) window.clearTimeout(timerC);
            observer.disconnect();
        };
    }, [payload]);

    useEffect(() => {
        setSnapshotFailed(false);
        setRichImageFallbackFailed(false);
    }, [payload?.content, payload?.htmlContent, payload?.richTextSnapshotPreview]);

    // Trigger entrance animation on each new payload (window show or content change)
    useEffect(() => {
        if (!payload) return;
        setAnimating(true);
        const timer = window.setTimeout(() => setAnimating(false), 250);
        return () => window.clearTimeout(timer);
    }, [payload]);

    useEffect(() => {
        if (richSnapshotFallbackTimerRef.current) {
            window.clearTimeout(richSnapshotFallbackTimerRef.current);
            richSnapshotFallbackTimerRef.current = null;
        }
        if (!useSnapshotPreviewImage) return;

        richSnapshotFallbackTimerRef.current = window.setTimeout(() => {
            const img = richSnapshotImgRef.current;
            if (!img || !img.complete || img.naturalWidth <= 0 || img.naturalHeight <= 0) {
                richPreviewFailureLog("snapshot image timeout -> fallback to html", {
                    itemId: payload?.content,
                    hasImageElement: !!img,
                    complete: img?.complete ?? false,
                    naturalWidth: img?.naturalWidth ?? 0,
                    naturalHeight: img?.naturalHeight ?? 0
                });
                setSnapshotFailed(true);
            }
        }, 700);

        return () => {
            if (richSnapshotFallbackTimerRef.current) {
                window.clearTimeout(richSnapshotFallbackTimerRef.current);
                richSnapshotFallbackTimerRef.current = null;
            }
        };
    }, [useSnapshotPreviewImage, effectiveRichTextSnapshotSrc, payload?.content, payload?.htmlContent]);

    // 右键菜单处理（限制在窗口视口范围内，窗口再小也能滚动查看）
    const handleContextMenu = (e: React.MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        const menuWidth = 160;
        const vw = document.documentElement.clientWidth;
        const vh = document.documentElement.clientHeight;
        const menuHeight = Math.min(130, vh - 8); // 菜单不超过视口高度 - 边距
        const maxHeight = vh - 8; // 限制最大高度，留 8px 边距
        const x = Math.max(0, Math.min(e.clientX, vw - menuWidth));
        const y = Math.max(0, Math.min(e.clientY, vh - menuHeight));
        setContextMenu({ visible: true, x, y, maxHeight });
    };

    const closeContextMenu = () => {
        setContextMenu(prev => ({ ...prev, visible: false }));
    };

    useEffect(() => {
        if (!contextMenu.visible) return;
        const handleClick = (e: MouseEvent) => {
            if (contextMenuRef.current && !contextMenuRef.current.contains(e.target as Node)) {
                closeContextMenu();
            }
        };
        document.addEventListener("mousedown", handleClick);
        return () => document.removeEventListener("mousedown", handleClick);
    }, [contextMenu.visible]);

    const handleCopy = useCallback(async () => {
        if (!payload) return;
        closeContextMenu();
        try {
            await invoke("copy_to_clipboard", {
                content: payload.content,
                contentType: payload.contentType,
                paste: false,
                id: 0,
                deleteAfterUse: false,
            });
        } catch (err) {
            console.error("Copy from preview failed:", err);
        }
    }, [payload]);

    const handleOpen = useCallback(async () => {
        if (!payload) return;
        closeContextMenu();
        try {
            await invoke("open_content", {
                content: payload.content,
                contentType: payload.contentType,
                id: 0,
            });
        } catch (err) {
            console.error("Open from preview failed:", err);
        }
    }, [payload]);

    const handleClose = useCallback(async () => {
        closeContextMenu();
        try {
            await getCurrentWindow().hide();
            emitTo("main", "compact-preview-close", true).catch(console.error);
        } catch (err) {
            console.error("Close preview failed:", err);
        }
    }, []);

    const content = useMemo(() => {
        if (!payload) return null;
        if (payload.contentType === "image") {
            const src = payload.content.startsWith("data:")
                ? payload.content
                : (toTauriLocalImageSrc(payload.content) || payload.content);
            return (
                <img
                    src={src}
                    alt="preview"
                    onLoad={() => requestResizeRef.current?.()}
                    style={{ width: "auto", height: "auto", borderRadius: "4px" }}
                />
            );
        }
        if (payload.contentType === "video") {
            const src = payload.content.startsWith("data:")
                ? payload.content
                : (toTauriLocalImageSrc(payload.content) || payload.content);
            return (
                <video
                    src={src}
                    preload="metadata"
                    muted
                    playsInline
                    onLoadedMetadata={(e) => {
                        seekVideoPreviewFrame(e.currentTarget);
                        requestResizeRef.current?.();
                    }}
                    style={{ width: "auto", height: "auto", borderRadius: "4px", background: "#000" }}
                />
            );
        }
        if (payload.contentType === "rich_text" && payload.htmlContent) {
            if (effectiveRichImageFallbackSrc) {
                return (
                    <img
                        src={effectiveRichImageFallbackSrc}
                        alt="rich text preview"
                        onLoad={() => requestResizeRef.current?.()}
                        onError={() => setRichImageFallbackFailed(true)}
                        style={{ width: "auto", height: "auto", borderRadius: "4px" }}
                    />
                );
            }
            if (effectiveRichTextSnapshotSrc) {
                return (
                    <img
                        ref={richSnapshotImgRef}
                        src={effectiveRichTextSnapshotSrc}
                        alt="rich text snapshot preview"
                        onLoad={() => {
                            if (richSnapshotFallbackTimerRef.current) {
                                window.clearTimeout(richSnapshotFallbackTimerRef.current);
                                richSnapshotFallbackTimerRef.current = null;
                            }
                            requestResizeRef.current?.();
                        }}
                        onError={() => {
                            if (richSnapshotFallbackTimerRef.current) {
                                window.clearTimeout(richSnapshotFallbackTimerRef.current);
                                richSnapshotFallbackTimerRef.current = null;
                            }
                            setSnapshotFailed(true);
                        }}
                        style={{ width: "100%", maxWidth: "100%", height: "auto", display: "block", borderRadius: "4px" }}
                    />
                );
            }
            const { cleanHtml } = extractRichImageFallback(payload.htmlContent);
            return (
                <HtmlContent
                    className="rich-text-preview"
                    htmlContent={cleanHtml || payload.htmlContent}
                    fallbackText={payload.preview || payload.content}
                    preview={false}
                    style={{ maxHeight: "none", overflow: "visible", fontSize: "var(--clipboard-item-font-size)", lineHeight: "1.5" }}
                />
            );
        }
        const text = payload.content || payload.preview || "";
        return <div style={{ whiteSpace: "pre", overflow: "hidden" }}>{text}</div>;
    }, [payload, effectiveRichImageFallbackSrc, effectiveRichTextSnapshotSrc]);

    return (
        <div className={`compact-preview-root${animating ? ' preview-entering' : ''}`} onContextMenu={handleContextMenu}>
            <div
                ref={containerRef}
                className={`compact-popover-portal compact-preview-window theme-fluent ${payload?.contentType === "image" ? "compact-preview-image" : ""} ${payload?.contentType === "image" || payload?.contentType === "video" || !!effectiveRichImageFallbackSrc ? "compact-preview-media" : ""} ${payload?.colorMode === 'light' ? 'light-mode' : 'dark-mode'}`}
                style={{ display: "flex", flexDirection: "column" }}
            >
                <div ref={metaRef} className="popover-meta">
                    <div className="meta-row">
                        {getIcon(payload?.contentType || "text")}
                        <span>{payload?.contentType || "text"}</span>
                    </div>
                    <div className="meta-dot">•</div>
                    <div className="meta-row">
                        <AppWindow size={14} />
                        <span>{payload?.sourceApp || "Unknown"}</span>
                    </div>
                    <div className="meta-dot">•</div>
                    <div className="meta-row">
                        <Clock size={14} />
                        <span>
                            {payload?.timestamp && payload?.language
                                ? getConciseTime(payload.timestamp, payload.language)
                                : "-"}
                        </span>
                    </div>
                </div>
                <div ref={dividerRef} className="popover-divider" />
                <div ref={contentRef} className="popover-content">{content}</div>
            </div>
            {/* 右键菜单 */}
            {contextMenu.visible && (
                <div
                    ref={contextMenuRef}
                    className="preview-context-menu"
                    style={{
                        left: contextMenu.x,
                        top: contextMenu.y,
                        maxHeight: contextMenu.maxHeight,
                    }}
                >
                    <div className="preview-context-menu-item" onClick={handleCopy}>
                        <Copy size={14} className="preview-context-menu-item-icon" />
                        <span className="preview-context-menu-item-label">复制</span>
                    </div>
                    <div className="preview-context-menu-item" onClick={handleOpen}>
                        <ExternalLink size={14} className="preview-context-menu-item-icon" />
                        <span className="preview-context-menu-item-label">打开</span>
                    </div>
                    <div className="preview-context-menu-separator" />
                    <div className="preview-context-menu-item danger" onClick={handleClose}>
                        <X size={14} className="preview-context-menu-item-icon" />
                        <span className="preview-context-menu-item-label">关闭预览</span>
                    </div>
                </div>
            )}
        </div>
    );
};

export default CompactPreviewWindow;
