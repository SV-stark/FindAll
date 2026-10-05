//! The start screen: hero, recent searches, and the shortcut/feature reference.
//!
//! Shown when the query is empty. Reads only settings, so it can be rendered and
//! reasoned about without the results machinery.

use super::super::{App, Message, theme};
use crate::iced_ui::icons::load_icon_size;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Font, Length, Padding, font};

pub(super) fn welcome_hero_view(app: &App) -> Element<'_, Message> {
    let hero = column![
        Space::new().height(Length::Fixed(16.0)),
        row![
            container(load_icon_size("sparkles", 32.0))
                .padding(14)
                .style(theme::accent_badge_container),
            column![
                text("FindAll Instant Search")
                    .size(theme::fs(22.0))
                    .font(Font {
                        weight: font::Weight::Bold,
                        ..Font::default()
                    }),
                text("Ultrafast local text, document, and filename search engine")
                    .size(theme::fs(13.0))
                    .style(theme::dim_text_style()),
            ]
            .spacing(4),
        ]
        .spacing(16)
        .align_y(Alignment::Center),
        Space::new().height(Length::Fixed(24.0)),
        shortcut_and_feature_cards(),
        Space::new().height(Length::Fixed(20.0)),
        recent_searches_card(app),
        Space::new().height(Length::Fixed(12.0)),
        // System Index Status Pill
        container(
            row![
                load_icon_size("database", 16.0),
                text(format!(
                    "Index Status: {} files indexed ({})",
                    app.files_indexed, app.index_size
                ))
                .size(theme::fs(12.0))
                .style(theme::muted_text_style()),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        )
        .padding(Padding::from([8, 16]))
        .style(theme::badge_container),
    ]
    .spacing(12)
    .padding(Padding::new(28.0))
    .max_width(740.0)
    .align_x(Alignment::Center);

    container(scrollable(hero))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::FillPortion(2))
        .into()
}

#[allow(clippy::too_many_lines)]
/// Recent searches, ranked by how often each was run.
///
/// The history was recorded and persisted from the start but never surfaced,
/// so the feature was invisible. Clicking an entry re-runs that query.
fn recent_searches_card(app: &App) -> Element<'_, Message> {
    const MAX_SHOWN: usize = 6;

    if !app.settings.search_history_enabled {
        return Space::new().height(0).into();
    }

    let history = &app.settings.search_history;
    if history.is_empty() {
        return Space::new().height(0).into();
    }

    let mut rows = column![].spacing(6);
    for item in history.iter().take(MAX_SHOWN) {
        rows = rows.push(
            button(
                row![
                    load_icon_size("search", 13.0),
                    text(&item.query).size(theme::fs(13.0)).width(Length::Fill),
                    text(format!("{}x", item.frequency))
                        .size(theme::fs(11.0))
                        .style(theme::dim_text_style()),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .on_press(Message::RunRecentSearch(item.query.clone()))
            .padding(Padding::from([8, 12]))
            .width(Length::Fill)
            .style(theme::ghost_button()),
        );
    }

    container(
        column![
            row![
                load_icon_size("keyboard", 15.0),
                text("Recent Searches").size(theme::fs(14.0)).font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                }),
                Space::new().width(Length::Fill),
                button(
                    row![
                        load_icon_size("trash", 13.0),
                        text("Clear").size(theme::fs(12.0)),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center)
                )
                .on_press(Message::ClearSearchHistory)
                .padding(Padding::from([6, 10]))
                .style(theme::ghost_button()),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            Space::new().height(Length::Fixed(10.0)),
            rows,
        ]
        .width(Length::Fill),
    )
    .padding(18)
    .style(theme::padded_card_container)
    .width(Length::Fill)
    .into()
}

/// Shortcut reference and feature summary shown on the start screen.
///
/// Kept apart from `welcome_hero_view` so the shortcut list stays readable and
/// independently editable.
fn shortcut_and_feature_cards() -> Element<'static, Message> {
    let card_header = |icon: &'static str, title: &'static str| {
        row![
            load_icon_size(icon, 16.0),
            text(title).size(theme::fs(14.0)).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
    };

    let shortcuts = container(
        column![
            card_header("keyboard", "Keyboard Shortcuts"),
            Space::new().height(Length::Fixed(8.0)),
            shortcut_row("Alt + Space", "Global Search Window"),
            shortcut_row("Ctrl + F", "Focus Search Input"),
            shortcut_row("↑ / ↓", "Navigate Results"),
            shortcut_row("Enter", "Open Selected File"),
            shortcut_row("Ctrl + Enter", "Open Containing Folder"),
            shortcut_row("Ctrl + C", "Copy File Path"),
            shortcut_row("Right Click", "Open Result Menu"),
            shortcut_row("Esc", "Close Menu / Clear Selection"),
            shortcut_row("Double Click", "Run the chosen result action"),
        ]
        .spacing(8),
    )
    .padding(18)
    .style(theme::padded_card_container)
    .width(Length::FillPortion(1));

    let features = container(
        column![
            card_header("star", "Pro Search Capabilities"),
            Space::new().height(Length::Fixed(8.0)),
            feature_tip("Full Text vs Filename", "Toggle search scope in top bar"),
            feature_tip(
                "Extension Filter",
                "Filter PDF, MD, RS, TXT, Code in sidebar",
            ),
            feature_tip("Exact & Case Match", "Use 'Aa' and 'W' flags for precision"),
            feature_tip(
                "Instant Document Preview",
                "Inspect text snippets & tables live",
            ),
            feature_tip(
                "Query Operators",
                "path:, title:, ext:, size:>10MB, modified:>7d"
            ),
        ]
        .spacing(8),
    )
    .padding(18)
    .style(theme::padded_card_container)
    .width(Length::FillPortion(1));

    row![shortcuts, features]
        .spacing(16)
        .width(Length::Fill)
        .into()
}

fn shortcut_row<'a>(key: &'a str, desc: &'a str) -> Element<'a, Message> {
    row![
        container(text(key).size(theme::fs(11.0)).font(Font {
            weight: font::Weight::Bold,
            ..Font::default()
        }))
        .padding(Padding::from([3, 8]))
        .style(theme::badge_container),
        text(desc)
            .size(theme::fs(12.0))
            .style(theme::muted_text_style()),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

fn feature_tip<'a>(title: &'a str, desc: &'a str) -> Element<'a, Message> {
    column![
        text(title).size(theme::fs(12.0)).font(Font {
            weight: font::Weight::Bold,
            ..Font::default()
        }),
        text(desc)
            .size(theme::fs(11.0))
            .style(theme::dim_text_style()),
    ]
    .spacing(2)
    .into()
}
