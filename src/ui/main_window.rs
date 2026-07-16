/// Main clipboard window — matches WinPaste UI design with WinUI3 native controls.
/// Features: toast notifications, confirm dialog, search debounce, improved card styling.
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};
use std::time::Duration;
use windows_reactor::*;

use crate::domain::ClipboardEntryView;
use crate::infrastructure::repository::clipboard_repo::ClipboardRepository;
use crate::state::global_state::*;
use crate::ui::styles;

// ── Helpers ──────────────────────────────────────────────────────────────

fn format_timestamp(millis: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let diff = now - millis;
    if diff < 60_000 {
        format!("{}秒前", diff / 1000)
    } else if diff < 3600_000 {
        format!("{}分钟前", diff / 60_000)
    } else if diff < 86400_000 {
        format!("{}小时前", diff / 3600_000)
    } else {
        format!("{}天前", diff / 86400_000)
    }
}

fn app_icon_color(source: &str) -> (u8, u8, u8) {
    if source.contains("Weixin") || source.contains("微信") {
        (7, 193, 96)
    } else if source.contains("msedge") || source.contains("Edge") {
        (0, 120, 212)
    } else if source.contains("explorer") {
        (255, 185, 0)
    } else if source.contains("code") || source.contains("Code") {
        (0, 120, 212)
    } else if source.contains("Chrome") {
        (219, 68, 55)
    } else {
        (100, 100, 100)
    }
}

// ── Clipboard item card ──────────────────────────────────────────────────

/// Generate a consistent color for a tag chip based on the tag name.
pub fn tag_chip_color(tag: &str) -> Color {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    tag.hash(&mut hasher);
    let hash = hasher.finish();
    // Generate a hue from 0-360, with fixed saturation and lightness for vibrant but readable colors
    let hue = (hash % 360) as f64;
    let saturation = 0.65;
    let lightness = 0.45;
    // Convert HSL to RGB
    let c = (1.0_f64 - (2.0_f64 * lightness - 1.0_f64).abs()) * saturation;
    let x = c * (1.0_f64 - ((hue / 60.0_f64) % 2.0_f64 - 1.0_f64).abs());
    let m = lightness - c / 2.0;
    let (r, g, b) = if hue < 60.0 {
        (c, x, 0.0)
    } else if hue < 120.0 {
        (x, c, 0.0)
    } else if hue < 180.0 {
        (0.0, c, x)
    } else if hue < 240.0 {
        (0.0, x, c)
    } else if hue < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Color::rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Paste a clipboard entry by its ID (runs in background thread to avoid blocking UI).
fn paste_entry_by_id(entry_id: i64) {
    std::thread::spawn(move || {
        if let Some(db_conn) = crate::state::global_state::DB_CONN.get() {
            let repo =
                crate::infrastructure::repository::clipboard_repo::SqliteClipboardRepository::new(
                    db_conn.clone(),
                );
            if let Ok(Some(entry)) = repo.get_entry_by_id(entry_id) {
                let _ = crate::services::clipboard_ops::paste_entry(&entry);
            }
        }
    });
}

/// Show a toast message (auto-hides after 2 seconds).
pub fn show_toast(message: &str) {
    if let Ok(mut toast) = crate::state::global_state::TOAST_MESSAGE.lock() {
        *toast = message.to_string();
    }
    crate::state::global_state::TOAST_TIMESTAMP.store(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        Ordering::Relaxed,
    );
}

/// Show a confirm dialog and return the result.
pub fn show_confirm_dialog(message: &str) -> bool {
    // For now, use a simple message box via Win32 API
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_YESNO, MB_ICONQUESTION, MB_DEFBUTTON2};
        let msg: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
        let title: Vec<u16> = "确认\0".encode_utf16().collect();
        let result = MessageBoxW(
            None,
            windows::core::PCWSTR(msg.as_ptr()),
            windows::core::PCWSTR(title.as_ptr()),
            MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2,
        );
        result.0 == 6 // IDYES
    }
}

fn clipboard_item(entry: &ClipboardEntryView, index: i32) -> Element {
    let entry_id = entry.id;
    let content_type = entry.content_type.clone();
    let is_pinned = entry.is_pinned;
    let source_name = if entry.source_app.is_empty() {
        "Unknown".to_string()
    } else {
        entry.source_app.clone()
    };
    let time_str = format_timestamp(entry.timestamp);
    let (r, g, b) = app_icon_color(&source_name);
    let theme = styles::get_theme_colors();

    // Check if this item is selected via keyboard navigation
    let is_selected = SELECTED_INDEX.load(Ordering::Relaxed) == index;

    // Content preview: text for non-image, actual image for image type
    let content_preview: Element = if entry.content_type == "image" {
        crate::ui::content_preview::render_image_card(entry, &theme)
    } else if entry.content_type == "rich_text" {
        // Rich text: show HTML content if available, otherwise show preview
        let preview: String = entry.preview.chars().take(160).collect();
        vstack((
            hstack((
                text_block("\u{1F4DD}".to_string())
                    .font_size(16.0),
                text_block("富文本".to_string())
                    .font_size(11.0)
                    .foreground(Color::rgb(102, 102, 102)),
            ))
            .spacing(4.0),
            text_block(preview)
                .font_size(13.0)
                .foreground(Color::rgb(51, 51, 51)),
        ))
        .spacing(4.0)
        .padding(Thickness::xy(16.0, 8.0))
        .into()
    } else if entry.content_type == "code" {
        // Code: show with monospace font
        let preview: String = entry.preview.chars().take(120).collect();
        vstack((
            text_block(preview)
                .font_size(12.0)
                .font_family("Consolas".to_string())
                .foreground(Color::rgb(51, 51, 51)),
            text_block(format!("{} 行", entry.line_count))
                .font_size(10.0)
                .foreground(Color::rgb(153, 153, 153)),
        ))
        .spacing(2.0)
        .padding(Thickness::xy(16.0, 8.0))
        .into()
    } else if entry.content_type == "file" {
        // File: show file icon and name
        let icon = crate::ui::content_preview::file_type_icon(&entry.first_file);
        let name = if entry.file_count > 1 {
            format!("{} 个文件", entry.file_count)
        } else {
            entry.first_file.clone()
        };
        hstack((
            text_block(icon.to_string())
                .font_size(20.0),
            text_block(name)
                .font_size(13.0)
                .foreground(Color::rgb(51, 51, 51))
                .horizontal_alignment(HorizontalAlignment::Left),
        ))
        .spacing(8.0)
        .padding(Thickness::xy(16.0, 8.0))
        .into()
    } else {
        // Text, URL, video: show preview
        let preview: String = entry.preview.chars().take(160).collect();
        let preview_display = if preview.len() < entry.preview.len() {
            format!("{}\u{2026}", preview)
        } else {
            preview
        };
        text_block(preview_display)
            .font_size(14.0)
            .foreground(Color::rgb(102, 102, 102))
            .padding(Thickness {
                top: 0.0,
                bottom: 12.0,
                left: 16.0,
                right: 16.0,
            })
            .into()
    };

    // Selection highlight background with rounded corners and shadow effect
    let card_bg = if is_selected {
        Color::rgb(204, 225, 255) // Light blue highlight
    } else if entry.is_pinned {
        Color::rgb(255, 249, 230) // Light yellow for pinned
    } else {
        Color::rgb(255, 255, 255)
    };

    let left_border_color = if is_selected {
        Color::rgb(0, 120, 212) // Blue accent for selected
    } else if entry.is_pinned {
        Color::rgb(255, 185, 0) // Gold for pinned
    } else {
        Color::rgb(230, 230, 230)
    };

    // Check if this item is hovered
    let is_hovered = HOVERED_INDEX.load(Ordering::Relaxed) == index;
    let target_scale = if is_hovered { 1.03 } else { 1.0 };

    // Card with rounded corners and subtle border
    border(
        vstack((
            // Row 1: App icon dot + source name + relative time
            hstack((
                border(text_block(String::new()))
                    .width(8.0)
                    .height(8.0)
                    .corner_radius(4.0)
                    .background(Color::rgb(r, g, b)),
                text_block(source_name)
                    .font_size(12.0)
                    .font_weight(600u16)
                    .foreground(Color::rgb(51, 51, 51)),
                text_block(time_str)
                    .font_size(11.0)
                    .foreground(Color::rgb(153, 153, 153))
                    .horizontal_alignment(HorizontalAlignment::Right),
            ))
            .spacing(6.0)
            .padding(Thickness::xy(16.0, 12.0))
            .vertical_alignment(VerticalAlignment::Center)
            .horizontal_alignment(HorizontalAlignment::Stretch),
            // Row 2: Content preview (image or text)
            content_preview,
            // Row 3: Tag chips (if any)
            if entry.tags.is_empty() {
                Element::from(vstack(()).spacing(0.0))
            } else {
                let tag_chips: Vec<Element> = entry.tags.iter().map(|tag| {
                    let tag_color = tag_chip_color(tag);
                    border(
                        text_block(tag.clone())
                            .font_size(10.0)
                            .foreground(Color::rgb(255, 255, 255))
                            .padding(Thickness::xy(6.0, 2.0))
                    )
                    .corner_radius(4.0)
                    .background(tag_color)
                    .into()
                }).collect();
                hstack(tag_chips)
                    .spacing(4.0)
                    .padding(Thickness::xy(16.0, 4.0))
                    .into()
            },
        ))
        .spacing(0.0),
    )
    .corner_radius(6.0)
    .border_brush(left_border_color)
    .border_thickness(Thickness {
        top: if is_selected || entry.is_pinned { 1.0 } else { 0.0 },
        bottom: 1.0,
        left: if is_selected || entry.is_pinned { 3.0 } else { 1.0 },
        right: 1.0,
    })
    .margin(Thickness::xy(8.0, 2.0))
    .background(card_bg)
    .with_scale_transition(Duration::from_millis(150))
    .animate(AnimationConfig {
        scale: Some(target_scale),
        duration: Duration::from_millis(150),
        easing: Easing::EaseOut,
        ..AnimationConfig::default()
    })
    .on_tapped(move || {
        // Left click: paste
        paste_entry_by_id(entry_id);
    })
    .on_right_tapped(move || {
        // Right click: show context menu
        // Get screen position from cursor
        unsafe {
            use windows::Win32::Foundation::POINT;
            use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            crate::ui::context_menu::show_context_menu(
                entry_id,
                is_pinned,
                &content_type,
                pt.x,
                pt.y,
            );
        }
    })
    .on_pointer_entered(move |_| {
        HOVERED_INDEX.store(index, Ordering::Relaxed);
    })
    .on_pointer_exited(move || {
        HOVERED_INDEX.store(-1, Ordering::Relaxed);
    })
    .into()
}

// ── Type filter pill button ──────────────────────────────────────────────

fn filter_pill(label: &str, is_active: bool) -> Element {
    let btn = button(label.to_string()).font_size(12.0).height(30.0);
    if is_active {
        btn.accent()
    } else {
        btn.subtle()
            .background(Color::rgb(245, 245, 245))
            .foreground(Color::rgb(102, 102, 102))
    }
    .into()
}

// ── Toast overlay ────────────────────────────────────────────────────────

fn toast_overlay() -> Option<Element> {
    let toast_msg = TOAST_MESSAGE.lock().ok()?;
    if toast_msg.is_empty() {
        return None;
    }
    let toast_ts = TOAST_TIMESTAMP.load(Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now.saturating_sub(toast_ts) > 2000 {
        return None;
    }

    Some(
        border(
            text_block(toast_msg.clone())
                .font_size(13.0)
                .foreground(Color::rgb(255, 255, 255))
                .horizontal_alignment(HorizontalAlignment::Center)
                .padding(Thickness::xy(16.0, 8.0))
        )
        .corner_radius(6.0)
        .background(Color::rgb(50, 50, 50))
        .horizontal_alignment(HorizontalAlignment::Center)
        .vertical_alignment(VerticalAlignment::Bottom)
        .margin(Thickness::xy(0.0, 0.0))
        .into()
    )
}

// ── Main render ──────────────────────────────────────────────────────────

pub fn render(cx: &mut RenderCx) -> Element {
    let (_version, set_version) = cx.use_async_state::<u64>(0);
    let (_sel_gen, set_sel_gen) = cx.use_async_state::<u64>(0);
    let (_toast_gen, set_toast_gen) = cx.use_async_state::<u64>(0);
    let (search_query, set_search) = cx.use_state(String::new());
    let (show_settings, set_show_settings) = cx.use_state(false);
    let (show_tag_manager, set_show_tag_manager) = cx.use_state(false);
    let (type_filter, _set_type_filter) = cx.use_state(String::new());

    cx.use_effect((), move || {
        // Load history from DB
        if let Some(guard) = crate::APP_CTX.get() {
            let ctx = guard.lock().unwrap();
            let repo =
                crate::infrastructure::repository::clipboard_repo::SqliteClipboardRepository::new(
                    ctx.db_conn.clone(),
                );
            match repo.get_history(200, 0, None) {
                Ok(history) => {
                    let count = history.len();
                    let mut entries = crate::CLIPBOARD_ENTRIES.lock().unwrap();
                    for entry in &history {
                        entries.push(crate::domain::ClipboardEntryView::from_entry(entry));
                    }
                    crate::info!(
                        "Loaded {} entries from DB into cache (total cache: {})",
                        count,
                        entries.len()
                    );
                }
                Err(e) => {
                    crate::error!("Failed to load history from DB: {}", e);
                }
            }
        } else {
            crate::error!("APP_CTX not set when use_effect runs!");
        }
        let _ = set_version.call(1);

        // Poll for clipboard changes, keyboard navigation changes, and toast
        let set_v = set_version.clone();
        let set_s = set_sel_gen.clone();
        let set_t = set_toast_gen.clone();
        std::thread::spawn(move || {
            let mut last_sel = SELECTED_INDEX.load(Ordering::Relaxed);
            let mut last_hover = HOVERED_INDEX.load(Ordering::Relaxed);
            let mut last_toast_ts = 0u64;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(200)); // Faster polling for toast
                let mut changed = false;
                if let Some(guard) = crate::APP_CTX.get() {
                    let ctx = guard.lock().unwrap();
                    loop {
                        match ctx.monitor_rx.try_recv() {
                            Ok(
                                crate::services::clipboard_monitor::MonitorEvent::ClipboardUpdated(
                                    entry,
                                ),
                            ) => {
                                let view = crate::domain::ClipboardEntryView::from_entry(&entry);
                                let mut entries = crate::CLIPBOARD_ENTRIES.lock().unwrap();
                                entries.insert(0, view);
                                if entries.len() > 200 {
                                    entries.truncate(200);
                                }
                                changed = true;
                            }
                            _ => break,
                        }
                    }
                }
                if changed {
                    let _ = set_v.call(1);
                }
                // Poll for keyboard navigation changes
                let cur_sel = SELECTED_INDEX.load(Ordering::Relaxed);
                if cur_sel != last_sel {
                    last_sel = cur_sel;
                    let _ = set_s.call(1);
                }
                // Poll for hover state changes
                let cur_hover = HOVERED_INDEX.load(Ordering::Relaxed);
                if cur_hover != last_hover {
                    last_hover = cur_hover;
                    // Trigger re-render for hover animation
                    let _ = set_v.call(1);
                }
                // Poll for toast changes
                let cur_toast_ts = TOAST_TIMESTAMP.load(Ordering::Relaxed);
                if cur_toast_ts != last_toast_ts {
                    last_toast_ts = cur_toast_ts;
                    let _ = set_t.call(1);
                }
            }
        });
    });

    if show_settings {
        let back_set = set_show_settings.clone();
        vstack((
            TitleBar::new("设置")
                .back_button_visible(true)
                .back_button_enabled(true)
                .on_back_requested(move || {
                    let _ = back_set.call(false);
                }),
            Element::from(crate::ui::settings_window::render_settings(cx)),
        ))
        .background(Color::rgb(249, 249, 249))
        .into()
    } else if show_tag_manager {
        // Tag Manager view
        let tag_content = crate::ui::tag_manager::render(cx);
        let back_set2 = set_show_tag_manager.clone();

        vstack((
            TitleBar::new("标签管理")
                .back_button_visible(true)
                .back_button_enabled(true)
                .on_back_requested(move || {
                    let _ = back_set2.call(false);
                }),
            Element::from(tag_content),
        ))
        .background(Color::rgb(249, 249, 249))
        .into()
    } else {
        let all_entries = match crate::CLIPBOARD_ENTRIES.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                // Mutex poisoned or locked, show empty state
                return grid((
                    TitleBar::new("")
                        .footer(
                            command_bar(vec![
                                app_bar_button_icon("Settings", Symbol::Setting),
                            ])
                            .default_label_position(CommandBarDefaultLabelPosition::Collapsed),
                        )
                        .grid_row(0),
                    Element::from(
                        vstack((
                            text_block("\u{1F4CB}".to_string())
                                .font_size(48.0)
                                .foreground(Color::rgb(204, 204, 204)),
                            text_block("加载中...".to_string())
                                .font_size(15.0)
                                .foreground(Color::rgb(153, 153, 153)),
                        ))
                        .spacing(12.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .horizontal_alignment(HorizontalAlignment::Center)
                        .padding(Thickness::uniform(40.0))
                        .background(Color::rgb(255, 255, 255))
                        .grid_row(1),
                    ),
                ))
                .rows([GridLength::Auto, GridLength::STAR])
                .background(Color::rgb(249, 249, 249))
                .into();
            }
        };
        let _count = all_entries.len();
        let tf = type_filter.clone();
        let search_lower = if search_query.is_empty() {
            String::new()
        } else {
            search_query.to_lowercase()
        };

        // Detect tag:xxx search syntax
        let tag_filter = if search_lower.starts_with("tag:") {
            search_lower[4..].trim().to_string()
        } else {
            String::new()
        };
        let text_search = if tag_filter.is_empty() {
            search_lower.clone()
        } else {
            String::new() // Don't do text search when filtering by tag
        };

        let filtered: Vec<Element> = all_entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let search_match = text_search.is_empty()
                    || e.preview.to_lowercase().contains(&text_search)
                    || e.source_app.to_lowercase().contains(&text_search);
                let tag_match = tag_filter.is_empty()
                    || e.tags.iter().any(|t| t.to_lowercase() == tag_filter);
                let type_match = tf.is_empty() || e.content_type == tf;
                search_match && tag_match && type_match
            })
            .map(|(i, e)| clipboard_item(e, i as i32))
            .collect();
        let item_count = filtered.len() as i32;
        LIST_ITEM_COUNT.store(item_count, Ordering::Relaxed);
        drop(all_entries);

        let settings_set = set_show_settings.clone();
        let tag_set = set_show_tag_manager.clone();
        let tf_text = type_filter.clone();
        let tf_img = type_filter.clone();
        let tf_file = type_filter.clone();
        let tf_url = type_filter.clone();
        let tf_code = type_filter.clone();
        let tf_video = type_filter.clone();
        let tf_rich = type_filter.clone();
        let _tf_clear = type_filter.clone();

        // Toast overlay
        let toast_elem = toast_overlay();

        grid((
            // Row 0: TitleBar with CommandBar icon buttons in footer
            TitleBar::new("")
                .footer(
                    command_bar(vec![
                        app_bar_button_icon("Pin", Symbol::Pin),
                        app_bar_button_icon("New", Symbol::NewWindow),
                        app_bar_button_icon("Tag", Symbol::Tag),
                        app_bar_button_icon("Settings", Symbol::Setting),
                    ])
                    .default_label_position(CommandBarDefaultLabelPosition::Collapsed)
                    .on_click(move |label: String| match label.as_str() {
                        "Settings" => {
                            let _ = settings_set.call(true);
                        }
                        "Tag" => {
                            let _ = tag_set.call(true);
                        }
                        "New" => {
                            crate::app::sticky_manager::create_sticky_from_clipboard("New note");
                        }
                        "Pin" => {
                            crate::app::window_manager::toggle_pin();
                            let pinned = crate::state::global_state::WINDOW_PINNED.load(Ordering::Relaxed);
                            show_toast(if pinned { "窗口已置顶" } else { "取消置顶" });
                        }
                        _ => {}
                    }),
                )
                .grid_row(0),
            // Row 1: Search box
            text_box(search_query)
                .placeholder_text("搜索剪切板\u{2026}")
                .on_text_changed(set_search)
                .margin(Thickness::xy(12.0, 4.0))
                .grid_row(1),
            // Row 2: Filter pills
            hstack((
                filter_pill("文本", tf_text == "text"),
                filter_pill("图片", tf_img == "image"),
                filter_pill("文件", tf_file == "file"),
                filter_pill("URL", tf_url == "url"),
                filter_pill("代码", tf_code == "code"),
                filter_pill("视频", tf_video == "video"),
                filter_pill("富文本", tf_rich == "rich_text"),
            ))
            .spacing(6.0)
            .padding(Thickness::xy(12.0, 6.0))
            .grid_row(2),
            // Row 3: Content list
            if filtered.is_empty() {
                Element::from(
                    vstack((
                        text_block("\u{1F4CB}".to_string())
                            .font_size(48.0)
                            .foreground(Color::rgb(204, 204, 204)),
                        text_block("剪切板历史将显示在这里".to_string())
                            .font_size(15.0)
                            .foreground(Color::rgb(153, 153, 153)),
                        text_block("复制内容后会自动显示".to_string())
                            .font_size(12.0)
                            .foreground(Color::rgb(180, 180, 180)),
                    ))
                    .spacing(8.0)
                    .vertical_alignment(VerticalAlignment::Center)
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .padding(Thickness::uniform(40.0))
                    .background(Color::rgb(255, 255, 255))
                    .grid_row(3),
                )
            } else {
                Element::from(
                    scroll_viewer(vstack(filtered).spacing(4.0))
                        .padding(Thickness::uniform(0.0))
                        .grid_row(3),
                )
            },
            // Toast overlay (if visible)
            if let Some(toast) = toast_elem {
                toast.grid_row(3)
            } else {
                Element::from(vstack(()))
            },
        ))
        .rows([
            GridLength::Auto, // Title bar
            GridLength::Auto, // Search
            GridLength::Auto, // Filter pills
            GridLength::STAR, // Content list
        ])
        .background(Color::rgb(249, 249, 249))
        .into()
    }
}
