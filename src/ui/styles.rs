/// Theme, style constants, and color palettes for the clipboard manager UI.

// ── Window dimensions ────────────────────────────────────────────────────

pub const WINDOW_DEFAULT_WIDTH: f32 = 380.0;
pub const WINDOW_DEFAULT_HEIGHT: f32 = 600.0;
pub const WINDOW_MIN_WIDTH: f32 = 80.0;
pub const WINDOW_MIN_HEIGHT: f32 = 400.0;

/// Compact mode dimensions.
pub const COMPACT_WIDTH: f32 = 120.0;
pub const COMPACT_HEIGHT: f32 = 220.0;
/// Number of items visible in compact mode.
pub const COMPACT_VISIBLE_ITEMS: usize = 3;

pub const CORNER_RADIUS: f32 = 8.0;
pub const CONTENT_MARGIN: f64 = 12.0;
pub const ITEM_PADDING: f64 = 8.0;
pub const SECONDARY_TEXT_OPACITY: f64 = 0.6;

/// Edge docking offset from screen edge (pixels).
pub const DOCK_OFFSET: i32 = 0;

// ── Color palettes ───────────────────────────────────────────────────────

pub struct ThemeColors {
    pub background: (u8, u8, u8),
    pub card_background: (u8, u8, u8),
    pub card_hover: (u8, u8, u8),
    pub text_primary: (u8, u8, u8),
    pub text_secondary: (u8, u8, u8),
    pub text_muted: (u8, u8, u8),
    pub accent: (u8, u8, u8),
    pub border: (u8, u8, u8),
    pub search_bg: (u8, u8, u8),
    pub status_bar_bg: (u8, u8, u8),
}

impl ThemeColors {
    pub fn light() -> Self {
        Self {
            background: (249, 249, 249),
            card_background: (255, 255, 255),
            card_hover: (240, 240, 240),
            text_primary: (32, 32, 32),
            text_secondary: (100, 100, 100),
            text_muted: (160, 160, 160),
            accent: (0, 120, 212),
            border: (230, 230, 230),
            search_bg: (255, 255, 255),
            status_bar_bg: (245, 245, 245),
        }
    }

    pub fn dark() -> Self {
        Self {
            background: (32, 32, 32),
            card_background: (44, 44, 44),
            card_hover: (55, 55, 55),
            text_primary: (255, 255, 255),
            text_secondary: (160, 160, 160),
            text_muted: (100, 100, 100),
            accent: (96, 165, 250),
            border: (60, 60, 60),
            search_bg: (50, 50, 50),
            status_bar_bg: (38, 38, 38),
        }
    }
}

/// Get the active theme colors based on the current theme setting.
pub fn get_theme_colors() -> ThemeColors {
    let theme =
        crate::state::global_state::CURRENT_THEME.load(std::sync::atomic::Ordering::Relaxed);
    match theme {
        1 => ThemeColors::dark(),
        _ => ThemeColors::light(), // 0 = light, 2 = system (default to light)
    }
}

/// Apply a color from a palette tuple to a windows_reactor Color.
pub fn to_color(rgb: (u8, u8, u8)) -> windows_reactor::Color {
    windows_reactor::Color::rgb(rgb.0, rgb.1, rgb.2)
}
