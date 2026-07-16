/// Rich content preview rendering for clipboard items.
/// Uses `ClipboardEntryView` (no full content needed for display).
use crate::domain::ClipboardEntryView;
use crate::infrastructure::repository::clipboard_repo::ClipboardRepository;
use crate::ui::styles;
use windows_reactor::*;

/// Unified card renderer that dispatches by content type.
pub fn render_card(entry: &ClipboardEntryView, theme: &styles::ThemeColors) -> Element {
    match entry.content_type.as_str() {
        "image" => render_image_card(entry, theme),
        "file" => render_file_card(entry, theme),
        "code" => render_code_card(entry, theme),
        "video" => render_video_card(entry, theme),
        "url" => render_url_card(entry, theme),
        "rich_text" => render_rich_text_card(entry, theme),
        _ => render_text_card(entry, theme),
    }
}

/// Get emoji icon for a file extension.
pub fn file_type_icon(file_path: &str) -> &'static str {
    let ext = file_path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        // Archives
        "zip" | "rar" | "7z" | "tar" | "gz" | "xz" => "\u{1F4E6}",
        // Audio
        "mp3" | "wav" | "flac" | "m4a" | "ogg" | "aac" => "\u{1F3B5}",
        // Video
        "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" => "\u{1F3AC}",
        // Images
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" => "\u{1F5BC}",
        // Documents
        "pdf" => "\u{1F4D5}",
        "doc" | "docx" => "\u{1F4C4}",
        "xls" | "xlsx" | "csv" => "\u{1F4CA}",
        "ppt" | "pptx" => "\u{1F4CA}",
        // Code
        "js" | "ts" | "tsx" | "jsx" | "py" | "rs" | "c" | "cpp" | "h"
        | "go" | "java" | "html" | "css" | "json" | "xml" | "yaml" | "yml"
        | "toml" | "sh" | "bat" | "ps1" => "\u{1F4BB}",
        // Executables
        "exe" | "msi" | "dll" => "\u{2699}",
        // Text
        "txt" | "md" | "log" | "ini" | "cfg" => "\u{1F4DD}",
        // Fonts
        "ttf" | "otf" | "woff" | "woff2" => "\u{1F524}",
        // Default
        _ => "\u{1F4C4}",
    }
}

/// Render a text/rich_text content preview card.
fn render_text_card(entry: &ClipboardEntryView, theme: &styles::ThemeColors) -> Element {
    let preview = truncate_str(&entry.preview, 160);
    let meta = format!("{} chars, {} lines", entry.content_len, entry.line_count);

    vstack((
        text_block(preview)
            .font_size(13.0)
            .foreground(styles::to_color(theme.text_primary))
            .horizontal_alignment(HorizontalAlignment::Left)
            .grid_row(0),
        text_block(meta)
            .font_size(10.0)
            .foreground(styles::to_color(theme.text_muted))
            .horizontal_alignment(HorizontalAlignment::Left)
            .grid_row(1),
    ))
    .spacing(4.0)
    .padding(Thickness::uniform(8.0))
    .background(styles::to_color(theme.card_background))
    .into()
}

/// Render an image content preview card — loads image from DB and displays it.
pub fn render_image_card(entry: &ClipboardEntryView, _theme: &styles::ThemeColors) -> Element {
    let entry_id = entry.id;

    // Load full entry from DB to get the image file path
    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let repo = crate::infrastructure::repository::clipboard_repo::SqliteClipboardRepository::new(ctx.db_conn.clone());
        if let Ok(Some(full_entry)) = repo.get_entry_by_id(entry_id) {
            drop(ctx);
            let content = &full_entry.content;
            let file_path = if full_entry.is_external {
                content.trim().to_string()
            } else if content.starts_with("data:image") {
                if let Some(idx) = content.find(',') {
                    let b64 = &content[idx + 1..];
                    if let Ok(bytes) = base64::Engine::decode(
                        &base64::engine::general_purpose::STANDARD, b64
                    ) {
                        let temp_dir = std::env::temp_dir().join("clipboard_images");
                        let _ = std::fs::create_dir_all(&temp_dir);
                        let file_name = format!("img_{}.png", entry_id);
                        let file_path = temp_dir.join(file_name);
                        if file_path.exists() || std::fs::write(&file_path, &bytes).is_ok() {
                            file_path.to_string_lossy().to_string()
                        } else {
                            return image_fallback(entry);
                        }
                    } else {
                        return image_fallback(entry);
                    }
                } else {
                    return image_fallback(entry);
                }
            } else {
                return image_fallback(entry);
            };

            let uri = format!("file:///{}", file_path.replace('\\', "/"));

            return border(
                Image::new_with_uri(&uri)
                    .stretch(Stretch::UniformToFill)
                    .width(340.0)
                    .height(180.0)
                    .horizontal_alignment(HorizontalAlignment::Left),
            )
            .corner_radius(4.0)
            .padding(Thickness { top: 0.0, bottom: 12.0, left: 16.0, right: 16.0 })
            .into();
        }
    }

    image_fallback(entry)
}

fn image_fallback(entry: &ClipboardEntryView) -> Element {
    let meta = if entry.is_data_url {
        format!("Image ({})", entry.image_format)
    } else {
        "Image (raw data)".to_string()
    };
    text_block(meta)
        .font_size(12.0)
        .foreground(Color::rgb(102, 102, 102))
        .padding(Thickness::uniform(8.0))
        .into()
}

/// Render a file content preview card with type-specific icon.
fn render_file_card(entry: &ClipboardEntryView, theme: &styles::ThemeColors) -> Element {
    let (icon, label) = if entry.file_count == 1 {
        let icon = file_type_icon(&entry.first_file);
        let name = entry.first_file.rsplit('\\').next().unwrap_or(&entry.first_file);
        let name = truncate_str(name, 40);
        (icon, name)
    } else {
        ("\u{1F4C1}", format!("{} files", entry.file_count))
    };

    vstack((
        hstack((
            text_block(icon.to_string())
                .font_size(20.0)
                .grid_row(0),
            text_block(label)
                .font_size(13.0)
                .foreground(styles::to_color(theme.text_primary))
                .horizontal_alignment(HorizontalAlignment::Left)
                .grid_row(0),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Left)
        .grid_row(0),
        text_block(truncate_str(&entry.preview, 100))
            .font_size(10.0)
            .foreground(styles::to_color(theme.text_muted))
            .horizontal_alignment(HorizontalAlignment::Left)
            .grid_row(1),
    ))
    .spacing(4.0)
    .padding(Thickness::uniform(8.0))
    .background(styles::to_color(theme.card_background))
    .into()
}

/// Render a code content preview card with monospace styling.
fn render_code_card(entry: &ClipboardEntryView, theme: &styles::ThemeColors) -> Element {
    vstack((
        text_block(entry.code_preview.clone())
            .font_size(12.0)
            .foreground(styles::to_color(theme.text_primary))
            .horizontal_alignment(HorizontalAlignment::Left)
            .grid_row(0),
        text_block(format!("{} lines", entry.line_count))
            .font_size(10.0)
            .foreground(styles::to_color(theme.text_muted))
            .horizontal_alignment(HorizontalAlignment::Left)
            .grid_row(1),
    ))
    .spacing(4.0)
    .padding(Thickness::uniform(8.0))
    .background(styles::to_color(theme.card_background))
    .into()
}

/// Render a video content preview card.
fn render_video_card(entry: &ClipboardEntryView, theme: &styles::ThemeColors) -> Element {
    let file_name = if entry.first_file.is_empty() {
        truncate_str(&entry.preview, 40)
    } else {
        let name = entry.first_file.rsplit('\\').next().unwrap_or(&entry.first_file);
        truncate_str(name, 40)
    };

    vstack((
        hstack((
            text_block("\u{1F3AC}".to_string())
                .font_size(20.0)
                .grid_row(0),
            text_block(file_name)
                .font_size(13.0)
                .foreground(styles::to_color(theme.text_primary))
                .horizontal_alignment(HorizontalAlignment::Left)
                .grid_row(0),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Left)
        .grid_row(0),
        text_block("Video".to_string())
            .font_size(10.0)
            .foreground(styles::to_color(theme.text_muted))
            .horizontal_alignment(HorizontalAlignment::Left)
            .grid_row(1),
    ))
    .spacing(4.0)
    .padding(Thickness::uniform(8.0))
    .background(styles::to_color(theme.card_background))
    .into()
}

/// Render a URL content preview card.
fn render_url_card(entry: &ClipboardEntryView, theme: &styles::ThemeColors) -> Element {
    let url = truncate_str(&entry.preview, 80);

    vstack((
        hstack((
            text_block("\u{1F517}".to_string())
                .font_size(20.0)
                .grid_row(0),
            text_block(url)
                .font_size(13.0)
                .foreground(styles::to_color(theme.accent))
                .horizontal_alignment(HorizontalAlignment::Left)
                .grid_row(0),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Left)
        .grid_row(0),
        text_block("URL".to_string())
            .font_size(10.0)
            .foreground(styles::to_color(theme.text_muted))
            .horizontal_alignment(HorizontalAlignment::Left)
            .grid_row(1),
    ))
    .spacing(4.0)
    .padding(Thickness::uniform(8.0))
    .background(styles::to_color(theme.card_background))
    .into()
}

/// Render a rich text content preview card.
fn render_rich_text_card(entry: &ClipboardEntryView, theme: &styles::ThemeColors) -> Element {
    let preview = truncate_str(&entry.preview, 160);

    vstack((
        hstack((
            text_block("\u{1F4DD}".to_string())
                .font_size(20.0)
                .grid_row(0),
            text_block("Rich Text".to_string())
                .font_size(12.0)
                .foreground(styles::to_color(theme.text_muted))
                .horizontal_alignment(HorizontalAlignment::Left)
                .grid_row(0),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Left)
        .grid_row(0),
        text_block(preview)
            .font_size(13.0)
            .foreground(styles::to_color(theme.text_primary))
            .horizontal_alignment(HorizontalAlignment::Left)
            .grid_row(1),
    ))
    .spacing(4.0)
    .padding(Thickness::uniform(8.0))
    .background(styles::to_color(theme.card_background))
    .into()
}

/// Truncate string helper.
fn truncate_str(s: &str, max: usize) -> String {
    let truncated: String = s.chars().take(max).collect();
    if truncated.len() < s.len() {
        format!("{}\u{2026}", truncated)
    } else {
        truncated
    }
}
