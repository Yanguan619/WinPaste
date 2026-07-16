use windows_reactor::*;

use crate::infrastructure::repository::clipboard_repo::ClipboardRepository;
use crate::infrastructure::repository::tag_repo::SqliteTagRepository;
use crate::ui::main_window;
use crate::ui::styles;

/// Render the tag manager view.
pub fn render(cx: &mut RenderCx) -> Element {
    let (_refresh, _set_refresh) = cx.use_async_state::<u64>(0);
    let (selected_tag, set_selected_tag) = cx.use_state(String::new());
    let (tag_search, set_tag_search) = cx.use_state(String::new());

    // Load tags from DB
    let tags: Vec<(String, i64)> = if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        let tag_repo = SqliteTagRepository::new(ctx.db_conn.clone());
        tag_repo.get_all_tags_with_count().unwrap_or_default()
    } else {
        Vec::new()
    };

    // Filter tags by search
    let filtered_tags: Vec<(String, i64)> = if tag_search.is_empty() {
        tags.clone()
    } else {
        let search_lower = tag_search.to_lowercase();
        tags.iter()
            .filter(|(name, _)| name.to_lowercase().contains(&search_lower))
            .cloned()
            .collect()
    };

    // Load items for selected tag
    let tag_items: Vec<crate::domain::ClipboardEntry> = if !selected_tag.is_empty() {
        if let Some(guard) = crate::APP_CTX.get() {
            let ctx = guard.lock().unwrap();
            let tag_repo = SqliteTagRepository::new(ctx.db_conn.clone());
            tag_repo
                .get_entries_by_tag(&selected_tag)
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let selected_clone = selected_tag.clone();
    let set_tag = set_selected_tag.clone();
    let theme = styles::get_theme_colors();

    // Left sidebar: tag list
    let tag_list_items: Vec<Element> = filtered_tags
        .iter()
        .map(|(name, count)| {
            let tag_name = name.clone();
            let tag_name2 = name.clone();
            let is_active = selected_clone == *name;
            let set_tag_inner = set_tag.clone();

            let bg = if is_active {
                styles::to_color(theme.accent)
            } else {
                Color::rgb(245, 245, 245)
            };
            let fg = if is_active {
                Color::rgb(255, 255, 255)
            } else {
                styles::to_color(theme.text_primary)
            };

            border(
                hstack((
                    text_block(format!("#{}", tag_name))
                        .font_size(13.0)
                        .foreground(fg)
                        .horizontal_alignment(HorizontalAlignment::Left),
                    text_block(count.to_string())
                        .font_size(11.0)
                        .foreground(if is_active {
                            Color::rgb(200, 220, 255)
                        } else {
                            styles::to_color(theme.text_muted)
                        })
                        .horizontal_alignment(HorizontalAlignment::Right),
                ))
                .spacing(8.0)
                .padding(Thickness::xy(12.0, 8.0)),
            )
            .corner_radius(6.0)
            .background(bg)
            .margin(Thickness::xy(4.0, 2.0))
            .on_tapped(move || {
                set_tag_inner.call(tag_name2.clone());
            })
            .into()
        })
        .collect();

    // Right content: items for selected tag
    let item_elements: Vec<Element> = tag_items.iter().map(|entry| {
        let preview = if entry.preview.len() > 80 {
            format!("{}...", &entry.preview[..80])
        } else {
            entry.preview.clone()
        };
        let entry_id = entry.id;
        let tag_for_color = if !selected_tag.is_empty() { selected_tag.clone() } else { "default".to_string() };
        let chip_color = main_window::tag_chip_color(&tag_for_color);

        border(
            vstack((
                hstack((
                    border(text_block(String::new()))
                        .width(6.0)
                        .height(6.0)
                        .corner_radius(3.0)
                        .background(chip_color),
                    text_block(entry.content_type.clone())
                        .font_size(11.0)
                        .foreground(styles::to_color(theme.text_muted)),
                ))
                .spacing(6.0)
                .vertical_alignment(VerticalAlignment::Center),
                text_block(preview)
                    .font_size(13.0)
                    .foreground(styles::to_color(theme.text_primary))
                    .horizontal_alignment(HorizontalAlignment::Left),
            ))
            .spacing(4.0)
            .padding(Thickness::xy(12.0, 8.0))
        )
        .corner_radius(4.0)
        .background(styles::to_color(theme.card_background))
        .margin(Thickness::xy(0.0, 2.0))
        .on_tapped(move || {
            // Paste this entry
            if let Some(guard) = crate::APP_CTX.get() {
                let ctx = guard.lock().unwrap();
                let repo = crate::infrastructure::repository::clipboard_repo::SqliteClipboardRepository::new(ctx.db_conn.clone());
                if let Ok(Some(entry)) = repo.get_entry_by_id(entry_id) {
                    drop(ctx);
                    let _ = crate::services::clipboard_ops::paste_entry(&entry);
                }
            }
        })
        .into()
    }).collect();

    let content_area = if selected_tag.is_empty() {
        Element::from(
            vstack((text_block("选择一个标签查看内容".to_string())
                .font_size(14.0)
                .foreground(styles::to_color(theme.text_muted)),))
            .vertical_alignment(VerticalAlignment::Center)
            .horizontal_alignment(HorizontalAlignment::Center),
        )
    } else {
        Element::from(
            vstack((
                hstack((
                    text_block(format!("#{}", selected_tag))
                        .font_size(16.0)
                        .font_weight(600u16)
                        .foreground(styles::to_color(theme.text_primary)),
                    text_block(format!("{} 条记录", tag_items.len()))
                        .font_size(12.0)
                        .foreground(styles::to_color(theme.text_muted)),
                ))
                .spacing(8.0)
                .padding(Thickness::xy(16.0, 12.0)),
                scroll_viewer(vstack(item_elements).spacing(2.0)).padding(Thickness::xy(8.0, 0.0)),
            ))
            .spacing(0.0),
        )
    };

    // Main layout: sidebar + content
    grid((
        // Sidebar
        border(
            vstack((
                text_block("标签管理".to_string())
                    .font_size(14.0)
                    .font_weight(600u16)
                    .foreground(styles::to_color(theme.text_primary))
                    .padding(Thickness::xy(12.0, 12.0)),
                // Search box
                text_box(tag_search)
                    .placeholder_text("搜索标签...".to_string())
                    .on_text_changed(set_tag_search)
                    .margin(Thickness::xy(8.0, 0.0)),
                // Tag list
                scroll_viewer(vstack(tag_list_items).spacing(0.0)).padding(Thickness::xy(0.0, 4.0)),
            ))
            .spacing(4.0)
            .padding(Thickness::uniform(4.0)),
        )
        .corner_radius(8.0)
        .background(styles::to_color(theme.card_background))
        .width(220.0)
        .grid_column(0),
        // Content area
        content_area.grid_column(1),
    ))
    .columns([GridLength::STAR, GridLength::STAR])
    .padding(Thickness::uniform(8.0))
    .background(styles::to_color(theme.background))
    .into()
}
