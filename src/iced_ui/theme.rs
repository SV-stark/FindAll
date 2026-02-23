use iced::{
    border::Radius,
    widget::{button, container, text_input},
    Background, Border, Color, Shadow, Theme,
};

pub fn accent_color() -> Color {
    Color::from_rgb8(99, 102, 241)
}

pub fn accent_color_light() -> Color {
    Color::from_rgba8(99, 102, 241, 0.15)
}

fn is_dark_theme(theme: &Theme) -> bool {
    matches!(theme, Theme::Dark)
}

pub fn primary_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, _status: button::Status| {
        let accent = accent_color();

        button::Style {
            background: Some(Background::Color(accent)),
            text_color: Color::WHITE,
            border: Border {
                color: accent,
                width: 1.0,
                radius: Radius::from(8.0),
            },
            shadow: Shadow {
                color: Color::from_rgba8(99, 102, 241, 0.3),
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 4.0,
            },
            ..Default::default()
        }
    }
}

pub fn secondary_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme: &Theme, _status: button::Status| {
        let is_dark = is_dark_theme(theme);
        let bg = if is_dark {
            Color::from_rgb8(51, 65, 85)
        } else {
            Color::from_rgb8(241, 245, 249)
        };

        button::Style {
            background: Some(Background::Color(bg)),
            text_color: if is_dark {
                Color::from_rgb8(226, 232, 240)
            } else {
                Color::from_rgb8(30, 41, 59)
            },
            border: Border {
                color: if is_dark {
                    Color::from_rgb8(71, 85, 105)
                } else {
                    Color::from_rgb8(203, 213, 225)
                },
                width: 1.0,
                radius: Radius::from(8.0),
            },
            shadow: Shadow::default(),
            ..Default::default()
        }
    }
}

pub fn ghost_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme: &Theme, _status: button::Status| {
        let is_dark = is_dark_theme(theme);

        button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: if is_dark {
                Color::from_rgb8(148, 163, 175)
            } else {
                Color::from_rgb8(100, 116, 139)
            },
            border: Border::default(),
            shadow: Shadow::default(),
            ..Default::default()
        }
    }
}

pub fn icon_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    ghost_button()
}

pub fn search_input() -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |theme: &Theme, _status: text_input::Status| {
        let is_dark = is_dark_theme(theme);

        text_input::Style {
            background: Background::Color(if is_dark {
                Color::from_rgb8(30, 41, 59)
            } else {
                Color::from_rgb8(255, 255, 255)
            }),
            border: Border {
                color: if is_dark {
                    Color::from_rgb8(71, 85, 105)
                } else {
                    Color::from_rgb8(203, 213, 225)
                },
                width: 1.0,
                radius: Radius::from(10.0),
            },
            icon: Color::from_rgb8(100, 116, 139),
            placeholder: Color::from_rgb8(100, 116, 139),
            value: Color::from_rgb8(226, 232, 240),
            selection: accent_color_light(),
        }
    }
}

pub fn small_input() -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |theme: &Theme, _status: text_input::Status| {
        let is_dark = is_dark_theme(theme);

        text_input::Style {
            background: Background::Color(if is_dark {
                Color::from_rgb8(30, 41, 59)
            } else {
                Color::from_rgb8(249, 250, 251)
            }),
            border: Border {
                color: if is_dark {
                    Color::from_rgb8(71, 85, 105)
                } else {
                    Color::from_rgb8(203, 213, 225)
                },
                width: 1.0,
                radius: Radius::from(6.0),
            },
            icon: Color::from_rgb8(100, 116, 139),
            placeholder: Color::from_rgb8(100, 116, 139),
            value: Color::from_rgb8(226, 232, 240),
            selection: accent_color_light(),
        }
    }
}

pub fn sidebar_container(_theme: &Theme) -> container::Style {
    container::Style::default()
}

pub fn main_content_container(_theme: &Theme) -> container::Style {
    container::Style::default()
}

pub fn top_bar_container(_theme: &Theme) -> container::Style {
    container::Style::default()
}

pub fn padded_card_container(_theme: &Theme) -> container::Style {
    container::Style::default()
}

pub fn modern_card(_theme: &Theme) -> container::Style {
    container::Style::default()
}

pub fn result_card_hover(_theme: &Theme) -> container::Style {
    let accent = accent_color();
    container::Style {
        background: Some(Background::Color(accent_color_light())),
        border: Border {
            color: accent,
            width: 1.0,
            radius: Radius::from(10.0),
        },
        ..Default::default()
    }
}

pub fn result_card_normal(_theme: &Theme) -> container::Style {
    container::Style::default()
}

pub fn result_button(is_selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, _status: button::Status| {
        if is_selected {
            let accent = accent_color();
            button::Style {
                background: Some(Background::Color(accent)),
                text_color: Color::WHITE,
                border: Border {
                    color: accent,
                    width: 1.0,
                    radius: Radius::from(8.0),
                },
                shadow: Shadow {
                    color: Color::from_rgba8(99, 102, 241, 0.3),
                    offset: iced::Vector::new(0.0, 2.0),
                    blur_radius: 4.0,
                },
                ..Default::default()
            }
        } else {
            button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                text_color: Color::from_rgb8(148, 163, 175),
                border: Border::default(),
                shadow: Shadow::default(),
                ..Default::default()
            }
        }
    }
}

pub fn input_container(_theme: &Theme) -> container::Style {
    container::Style::default()
}

pub fn tab_button(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, _status: button::Status| {
        if is_active {
            let accent = accent_color();
            button::Style {
                background: Some(Background::Color(accent)),
                text_color: Color::WHITE,
                border: Border {
                    color: accent,
                    width: 1.0,
                    radius: Radius::from(8.0),
                },
                shadow: Shadow::default(),
                ..Default::default()
            }
        } else {
            button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                text_color: Color::from_rgb8(148, 163, 175),
                border: Border::default(),
                shadow: Shadow::default(),
                ..Default::default()
            }
        }
    }
}

pub fn tab_active() -> impl Fn(&Theme, button::Status) -> button::Style {
    tab_button(true)
}

pub fn tab_inactive() -> impl Fn(&Theme, button::Status) -> button::Style {
    tab_button(false)
}
