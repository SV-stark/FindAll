use iced::widget::{text, Text};
use iced::Element;

pub const FONT: iced::Font = iced::Font::with_name("lucide");

// Load the pre-built Lucide font from assets
pub const FONT_BYTES: &[u8] = include_bytes!("../../assets/lucide.ttf");

/// Icon name to Unicode character mapping for Lucide font
pub fn get_icon_char(name: &str) -> char {
    match name {
        "magnifying-glass" | "search" => '\u{e151}',
        "magnifying-glass-plus" => '\u{e1b6}',
        "magnifying-glass-minus" => '\u{e1b7}',
        "gear" | "settings" => '\u{e154}',
        "trash" => '\u{e18d}',
        "file" => '\u{e0c0}',
        "file-text" | "text" => '\u{e0cc}',
        "folder" => '\u{e0d7}',
        "folder-open" => '\u{e247}',
        "info" => '\u{e0f9}',
        "warning" => '\u{e193}',
        "check" => '\u{e06c}',
        "x" => '\u{e1b2}',
        "plus" => '\u{e13d}',
        "minus" => '\u{e11c}',
        "arrow-right" => '\u{e049}',
        "arrow-left" => '\u{e048}',
        "arrow-up" => '\u{e04a}',
        "arrow-down" => '\u{e042}',
        "download" => '\u{e0b2}',
        "copy" => '\u{e09e}',
        "database" => '\u{e0ad}',
        "star" => '\u{e176}',
        _ => {
            eprintln!("icon '{}' not found", name);
            '\u{003f}' // Question mark as fallback
        }
    }
}

/// Helper function to create an iced Text widget containing the given icon unicode.
pub fn icon<'a>(name: &str) -> Text<'a> {
    text(get_icon_char(name).to_string()).font(FONT)
}

/// For the old `load_icon` helper signature
pub fn load_icon<'a>(name: &str) -> Element<'a, super::Message> {
    icon(name).size(16).into()
}

pub fn load_icon_size<'a>(name: &str, size: f32) -> Element<'a, super::Message> {
    icon(name).size(size).into()
}
