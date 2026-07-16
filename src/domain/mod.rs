use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClipboardEntry {
    pub id: i64,
    pub content_type: String, // 'text', 'image', 'code', 'file', 'video'
    pub content: String,
    #[serde(default)]
    pub html_content: Option<String>,
    pub source_app: String,
    #[serde(default)]
    pub source_app_path: Option<String>,
    pub timestamp: i64,
    pub preview: String,
    pub is_pinned: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub use_count: i32,
    #[serde(default)]
    pub is_external: bool,
    #[serde(default)]
    pub pinned_order: i64,
    #[serde(default = "default_true")]
    pub file_preview_exists: bool,
}

fn default_true() -> bool {
    true
}

/// Lightweight entry for UI list display — no `content` or `html_content`.
/// Full data is fetched from DB on demand (paste, copy, etc.).
#[derive(Debug, Clone)]
pub struct ClipboardEntryView {
    pub id: i64,
    pub content_type: String,
    pub source_app: String,
    pub timestamp: i64,
    pub preview: String,
    pub is_pinned: bool,
    pub tags: Vec<String>,
    pub use_count: i32,
    pub pinned_order: i64,
    /// Pre-computed: `entry.content.len()`
    pub content_len: usize,
    /// Pre-computed: `entry.content.lines().count()`
    pub line_count: usize,
    /// Pre-computed: `entry.content.starts_with("data:image")`
    pub is_data_url: bool,
    /// Pre-computed: image format (e.g. "png"), empty if not image
    pub image_format: String,
    /// Pre-computed: file count for file type
    pub file_count: usize,
    /// Pre-computed: first file path basename for file type
    pub first_file: String,
    /// Pre-computed: first 3 lines of code
    pub code_preview: String,
}

impl ClipboardEntryView {
    pub fn from_entry(entry: &ClipboardEntry) -> Self {
        let content = &entry.content;
        let (is_data_url, image_format) = if content.starts_with("data:image") {
            let fmt = content
                .split(',')
                .next()
                .unwrap_or("")
                .split(';')
                .next()
                .unwrap_or("")
                .split(':')
                .last()
                .unwrap_or("png");
            (true, fmt.to_string())
        } else {
            (false, String::new())
        };
        let file_count = if entry.content_type == "file" {
            content.lines().count()
        } else {
            0
        };
        let first_file = if entry.content_type == "file" {
            content
                .lines()
                .next()
                .map(|p| {
                    std::path::Path::new(p)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| p.to_string())
                })
                .unwrap_or_default()
        } else {
            String::new()
        };
        let code_preview = if entry.content_type == "code" {
            let first_lines: String = content.lines().take(3).collect::<Vec<_>>().join("\n");
            let truncated: String = first_lines.chars().take(120).collect();
            if truncated.len() < first_lines.len() {
                format!("{}\u{2026}", truncated)
            } else {
                truncated
            }
        } else {
            String::new()
        };

        Self {
            id: entry.id,
            content_type: entry.content_type.clone(),
            source_app: entry.source_app.clone(),
            timestamp: entry.timestamp,
            preview: entry.preview.clone(),
            is_pinned: entry.is_pinned,
            tags: entry.tags.clone(),
            use_count: entry.use_count,
            pinned_order: entry.pinned_order,
            content_len: content.len(),
            line_count: content.lines().count(),
            is_data_url,
            image_format,
            file_count,
            first_file,
            code_preview,
        }
    }
}
