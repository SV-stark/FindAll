use iced::{border::Radius, widget::container, Background, Border, Color, Shadow, Theme};

pub fn sidebar_container(theme: &Theme) -> container::Style {
    let palette = theme.palette();
    let is_dark = theme == &Theme::Dark;

    container::Style {
        background: Some(Background::Color(if is_dark {
            Color::from_rgb8(30, 30, 40) // Slightly darker for sidebar
        } else {
            Color::from_rgb8(245, 245, 250) // Light gray for sidebar
        })),
        text_color: Some(palette.text),
        border: Border {
            color: if is_dark {
                Color::from_rgb8(50, 50, 65)
            } else {
                Color::from_rgb8(220, 220, 230)
            },
            width: 1.0,
            radius: Radius::from(0.0),
            ..Default::default()
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub fn main_content_container(theme: &Theme) -> container::Style {
    let palette = theme.palette();

    container::Style {
        background: Some(Background::Color(palette.background)),
        text_color: Some(palette.text),
        border: Border::default(),
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub fn top_bar_container(theme: &Theme) -> container::Style {
    let palette = theme.palette();
    let is_dark = theme == &Theme::Dark;

    container::Style {
        background: Some(Background::Color(if is_dark {
            Color::from_rgb8(35, 35, 45)
        } else {
            Color::from_rgb8(250, 250, 252)
        })),
        text_color: Some(palette.text),
        border: Border {
            color: if is_dark {
                Color::from_rgb8(50, 50, 65)
            } else {
                Color::from_rgb8(220, 220, 230)
            },
            width: 1.0,
            radius: Radius::from(0.0),
            ..Default::default()
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, if is_dark { 0.3 } else { 0.05 }),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    }
}

pub fn padded_card_container(theme: &Theme) -> container::Style {
    let palette = theme.palette();
    let is_dark = theme == &Theme::Dark;

    container::Style {
        background: Some(Background::Color(if is_dark {
            Color::from_rgb8(45, 45, 55)
        } else {
            Color::from_rgb8(255, 255, 255)
        })),
        text_color: Some(palette.text),
        border: Border {
            color: if is_dark {
                Color::from_rgb8(65, 65, 80)
            } else {
                Color::from_rgb8(230, 230, 240)
            },
            width: 1.0,
            radius: Radius::from(12.0),
            ..Default::default()
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, if is_dark { 0.15 } else { 0.05 }),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..Default::default()
    }
}

pub fn input_container(theme: &Theme) -> container::Style {
    let palette = theme.palette();
    let is_dark = theme == &Theme::Dark;

    container::Style {
        background: Some(Background::Color(if is_dark {
            Color::from_rgb8(25, 25, 35)
        } else {
            Color::from_rgb8(255, 255, 255)
        })),
        text_color: Some(palette.text),
        border: Border {
            color: if is_dark {
                Color::from_rgb8(60, 60, 75)
            } else {
                Color::from_rgb8(200, 200, 215)
            },
            width: 1.0,
            radius: Radius::from(8.0),
            ..Default::default()
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}
