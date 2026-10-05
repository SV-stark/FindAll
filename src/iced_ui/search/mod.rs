//! The search screen.
//!
//! This module is the shell: top navigation, the corruption/error banners, the
//! three-column layout, and the status bar. The panels it arranges live in
//! sibling modules:
//!
//! - [`filters`] — sidebar filter sections and the active-filter chips
//! - [`results`] — the result list and its empty states
//! - [`preview`] — the right-hand document preview
//! - [`welcome`] — the start screen shown when the query is empty
//! - [`context_menu`] — the right-click overlay
//! - [`snippet`] — snippet rendering, shared by `results` and `preview`
//!
//! Only the panel entry points are `pub(super)`. They are an implementation
//! detail of this screen; the sole public item is [`search_view`], which
//! `super::view` calls.

mod context_menu;
mod filters;
mod preview;
mod results;
mod snippet;
mod welcome;

use super::{App, Message, SearchMode, Tab, theme};
use crate::iced_ui::icons::load_icon_size;
use context_menu::context_menu_overlay;
use filters::{collapsed_sidebar, filter_chips, left_sidebar};
use iced::widget::{Space, TextInput, button, column, container, mouse_area, row, stack, text};
use iced::{Alignment, Element, Font, Length, Padding, font};
use preview::right_panel;
use results::results_panel;

pub fn search_view(app: &App) -> Element<'_, Message> {
    let mut col = column![top_navigation(app)];

    if let Some(state) = &app.state
        && state.db_corrupted
        && !app.db_corrupted_dismissed
    {
        col = col.push(
            container(
                row![
                    load_icon_size("warning", 16.0),
                    text(" Metadata database was corrupted and has been reset. Full re-index recommended.")
                        .size(theme::fs(13.0))
                        .style(theme::danger_text_style()),
                    Space::new().width(Length::Fill),
                    button(text("Dismiss").size(theme::fs(12.0)))
                        .on_press(Message::DismissError)
                        .padding(Padding::from([4, 8]))
                        .style(theme::ghost_button())
                ]
                .align_y(Alignment::Center)
                .spacing(8)
            )
            .padding(10)
            .style(theme::warning_banner)
            .width(Length::Fill)
        );
    }

    if let Some(err) = &app.search_error {
        col = col.push(
            container(
                row![
                    load_icon_size("warning", 16.0),
                    text(format!("Error: {err}"))
                        .size(theme::fs(13.0))
                        .style(theme::danger_text_style()),
                    Space::new().width(Length::Fill),
                    button(text("Dismiss").size(theme::fs(12.0)))
                        .on_press(Message::DismissError)
                        .padding(Padding::from([4, 8]))
                        .style(theme::ghost_button())
                ]
                .align_y(Alignment::Center)
                .spacing(8),
            )
            .padding(10)
            .style(theme::error_banner)
            .width(Length::Fill),
        );
    }

    col.push(main_layout(app))
        .push(status_bar(app))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[allow(clippy::too_many_lines)]
fn top_navigation(app: &App) -> Element<'_, Message> {
    let logo = row![
        container(load_icon_size("sparkles", 18.0))
            .padding(6)
            .style(theme::accent_badge_container),
        column![
            text("FindAll").size(theme::fs(17.0)).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }),
            text("Instant Local Search")
                .size(theme::fs(10.0))
                .style(theme::dim_text_style()),
        ]
        .spacing(1),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let search_bar = container(
        row![
            load_icon_size("search", 16.0),
            Space::new().width(Length::Fixed(4.0)),
            TextInput::new(
                match app.search_mode {
                    SearchMode::FullText => "Search everything (text, documents, code)...",
                    SearchMode::Filename => "Search filenames...",
                },
                &app.search_query,
            )
            .id(crate::iced_ui::get_search_input_id())
            .on_input(Message::SearchQueryChanged)
            .on_submit(Message::SearchSubmitted)
            .padding(Padding {
                top: 10.0,
                bottom: 10.0,
                left: 8.0,
                right: 8.0,
            })
            .size(theme::fs(15.0))
            .style(theme::search_input())
            .width(Length::Fill),
            if app.search_query.is_empty() {
                Element::from(Space::new().width(0).height(0))
            } else {
                Element::from(
                    button(load_icon_size("x", 14.0))
                        .on_press(Message::SearchQueryChanged(String::new()))
                        .style(theme::ghost_button())
                        .padding(Padding::new(6.0)),
                )
            },
            // Case Match Toggle Button ("Aa")
            button(text("Aa").size(theme::fs(12.0)).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }))
            .on_press(Message::ToggleCaseSensitive(!app.settings.case_sensitive))
            .style(move |t, s| theme::nav_button(app.settings.case_sensitive)(t, s))
            .padding(Padding::from([5, 8])),
            // Whole Word Toggle Button ("W")
            button(text("W").size(theme::fs(12.0)).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }))
            .on_press(Message::ToggleWholeWord(!app.settings.whole_word))
            .style(move |t, s| theme::nav_button(app.settings.whole_word)(t, s))
            .padding(Padding::from([5, 8])),
            // Search Mode Toggle Button
            button(
                row![
                    load_icon_size(
                        match app.search_mode {
                            SearchMode::FullText => "file-text",
                            SearchMode::Filename => "file",
                        },
                        12.0
                    ),
                    text(match app.search_mode {
                        SearchMode::FullText => "Text",
                        SearchMode::Filename => "File",
                    })
                    .size(theme::fs(11.0))
                    .font(Font {
                        weight: font::Weight::Bold,
                        ..Font::default()
                    })
                ]
                .spacing(4)
                .align_y(Alignment::Center)
            )
            .on_press(Message::SearchModeChanged(match app.search_mode {
                SearchMode::FullText => SearchMode::Filename,
                SearchMode::Filename => SearchMode::FullText,
            }))
            .style(move |t, s| {
                let active = matches!(app.search_mode, SearchMode::Filename);
                theme::nav_button(active)(t, s)
            })
            .padding(Padding::from([5, 10])),
            if app.is_searching {
                Element::from(
                    container(
                        text("Searching...")
                            .size(theme::fs(12.0))
                            .style(theme::dim_text_style()),
                    )
                    .padding(Padding::from([4, 12])),
                )
            } else {
                Element::from(
                    button(
                        row![
                            load_icon_size("arrow-right", 14.0),
                            text("Search").size(theme::fs(12.0)).font(Font {
                                weight: font::Weight::Bold,
                                ..Font::default()
                            })
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    )
                    .on_press(Message::SearchSubmitted)
                    .style(theme::search_button())
                    .padding(Padding::from([6, 14])),
                )
            }
        ]
        .spacing(6)
        .padding(Padding::from([4, 12]))
        .align_y(Alignment::Center),
    )
    .style(theme::input_container)
    .width(Length::FillPortion(3))
    .max_width(850.0);

    let menu_items = row![
        // Direct Client Theme Switcher (Dark 🌙 / Light ☀️)
        button(load_icon_size(
            if app.is_dark { "sun" } else { "moon" },
            18.0
        ))
        .on_press(Message::ToggleTheme)
        .style(theme::ghost_button())
        .padding(10.0),
        // Settings Button
        button(load_icon_size("settings", 18.0))
            .on_press(Message::TabChanged(Tab::Settings))
            .style(theme::ghost_button())
            .padding(10.0),
    ]
    .spacing(6);

    container(
        row![
            logo,
            Space::new().width(Length::Fill),
            search_bar,
            Space::new().width(Length::Fill),
            menu_items,
        ]
        .padding(Padding {
            top: 10.0,
            bottom: 10.0,
            left: 18.0,
            right: 18.0,
        })
        .align_y(Alignment::Center),
    )
    .style(theme::header_container)
    .width(Length::Fill)
    .into()
}

fn main_layout(app: &App) -> Element<'_, Message> {
    let sidebar = if app.sidebar_collapsed {
        collapsed_sidebar(app)
    } else {
        left_sidebar(app)
    };

    let body = row![
        sidebar,
        column![
            filter_chips(app),
            row![
                results_panel(app),
                container(right_panel(app))
                    .style(theme::sidebar_container)
                    .width(Length::FillPortion(3)),
            ]
            .height(Length::Fill)
        ]
        .width(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    // The context menu floats above the results list and swallows clicks so
    // clicking a menu entry does not also select the row underneath it.
    let body: Element<'_, Message> = match &app.context_menu {
        Some(menu_state) => {
            let menu = context_menu_overlay(menu_state);
            stack![body, menu].into()
        }
        None => body.into(),
    };

    // Clicking anywhere outside an open menu dismisses it.
    mouse_area(body).on_press(Message::HideContextMenu).into()
}

fn status_bar(app: &App) -> Element<'_, Message> {
    let mut status_row = row![
        container(
            row![
                load_icon_size("database", 12.0),
                text(format!("{} files indexed", app.files_indexed)).size(theme::fs(11.0)),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
        ),
        Space::new().width(Length::Fixed(16.0)),
        text(&app.index_size)
            .size(theme::fs(11.0))
            .style(theme::dim_text_style()),
        Space::new().width(Length::Fill),
    ];

    if !app.results.is_empty() {
        status_row = status_row.push(
            row![
                text(format!("{} results found", app.results.len()))
                    .size(theme::fs(11.0))
                    .style(theme::dim_text_style()),
                Space::new().width(Length::Fixed(12.0)),
                text("Export:")
                    .size(theme::fs(11.0))
                    .style(theme::dim_text_style()),
                button(text("CSV").size(theme::fs(10.0)).font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                }))
                .on_press(Message::ExportResults("csv".to_string()))
                .style(theme::secondary_button())
                .padding(Padding::from([2, 8])),
                button(text("JSON").size(theme::fs(10.0)).font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                }))
                .on_press(Message::ExportResults("json".to_string()))
                .style(theme::secondary_button())
                .padding(Padding::from([2, 8])),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
        status_row = status_row.push(Space::new().width(Length::Fixed(16.0)));
    }

    if let Some(p) = app.rebuild_progress {
        status_row = status_row
            .push(container(iced::widget::progress_bar(0.0..=1.0, p)).width(Length::Fixed(100.0)));
        status_row = status_row.push(Space::new().width(Length::Fixed(8.0)));

        if let Some(eta) = app.rebuild_eta {
            let eta_str = if eta >= 3600 {
                format!("ETA: {}h {}m", eta / 3600, (eta % 3600) / 60)
            } else if eta >= 60 {
                format!("ETA: {}m {}s", eta / 60, eta % 60)
            } else {
                format!("ETA: {eta}s")
            };
            status_row = status_row.push(text(eta_str).size(theme::fs(11.0)));
            status_row = status_row.push(Space::new().width(Length::Fixed(8.0)));
        }
    }

    if let Some(status) = &app.rebuild_status {
        status_row = status_row.push(text(status).size(theme::fs(11.0)));
    }

    container(status_row.padding(Padding {
        top: 6.0,
        bottom: 6.0,
        left: 18.0,
        right: 18.0,
    }))
    .style(theme::top_bar_container)
    .width(Length::Fill)
    .into()
}
