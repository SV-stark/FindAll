//! Right-click context menu for a result row.
//!
//! Self-contained: it depends on nothing else in the search view, so the
//! interaction logic (which row is targeted, whether it is pinned) can be
//! reasoned about without the results list in view.

use super::super::{ContextMenuState, Message, theme};
use crate::iced_ui::icons::load_icon_size;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Font, Length, Padding};

/// Right-click menu for a single result.
///
/// Positioned in the upper-left of the results pane rather than at the cursor:
/// Iced 0.14 does not expose pointer coordinates through `MouseArea`, and a menu
/// pinned to a known corner is predictable and cannot fall off-screen.
pub(super) fn context_menu_overlay(state: &ContextMenuState) -> Element<'_, Message> {
    let path = state.path.clone();
    let name = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path)
        .to_string();

    let (toggle_pin_label, toggle_pin_icon) = if state.pinned {
        ("Unpin from quick access", "x")
    } else {
        ("Pin to quick access", "star")
    };
    let toggle_pin_message = if state.pinned {
        Message::UnpinFile(path.clone())
    } else {
        Message::PinFile(path.clone())
    };

    let menu = column![
        container(
            column![
                text(name).size(theme::fs(12.0)).font(Font::MONOSPACE),
                text(path)
                    .size(theme::fs(11.0))
                    .style(theme::dim_text_style()),
            ]
            .spacing(2)
        )
        .padding(Padding::new(12.0))
        .style(theme::badge_container)
        .width(Length::Fill),
        container(Space::new().height(1.0))
            .style(theme::hit_highlight_container)
            .width(Length::Fill),
        context_menu_button("file-text", "Open", Message::OpenSelectedResult),
        context_menu_button(
            "folder-open",
            "Show in folder",
            Message::ShowSelectedInFolder
        ),
        context_menu_button("copy", "Copy full path", Message::CopySelectedPath),
        context_menu_button(toggle_pin_icon, toggle_pin_label, toggle_pin_message),
    ]
    .spacing(2)
    .width(Length::Fixed(320.0));

    container(menu)
        .style(theme::padded_card_container)
        .padding(Padding::new(4.0))
        .width(Length::Fixed(332.0))
        .into()
}

fn context_menu_button<'a>(
    icon: &'a str,
    label: &'a str,
    message: Message,
) -> Element<'a, Message> {
    button(
        row![
            load_icon_size(icon, 14.0),
            text(label).size(theme::fs(12.0))
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .on_press(message)
    .padding(Padding::from([10, 12]))
    .width(Length::Fill)
    .style(theme::ghost_button())
    .into()
}
