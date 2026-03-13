use iced::{
    border::Radius,
    widget::{button, container, text, text_input},
    Background, Border, Color, Theme,
};

// --- Color Palette (Inspired by Zinc/Amber Premium Dark) ---
pub const SURFACE_DARK: Color = Color::from_rgb(0.06, 0.06, 0.07); // #0f1115
pub const PANEL_DARK: Color = Color::from_rgb(0.1, 0.11, 0.13); // #1a1c21
pub const BORDER_DARK: Color = Color::from_rgb(0.16, 0.17, 0.2); // #292c33
pub const ACCENT_BLUE: Color = Color::from_rgb(0.23, 0.51, 0.96); // #3b82f6
pub const HIT_AMBER: Color = Color::from_rgb(0.96, 0.62, 0.04); // #f59e0b

pub const TEXT_BRIGHT: Color = Color::from_rgb(0.98, 0.98, 0.98);
pub const TEXT_MUTED: Color = Color::from_rgb(0.63, 0.64, 0.66);
pub const TEXT_DIM: Color = Color::from_rgb(0.40, 0.41, 0.43);

// --- Color Palette (Light) ---
pub const SURFACE_LIGHT: Color = Color::from_rgb(0.98, 0.98, 0.99); // #fafafa
pub const PANEL_LIGHT: Color = Color::from_rgb(0.94, 0.94, 0.96); // #f4f4f5
pub const BORDER_LIGHT: Color = Color::from_rgb(0.89, 0.90, 0.91); // #e4e4e7

pub const TEXT_BRIGHT_LIGHT: Color = Color::from_rgb(0.09, 0.09, 0.11); // #18181b
pub const TEXT_MUTED_LIGHT: Color = Color::from_rgb(0.45, 0.45, 0.50); // #71717a
pub const TEXT_DIM_LIGHT: Color = Color::from_rgb(0.63, 0.63, 0.67); // #a1a1aa

pub fn accent_color() -> Color {
    ACCENT_BLUE
}

pub fn accent_color_light() -> Color {
    let mut c = ACCENT_BLUE;
    c.a = 0.15;
    c
}

fn is_dark_theme(theme: &Theme) -> bool {
    matches!(theme, Theme::Dark)
}

fn surface_color(theme: &Theme) -> Color {
    if is_dark_theme(theme) { SURFACE_DARK } else { SURFACE_LIGHT }
}

fn panel_color(theme: &Theme) -> Color {
    if is_dark_theme(theme) { PANEL_DARK } else { PANEL_LIGHT }
}

fn border_color(theme: &Theme) -> Color {
    if is_dark_theme(theme) { BORDER_DARK } else { BORDER_LIGHT }
}

fn text_bright_color(theme: &Theme) -> Color {
    if is_dark_theme(theme) { TEXT_BRIGHT } else { TEXT_BRIGHT_LIGHT }
}

fn text_muted_color(theme: &Theme) -> Color {
    if is_dark_theme(theme) { TEXT_MUTED } else { TEXT_MUTED_LIGHT }
}

fn text_dim_color(theme: &Theme) -> Color {
    if is_dark_theme(theme) { TEXT_DIM } else { TEXT_DIM_LIGHT }
}

// --- Container Styles ---

pub fn main_content_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(surface_color(theme))),
        text_color: Some(text_bright_color(theme)),
        ..Default::default()
    }
}

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

pub fn side_nav_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    }
}

pub fn top_bar_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(panel_color(theme))),
        border: Border {
            color: border_color(theme),
            width: 0.0,
            radius: Radius::from(0.0),
        },
        ..Default::default()
    }
}

pub fn input_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(surface_color(theme))),
        border: Border {
            color: border_color(theme),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        ..Default::default()
    }
}

pub fn result_card_normal(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: Radius::from(0.0),
        },
        ..Default::default()
    }
}

pub fn result_card_hover(theme: &Theme) -> container::Style {
    let alpha = if is_dark_theme(theme) { 0.05 } else { 0.1 };
    let bg_color = if is_dark_theme(theme) { 
        Color::from_rgba(1.0, 1.0, 1.0, alpha) 
    } else { 
        Color::from_rgba(0.0, 0.0, 0.0, alpha) 
    };

    container::Style {
        background: Some(Background::Color(bg_color)),
        ..Default::default()
    }
}

pub fn result_card_selected(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.23, 0.51, 0.96, 0.2))),
        border: Border {
            color: ACCENT_BLUE,
            width: 1.0,
            radius: Radius::from(2.0),
        },
        ..Default::default()
    }
}

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

// --- Button Styles ---

pub fn primary_button() -> impl Fn(&Theme, button::Status) -> button::Style {
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
                background: Some(Background::Color(Color::from_rgb(0.3, 0.6, 1.0))),
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(Color::from_rgb(0.2, 0.4, 0.8))),
                ..base
            },
            _ => base,
        }
    }
}

pub fn secondary_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme: &Theme, status: button::Status| {
        let bg_hover = if is_dark_theme(theme) {
            Color::from_rgb(0.2, 0.22, 0.25)
        } else {
            Color::from_rgb(0.9, 0.9, 0.92)
        };
        
        let base = button::Style {
            background: Some(Background::Color(border_color(theme))),
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

pub fn ghost_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme: &Theme, status: button::Status| {
        let hover_bg = if is_dark_theme(theme) {
            Color::from_rgba(1.0, 1.0, 1.0, 0.05)
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.05)
        };

        let base = button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: text_muted_color(theme),
            border: Border::default(),
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

pub fn nav_button(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme: &Theme, status: button::Status| {
        if is_active {
            button::Style {
                background: Some(Background::Color(Color::from_rgba(0.23, 0.51, 0.96, 0.15))),
                text_color: ACCENT_BLUE,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: Radius::from(6.0),
                },
                ..Default::default()
            }
        } else {
            ghost_button()(theme, status)
        }
    }
}

pub fn top_menu_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme: &Theme, status: button::Status| {
        let mut style = ghost_button()(theme, status);
        style.text_color = text_bright_color(theme);
        style
    }
}

pub fn icon_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    ghost_button()
}

// --- Input Styles ---

pub fn search_input() -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |theme: &Theme, _status: text_input::Status| text_input::Style {
        background: Background::Color(surface_color(theme)),
        border: Border {
            color: border_color(theme),
            width: 1.0,
            radius: Radius::from(4.0),
        },
        icon: text_dim_color(theme),
        placeholder: text_dim_color(theme),
        value: text_bright_color(theme),
        selection: accent_color_light(),
    }
}

pub fn small_input() -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    search_input()
}

// --- Text Styles ---

pub fn dim_text_style() -> impl Fn(&Theme) -> text::Style {
    |theme| text::Style {
        color: Some(text_dim_color(theme)),
    }
}

pub fn muted_text_style() -> impl Fn(&Theme) -> text::Style {
    |theme| text::Style {
        color: Some(text_muted_color(theme)),
    }
}

pub fn error_text_style() -> impl Fn(&Theme) -> text::Style {
    |_| text::Style {
        color: Some(Color::from_rgb(0.9, 0.2, 0.2)),
    }
}

pub fn error_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.9, 0.2, 0.2, 0.1))),
        border: Border {
            color: Color::from_rgb(0.9, 0.2, 0.2),
            width: 1.0,
            radius: Radius::from(4.0),
        },
        ..Default::default()
    }
}

// --- Helper for Hit Highlights ---
pub fn hit_tag_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.96, 0.62, 0.04, 0.2))),
        border: Border {
            color: HIT_AMBER,
            width: 0.0,
            radius: Radius::from(2.0),
        },
        text_color: Some(HIT_AMBER),
        ..Default::default()
    }
}

// Compatibility shims if needed
pub fn result_button(is_selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    nav_button(is_selected)
}

pub fn tab_button(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    nav_button(is_active)
}

pub fn padded_card_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(panel_color(theme))),
        border: Border {
            color: border_color(theme),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        ..Default::default()
    }
}

pub fn error_container_style() -> container::Style {
    error_container(&Theme::Dark)
}