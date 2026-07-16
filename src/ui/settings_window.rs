/// Settings panel — reads directly from global SettingsState, no per-render allocations.
use std::sync::atomic::Ordering;
use windows_reactor::*;

use crate::infrastructure::repository::clipboard_repo::ClipboardRepository;
use crate::infrastructure::repository::settings_repo::SettingsRepository;
use crate::state::app_state::SettingsState;

// ── Settings render ──────────────────────────────────────────────────────

pub fn render_settings(_cx: &mut RenderCx) -> Element {
    let settings = load_settings_from_db();
    build_settings_panel(&settings)
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn theme_index_from_str(s: &str) -> i32 {
    match s {
        "light" => 0,
        "dark" => 1,
        _ => 2,
    }
}

// ── Setting components ───────────────────────────────────────────────────

fn setting_toggle(
    label: &str,
    current_value: bool,
    setting_key: &'static str,
) -> Element {
    hstack((
        text_block(label.to_string())
            .font_size(14.0)
            .foreground(Color::rgb(51, 51, 51))
            .vertical_alignment(VerticalAlignment::Center)
            .horizontal_alignment(HorizontalAlignment::Stretch),
        ToggleSwitch::new(current_value)
            .on_toggled(move |v| {
                save_bool_setting(setting_key, v);
            })
            .horizontal_alignment(HorizontalAlignment::Right),
    ))
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .padding(Thickness::xy(16.0, 10.0))
    .into()
}

fn setting_button(
    label: &str,
    button_text: &str,
    on_click: impl Fn() + Send + Sync + 'static,
) -> Element {
    hstack((
        text_block(label.to_string())
            .font_size(14.0)
            .foreground(Color::rgb(51, 51, 51))
            .vertical_alignment(VerticalAlignment::Center)
            .horizontal_alignment(HorizontalAlignment::Stretch),
        button(button_text.to_string())
            .accent()
            .on_click(on_click)
            .horizontal_alignment(HorizontalAlignment::Right),
    ))
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .padding(Thickness::xy(16.0, 10.0))
    .into()
}

fn setting_combobox(
    label: &str,
    options: &[&str],
    current_index: i32,
    setting_key: &'static str,
) -> Element {
    let options_owned: Vec<String> = options.iter().map(|s| s.to_string()).collect();
    let options_for_save = options_owned.clone();
    hstack((
        text_block(label.to_string())
            .font_size(14.0)
            .foreground(Color::rgb(51, 51, 51))
            .vertical_alignment(VerticalAlignment::Center)
            .horizontal_alignment(HorizontalAlignment::Stretch),
        ComboBox::new(options_owned)
            .placeholder_text("Select...".to_string())
            .selected_index(current_index)
            .on_selection_changed(move |i: i32| {
                let val = options_for_save.get(i as usize).map(|s| s.as_str()).unwrap_or("");
                save_string_setting(setting_key, val);
            })
            .horizontal_alignment(HorizontalAlignment::Right),
    ))
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .padding(Thickness::xy(16.0, 10.0))
    .into()
}

fn setting_number_box(
    label: &str,
    current_value: f64,
    min: f64,
    max: f64,
    setting_key: &'static str,
) -> Element {
    hstack((
        text_block(label.to_string())
            .font_size(14.0)
            .foreground(Color::rgb(51, 51, 51))
            .vertical_alignment(VerticalAlignment::Center)
            .horizontal_alignment(HorizontalAlignment::Stretch),
        NumberBox::new(current_value)
            .range(min, max)
            .on_value_changed(move |v: f64| {
                save_string_setting(setting_key, &v.to_string());
            })
            .horizontal_alignment(HorizontalAlignment::Right),
    ))
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .padding(Thickness::xy(16.0, 10.0))
    .into()
}

fn setting_slider(
    label: &str,
    current_value: f64,
    min: f64,
    max: f64,
    step: f64,
    setting_key: &'static str,
    unit: &str,
) -> Element {
    let unit_str = unit.to_string();
    vstack((
        hstack((
            text_block(label.to_string())
                .font_size(14.0)
                .foreground(Color::rgb(51, 51, 51))
                .vertical_alignment(VerticalAlignment::Center)
                .horizontal_alignment(HorizontalAlignment::Stretch),
            text_block(format!("{current_value:.0}{unit_str}"))
                .font_size(13.0)
                .foreground(Color::rgb(102, 102, 102))
                .horizontal_alignment(HorizontalAlignment::Right),
        ))
        .padding(Thickness::xy(16.0, 8.0)),
        Slider::new(current_value)
            .range(min, max)
            .step(step)
            .margin(Thickness::xy(16.0, 0.0))
            .on_value_changed(move |v: f64| {
                save_string_setting(setting_key, &v.to_string());
            }),
    ))
    .into()
}

// ── Card section ─────────────────────────────────────────────────────────

fn section_card(content: Element, title: &str, expanded: bool) -> Element {
    Expander::new(content)
        .header(title.to_string())
        .expanded(expanded)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

// ── DB helpers ───────────────────────────────────────────────────────────

pub fn save_bool_setting(key: &str, value: bool) {
    if let Some(db_conn) = crate::state::global_state::DB_CONN.get() {
        let repo = crate::infrastructure::repository::settings_repo::SqliteSettingsRepository::new(
            db_conn.clone(),
        );
        let _ = repo.set(key, if value { "true" } else { "false" });
    }
}

pub fn save_string_setting(key: &str, value: &str) {
    if let Some(db_conn) = crate::state::global_state::DB_CONN.get() {
        let repo = crate::infrastructure::repository::settings_repo::SqliteSettingsRepository::new(
            db_conn.clone(),
        );
        let _ = repo.set(key, value);
    }
}

// ── Build settings panel ─────────────────────────────────────────────────

fn load_settings_from_db() -> SettingsState {
    if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        SettingsState::from_ref(&ctx.settings)
    } else {
        SettingsState::default()
    }
}

fn build_settings_panel(settings: &SettingsState) -> Element {
    let color_mode_idx = theme_index_from_str(&settings.color_mode.lock().unwrap());
    let paste_method_idx = match settings.paste_method.lock().unwrap().as_str() {
        "ctrl_v" => 1,
        "game_mode" => 2,
        _ => 0,
    };
    let item_font_size = *settings.clipboard_item_font_size.lock().unwrap();
    let tag_font_size = *settings.clipboard_tag_font_size.lock().unwrap();
    let privacy_kinds = settings.privacy_protection_kinds.lock().unwrap().clone();

    let content = vstack((
        // General settings
        section_card(
            vstack(vec![
                setting_toggle("开机自启动", settings.auto_start.load(Ordering::Relaxed), "app.autostart"),
                setting_toggle("隐藏托盘图标", settings.hide_tray_icon.load(Ordering::Relaxed), "app.hide_tray_icon"),
                setting_toggle("贴边自动收纳", settings.edge_docking.load(Ordering::Relaxed), "app.edge_docking"),
                setting_combobox("面板呼出位置", &["随鼠标位置", "随光标位置", "屏幕居中"],
                    if settings.follow_caret.load(Ordering::Relaxed) { 1 }
                    else if !settings.follow_mouse.load(Ordering::Relaxed) { 2 }
                    else { 0 },
                    "app.popup_position"),
                setting_toggle("按键音效", settings.sound_enabled.load(Ordering::Relaxed), "app.sound_enabled"),
                setting_toggle("粘贴音效", settings.sound_paste_enabled.load(Ordering::Relaxed), "app.sound_paste_enabled"),
                setting_toggle("贴图功能", settings.sticky_enabled.load(Ordering::Relaxed), "app.sticky_enabled"),
                setting_toggle("静默启动", settings.silent_start.load(Ordering::Relaxed), "app.silent_start"),
                setting_toggle("显示回到顶部按钮", settings.quick_paste_enabled.load(Ordering::Relaxed), "app.quick_paste_enabled"),
                setting_toggle("标签管理页开关", settings.auto_hide_tags.load(Ordering::Relaxed), "app.auto_hide_tags"),
                setting_toggle("方向键选择", settings.arrow_key_selection.load(Ordering::Relaxed), "app.arrow_key_selection"),
            ]).into(), "常规设置", false),

        // Clipboard settings
        section_card(vstack(vec![
            setting_toggle("持久化存储", settings.persistent.load(Ordering::Relaxed), "app.persistent"),
            setting_toggle("启用存储上限", settings.persistent_limit_enabled.load(Ordering::Relaxed), "app.persistent_limit_enabled"),
            setting_number_box("存储上限", settings.persistent_limit.load(Ordering::Relaxed) as f64, 50.0, 99999.0, "app.persistent_limit"),
            setting_toggle("重复内容合并", settings.deduplicate.load(Ordering::Relaxed), "app.deduplicate"),
            setting_toggle("记录文件复制", settings.capture_files.load(Ordering::Relaxed), "app.capture_files"),
            setting_toggle("捕获带格式的文本", settings.capture_rich_text.load(Ordering::Relaxed), "app.capture_rich_text"),
            setting_toggle("富文本快照预览 (Beta)", settings.rich_text_snapshot_preview.load(Ordering::Relaxed), "app.rich_text_snapshot_preview"),
            setting_toggle("自动隐藏标签", settings.auto_hide_tags.load(Ordering::Relaxed), "app.auto_hide_tags"),
            setting_toggle("折叠置顶记录", settings.pinned_collapsed.load(Ordering::Relaxed), "app.pinned_collapsed"),
            setting_toggle("粘贴后自动删除", settings.delete_after_paste.load(Ordering::Relaxed), "app.delete_after_paste"),
            setting_toggle("粘贴后置顶到第一条", settings.move_to_top_after_paste.load(Ordering::Relaxed), "app.move_to_top_after_paste"),
            setting_toggle("快速粘贴 (Ctrl+Shift+数字)", settings.quick_paste_enabled.load(Ordering::Relaxed), "app.quick_paste_enabled"),
            setting_toggle("顺序粘贴模式", settings.sequential_mode.load(Ordering::Relaxed), "app.sequential_mode"),
            setting_combobox("粘贴方案", &["Shift+Insert", "Ctrl+V", "游戏模式"], paste_method_idx, "app.paste_method"),
            setting_toggle("隐私保护", settings.privacy_protection.load(Ordering::Relaxed), "app.privacy_protection"),
            setting_toggle("  手机号", privacy_kinds.contains(&"phone".to_string()), "app.privacy_kind_phone"),
            setting_toggle("  身份证", privacy_kinds.contains(&"idcard".to_string()), "app.privacy_kind_idcard"),
            setting_toggle("  邮箱", privacy_kinds.contains(&"email".to_string()), "app.privacy_kind_email"),
            setting_toggle("  密钥", privacy_kinds.contains(&"secret".to_string()), "app.privacy_kind_secret"),
            setting_toggle("  密码", privacy_kinds.contains(&"password".to_string()), "app.privacy_kind_password"),
            setting_toggle("替换Win+V快捷键", settings.use_win_v_shortcut.load(Ordering::Relaxed), "app.use_win_v_shortcut"),
        ]).into(), "剪贴板设置", false),

        // UI settings
        section_card(vstack((
            setting_combobox("颜色模式", &["浅色", "深色", "跟随系统"], color_mode_idx, "app.color_mode"),
            setting_toggle("显示应用边框", settings.show_app_border.load(Ordering::Relaxed), "app.show_app_border"),
            setting_toggle("显示来源应用图标", settings.show_source_app_icon.load(Ordering::Relaxed), "app.show_source_app_icon"),
            setting_toggle("系统毛玻璃特效", settings.vibrancy_enabled.load(Ordering::Relaxed), "app.vibrancy_enabled"),
            setting_toggle("紧凑模式", settings.compact_mode.load(Ordering::Relaxed), "app.compact_mode"),
            setting_slider("条目字体大小", item_font_size, 8.0, 24.0, 1.0, "app.clipboard_item_font_size", "px"),
            setting_slider("标签字体大小", tag_font_size, 8.0, 16.0, 1.0, "app.clipboard_tag_font_size", "px"),
        )).into(), "界面设置", false),

        // Default apps
        section_card(vstack((
            text_block("默认打开程序设置".to_string())
                .font_size(14.0)
                .foreground(Color::rgb(102, 102, 102))
                .padding(Thickness::xy(16.0, 10.0)),
            text_block("文本、图片、视频、代码、URL 类型可在设置中配置默认打开程序".to_string())
                .font_size(12.0)
                .foreground(Color::rgb(153, 153, 153))
                .padding(Thickness::xy(16.0, 0.0)),
        )).into(), "默认打开程序", false),

        // Data management
        section_card(vstack((
            setting_button("清空历史", "清空", || {
                if let Some(db_conn) = crate::state::global_state::DB_CONN.get() {
                    let repo = crate::infrastructure::repository::clipboard_repo::SqliteClipboardRepository::new(db_conn.clone());
                    if let Err(e) = repo.clear(None) {
                        crate::error!("Failed to clear history: {}", e);
                    } else {
                        let mut entries = crate::CLIPBOARD_ENTRIES.lock().unwrap();
                        entries.clear();
                        crate::info!("History cleared");
                    }
                }
            }),
            setting_button("打开数据目录", "打开", || {
                if let Some(guard) = crate::APP_CTX.get() {
                    let ctx = guard.lock().unwrap();
                    let _ = std::process::Command::new("explorer")
                        .arg(ctx.data_dir.to_string_lossy().to_string())
                        .spawn();
                }
            }),
        )).into(), "数据管理", false),

        Element::from(vstack((
            text_block(format!("clipboard v{}", env!("CARGO_PKG_VERSION")))
                .font_size(13.0)
                .foreground(Color::rgb(153, 153, 153))
                .horizontal_alignment(HorizontalAlignment::Center),
            text_block("WinUI3 native clipboard manager".to_string())
                .font_size(12.0)
                .foreground(Color::rgb(180, 180, 180))
                .horizontal_alignment(HorizontalAlignment::Center),
        ))
        .spacing(4.0)
        .padding(Thickness::xy(0.0, 8.0))),
    ))
    .spacing(4.0)
    .padding(Thickness::uniform(8.0))
    .horizontal_alignment(HorizontalAlignment::Stretch);

    Element::from(content)
}
