use iced::widget::{text, Text};
use iced::Element;

pub const FONT: iced::Font = iced::Font::with_name("Icons");

// Load the pre-built Phosphor font from assets
pub const FONT_BYTES: &[u8] = include_bytes!("../../assets/phosphor.ttf");

/// Icon name to Unicode character mapping for Phosphor Regular font
pub fn get_icon_char(name: &str) -> char {
    match name {
        "magnifying-glass" | "search" => '\u{e32e}',
        "magnifying-glass-plus" => '\u{e330}',
        "magnifying-glass-minus" => '\u{e332}',
        "gear" | "settings" => '\u{e212}',
        "trash" => '\u{e49e}',
        "file" => '\u{e1d4}',
        "file-text" | "text" => '\u{e1ea}',
        "folder" => '\u{e204}',
        "folder-open" => '\u{e206}',
        "info" => '\u{e27a}',
        "warning" => '\u{e4e2}',
        "check" => '\u{e0ec}',
        "x" => '\u{e4f6}',
        "plus" => '\u{e3bc}',
        "minus" => '\u{e344}',
        "arrow-right" => '\u{e056}',
        "arrow-left" => '\u{e052}',
        "arrow-up" => '\u{e05a}',
        "arrow-down" => '\u{e04a}',
        "download" => '\u{e194}',
        "copy" => '\u{e154}',
        "database" => '\u{e188}',
        "star" => '\u{e452}',
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
