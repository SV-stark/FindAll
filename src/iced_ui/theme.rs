use iced::{
    Background, Border, Color, Theme,
    border::Radius,
    widget::{button, container, text, text_input},
};

// --- Color Palette (Windows 11 Fluent UI Standards) ---
pub const SURFACE_DARK: Color = Color::from_rgb(0.125, 0.125, 0.125); // #202020 (Fluent Mica Dark)
pub const PANEL_DARK: Color = Color::from_rgb(0.176, 0.176, 0.176); // #2d2d2d (Fluent Solid Layer)
pub const BORDER_DARK: Color = Color::from_rgb(0.22, 0.22, 0.24); // #38383d (Fluent Subtle Border)
pub const ACCENT_BLUE: Color = Color::from_rgb(0.0, 0.47, 0.83); // #0078d4 (Fluent Windows Accent)
pub const ACCENT_BLUE_HOVER: Color = Color::from_rgb(0.06, 0.52, 0.89); // #1084d8
pub const HIT_AMBER: Color = Color::from_rgb(0.96, 0.62, 0.04); // #f59e0b

pub const TEXT_BRIGHT: Color = Color::from_rgb(0.98, 0.98, 0.98); // #fafafa
pub const TEXT_MUTED: Color = Color::from_rgb(0.82, 0.82, 0.82); // #d1d1d1
pub const TEXT_DIM: Color = Color::from_rgb(0.55, 0.55, 0.55); // #8e8e8e

// --- Light Theme Colors ---
pub const SURFACE_LIGHT: Color = Color::from_rgb(0.95, 0.95, 0.96); // #f3f3f5 (Fluent Light Surface)
pub const PANEL_LIGHT: Color = Color::from_rgb(1.0, 1.0, 1.0); // #ffffff (Fluent Light Card)
pub const BORDER_LIGHT: Color = Color::from_rgb(0.88, 0.88, 0.90); // #e0e0e6 (Fluent Light Border)

pub const TEXT_BRIGHT_LIGHT: Color = Color::from_rgb(0.10, 0.10, 0.12); // #1a1a1e
pub const TEXT_MUTED_LIGHT: Color = Color::from_rgb(0.37, 0.37, 0.40); // #5e5e66
pub const TEXT_DIM_LIGHT: Color = Color::from_rgb(0.55, 0.55, 0.60); // #8e8e99

#[must_use]
pub const fn accent_color() -> Color {
    ACCENT_BLUE
}

#[must_use]
pub const fn accent_color_light() -> Color {
    let mut c = ACCENT_BLUE;
    c.a = 0.15;
    c
}

const fn is_dark_theme(theme: &Theme) -> bool {
    matches!(theme, Theme::Dark)
}

const fn surface_color(theme: &Theme) -> Color {
    if is_dark_theme(theme) {
        SURFACE_DARK
    } else {
        SURFACE_LIGHT
    }
}

const fn panel_color(theme: &Theme) -> Color {
    if is_dark_theme(theme) {
        PANEL_DARK
    } else {
        PANEL_LIGHT
    }
}

const fn border_color(theme: &Theme) -> Color {
    if is_dark_theme(theme) {
        BORDER_DARK
    } else {
        BORDER_LIGHT
    }
}

const fn text_bright_color(theme: &Theme) -> Color {
    if is_dark_theme(theme) {
        TEXT_BRIGHT
    } else {
        TEXT_BRIGHT_LIGHT
    }
}

const fn text_muted_color(theme: &Theme) -> Color {
    if is_dark_theme(theme) {
        TEXT_MUTED
    } else {
        TEXT_MUTED_LIGHT
    }
}

const fn text_dim_color(theme: &Theme) -> Color {
    if is_dark_theme(theme) {
        TEXT_DIM
    } else {
        TEXT_DIM_LIGHT
    }
}

// --- Container Styles ---

#[must_use]
pub fn main_content_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(surface_color(theme))),
        text_color: Some(text_bright_color(theme)),
        ..Default::default()
    }
}

#[must_use]
pub fn sidebar_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(panel_color(theme))),
        border: Border {
            color: border_color(theme),
            width: 1.0,
            radius: Radius::from(0.0),
        },
        ..Default::default()
    }
}

#[must_use]
pub fn side_nav_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    }
}

#[must_use]
pub fn top_bar_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(panel_color(theme))),
        border: Border {
            color: border_color(theme),
            width: 1.0,
            radius: Radius::from(0.0),
        },
        ..Default::default()
    }
}

#[must_use]
pub fn header_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(panel_color(theme))),
        border: Border {
            color: border_color(theme),
            width: 1.0,
            radius: Radius::from(0.0),
        },
        ..Default::default()
    }
}

#[must_use]
pub fn input_container(theme: &Theme) -> container::Style {
    let is_dark = is_dark_theme(theme);
    container::Style {
        background: Some(Background::Color(if is_dark {
            Color::from_rgb(0.14, 0.14, 0.15)
        } else {
            Color::from_rgb(0.97, 0.97, 0.98)
        })),
        border: Border {
            color: if is_dark {
                Color::from_rgb(0.28, 0.28, 0.30)
            } else {
                Color::from_rgb(0.82, 0.82, 0.85)
            },
            width: 1.0,
            radius: Radius::from(8.0),
        },
        ..Default::default()
    }
}

#[must_use]
pub fn result_card_normal(theme: &Theme) -> container::Style {
    let is_dark = is_dark_theme(theme);
    container::Style {
        background: Some(Background::Color(if is_dark {
            Color::from_rgba(1.0, 1.0, 1.0, 0.02)
        } else {
            Color::from_rgb(1.0, 1.0, 1.0)
        })),
        border: Border {
            color: border_color(theme),
            width: 1.0,
            radius: Radius::from(8.0),
        },
        ..Default::default()
    }
}

#[must_use]
pub fn result_card_hover(theme: &Theme) -> container::Style {
    let is_dark = is_dark_theme(theme);
    container::Style {
        background: Some(Background::Color(if is_dark {
            Color::from_rgba(1.0, 1.0, 1.0, 0.05)
        } else {
            Color::from_rgb(0.96, 0.96, 0.97)
        })),
        border: Border {
            color: ACCENT_BLUE,
            width: 1.0,
            radius: Radius::from(8.0),
        },
        ..Default::default()
    }
}

#[must_use]
pub fn result_card_selected(theme: &Theme) -> container::Style {
    let is_dark = is_dark_theme(theme);
    container::Style {
        background: Some(Background::Color(if is_dark {
            Color::from_rgba(0.0, 0.47, 0.83, 0.12)
        } else {
            Color::from_rgba(0.0, 0.47, 0.83, 0.08)
        })),
        border: Border {
            color: ACCENT_BLUE,
            width: 1.0,
            radius: Radius::from(8.0),
        },
        ..Default::default()
    }
}

#[must_use]
pub fn table_header_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(panel_color(theme))),
        border: Border {
            color: border_color(theme),
            width: 1.0,
            radius: Radius::from(0.0),
        },
        text_color: Some(text_muted_color(theme)),
        ..Default::default()
    }
}

#[must_use]
pub fn hits_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(panel_color(theme))),
        border: Border {
            color: border_color(theme),
            width: 1.0,
            radius: Radius::from(0.0),
        },
        ..Default::default()
    }
}

#[must_use]
pub fn hit_highlight_container(theme: &Theme) -> container::Style {
    let is_dark = is_dark_theme(theme);
    container::Style {
        background: Some(Background::Color(if is_dark {
            Color::from_rgba(0.96, 0.62, 0.04, 0.08)
        } else {
            Color::from_rgba(0.96, 0.62, 0.04, 0.10)
        })),
        border: Border {
            color: Color::from_rgba(0.96, 0.62, 0.04, 0.35),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        ..Default::default()
    }
}

#[must_use]
pub fn code_block_container(theme: &Theme) -> container::Style {
    let is_dark = is_dark_theme(theme);
    container::Style {
        background: Some(Background::Color(if is_dark {
            Color::from_rgb(0.09, 0.09, 0.10)
        } else {
            Color::from_rgb(0.96, 0.96, 0.97)
        })),
        border: Border {
            color: border_color(theme),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        ..Default::default()
    }
}

// --- Button Styles ---

pub fn primary_button() -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    move |_theme: &Theme, status: button::Status| {
        let base = button::Style {
            background: Some(Background::Color(ACCENT_BLUE)),
            text_color: Color::WHITE,
            border: Border {
                color: ACCENT_BLUE,
                width: 0.0,
                radius: Radius::from(6.0),
            },
            ..Default::default()
        };

        match status {
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(ACCENT_BLUE_HOVER)),
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(Color::from_rgb(0.0, 0.40, 0.72))),
                ..base
            },
            _ => base,
        }
    }
}

pub fn search_button() -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    primary_button()
}

pub fn secondary_button() -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    move |theme: &Theme, status: button::Status| {
        let is_dark = is_dark_theme(theme);
        let bg_normal = if is_dark {
            Color::from_rgba(1.0, 1.0, 1.0, 0.06)
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.04)
        };
        let bg_hover = if is_dark {
            Color::from_rgba(1.0, 1.0, 1.0, 0.10)
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.08)
        };

        let base = button::Style {
            background: Some(Background::Color(bg_normal)),
            text_color: text_bright_color(theme),
            border: Border {
                color: border_color(theme),
                width: 1.0,
                radius: Radius::from(6.0),
            },
            ..Default::default()
        };

        match status {
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(bg_hover)),
                ..base
            },
            _ => base,
        }
    }
}

pub fn clear_filter_button() -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    secondary_button()
}

pub fn ghost_button() -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    move |theme: &Theme, status: button::Status| {
        let hover_bg = if is_dark_theme(theme) {
            Color::from_rgba(1.0, 1.0, 1.0, 0.08)
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.06)
        };

        let base = button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: text_muted_color(theme),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: Radius::from(6.0),
            },
            ..Default::default()
        };

        match status {
            button::Status::Hovered => button::Style {
                text_color: text_bright_color(theme),
                background: Some(Background::Color(hover_bg)),
                ..base
            },
            _ => base,
        }
    }
}

pub fn nav_button(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    move |theme: &Theme, status: button::Status| {
        if is_active {
            button::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.47, 0.83, 0.16))),
                text_color: if is_dark_theme(theme) {
                    Color::from_rgb(0.40, 0.75, 1.0)
                } else {
                    ACCENT_BLUE
                },
                border: Border {
                    color: ACCENT_BLUE,
                    width: 1.0,
                    radius: Radius::from(6.0),
                },
                ..Default::default()
            }
        } else {
            ghost_button()(theme, status)
        }
    }
}

pub fn top_menu_button() -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    move |theme: &Theme, status: button::Status| {
        let mut style = ghost_button()(theme, status);
        style.text_color = text_bright_color(theme);
        style
    }
}

pub fn icon_button() -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    ghost_button()
}

// --- Input Styles ---

pub fn search_input() -> impl Fn(&Theme, text_input::Status) -> text_input::Style + use<> {
    move |theme: &Theme, _status: text_input::Status| text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        icon: text_dim_color(theme),
        placeholder: text_dim_color(theme),
        value: text_bright_color(theme),
        selection: accent_color_light(),
    }
}

pub fn small_input() -> impl Fn(&Theme, text_input::Status) -> text_input::Style + use<> {
    search_input()
}

// --- Text Styles ---

pub fn dim_text_style() -> impl Fn(&Theme) -> text::Style + use<> {
    |theme| text::Style {
        color: Some(text_dim_color(theme)),
    }
}

pub fn muted_text_style() -> impl Fn(&Theme) -> text::Style + use<> {
    |theme| text::Style {
        color: Some(text_muted_color(theme)),
    }
}

pub fn error_text_style() -> impl Fn(&Theme) -> text::Style + use<> {
    |_| text::Style {
        color: Some(Color::from_rgb(0.95, 0.25, 0.25)),
    }
}

pub fn danger_text_style() -> impl Fn(&Theme) -> text::Style + use<> {
    error_text_style()
}

#[must_use]
pub fn error_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.95, 0.25, 0.25, 0.10))),
        border: Border {
            color: Color::from_rgb(0.95, 0.25, 0.25),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        ..Default::default()
    }
}

#[must_use]
pub fn error_banner(theme: &Theme) -> container::Style {
    error_container(theme)
}

#[must_use]
pub fn warning_banner(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.96, 0.62, 0.04, 0.12))),
        border: Border {
            color: Color::from_rgb(0.96, 0.62, 0.04),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        ..Default::default()
    }
}

// --- Badge / Pill Styles ---
#[must_use]
pub fn badge_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(if is_dark_theme(theme) {
            Color::from_rgba(1.0, 1.0, 1.0, 0.06)
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.05)
        })),
        border: Border {
            color: border_color(theme),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        text_color: Some(text_muted_color(theme)),
        ..Default::default()
    }
}

#[must_use]
pub fn accent_badge_container(theme: &Theme) -> container::Style {
    let is_dark = is_dark_theme(theme);
    container::Style {
        background: Some(Background::Color(if is_dark {
            Color::from_rgba(0.0, 0.47, 0.83, 0.16)
        } else {
            Color::from_rgba(0.0, 0.47, 0.83, 0.10)
        })),
        border: Border {
            color: Color::from_rgba(0.0, 0.47, 0.83, 0.35),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        text_color: Some(if is_dark {
            Color::from_rgb(0.40, 0.78, 1.0)
        } else {
            ACCENT_BLUE
        }),
        ..Default::default()
    }
}

// Extension Specific Color Badges for Client visual polish
#[must_use]
pub fn file_badge_container(theme: &Theme, ext: Option<&str>) -> container::Style {
    let is_dark = is_dark_theme(theme);
    let (bg, border, text_c) = match ext.unwrap_or("").to_lowercase().as_str() {
        "pdf" => (
            Color::from_rgba(0.88, 0.20, 0.20, 0.15),
            Color::from_rgba(0.88, 0.20, 0.20, 0.4),
            Color::from_rgb(0.95, 0.35, 0.35),
        ),
        "rs" | "py" | "js" | "ts" | "cpp" | "c" | "cs" | "java" | "go" | "html" | "css"
        | "json" | "toml" | "yaml" => (
            Color::from_rgba(0.38, 0.40, 0.95, 0.15),
            Color::from_rgba(0.38, 0.40, 0.95, 0.4),
            if is_dark {
                Color::from_rgb(0.65, 0.68, 1.0)
            } else {
                Color::from_rgb(0.30, 0.32, 0.85)
            },
        ),
        "md" | "txt" | "doc" | "docx" | "rtf" => (
            Color::from_rgba(0.0, 0.47, 0.83, 0.15),
            Color::from_rgba(0.0, 0.47, 0.83, 0.4),
            if is_dark {
                Color::from_rgb(0.40, 0.78, 1.0)
            } else {
                ACCENT_BLUE
            },
        ),
        "png" | "jpg" | "jpeg" | "svg" | "gif" | "webp" => (
            Color::from_rgba(0.05, 0.60, 0.55, 0.15),
            Color::from_rgba(0.05, 0.60, 0.55, 0.4),
            Color::from_rgb(0.20, 0.75, 0.70),
        ),
        "mp4" | "mkv" | "mp3" | "wav" | "flac" => (
            Color::from_rgba(0.58, 0.20, 0.92, 0.15),
            Color::from_rgba(0.58, 0.20, 0.92, 0.4),
            Color::from_rgb(0.75, 0.40, 1.0),
        ),
        _ => return badge_container(theme),
    };

    container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: border,
            width: 1.0,
            radius: Radius::from(6.0),
        },
        text_color: Some(text_c),
        ..Default::default()
    }
}

// --- Helper for Hit Highlights ---
#[must_use]
pub fn hit_tag_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.96, 0.62, 0.04, 0.15))),
        border: Border {
            color: Color::from_rgba(0.96, 0.62, 0.04, 0.4),
            width: 1.0,
            radius: Radius::from(4.0),
        },
        text_color: Some(HIT_AMBER),
        ..Default::default()
    }
}

// Compatibility shims

pub fn result_button(
    is_selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    nav_button(is_selected)
}

pub fn tab_button(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    nav_button(is_active)
}

#[must_use]
pub fn padded_card_container(theme: &Theme) -> container::Style {
    let is_dark = is_dark_theme(theme);
    container::Style {
        background: Some(Background::Color(if is_dark {
            Color::from_rgb(0.16, 0.16, 0.17)
        } else {
            Color::from_rgb(0.97, 0.97, 0.98)
        })),
        border: Border {
            color: border_color(theme),
            width: 1.0,
            radius: Radius::from(8.0),
        },
        ..Default::default()
    }
}

#[must_use]
pub fn sidebar_panel_container(theme: &Theme) -> container::Style {
    padded_card_container(theme)
}

#[must_use]
pub fn error_container_style() -> container::Style {
    error_container(&Theme::Dark)
}
