use super::{App, DateFilter, Message, SearchMode, SortBy, Tab, theme};
use crate::models::{DocumentElementHighlight, ElementType};
use iced::widget::{
    Space, TextInput, button, checkbox, column, container, mouse_area, rich_text, row, scrollable,
    span, stack, text,
};
use iced::{Alignment, Element, Font, Length, Padding, font};

// --- Icons from TTF Font ---
use crate::iced_ui::icons::{load_icon, load_icon_size};

fn sidebar_section<'a>(
    title: &'a str,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    column![
        text(title)
            .size(theme::fs(12.0))
            .font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            })
            .style(theme::muted_text_style()),
        container(content.into())
            .padding(Padding::new(12.0))
            .style(theme::sidebar_panel_container)
            .width(Length::Fill)
    ]
    .spacing(8)
    .into()
}

fn render_element(element: &DocumentElementHighlight) -> Element<'_, Message> {
    let spans = element
        .spans
        .iter()
        .map(|(text_part, color_opt)| {
            let mut s: iced::widget::text::Span<'_, Message> =
                span(text_part).size(theme::fs(13.0));
            if element.element_type == ElementType::CodeBlock {
                s = s.font(Font::MONOSPACE);
            }
            if let Some([r, g, b, a]) = color_opt {
                s = s.color(iced::Color::from_rgba(*r, *g, *b, *a));
            }
            s
        })
        .collect::<Vec<iced::widget::text::Span<'_, Message>>>();

    let content = rich_text(spans);

    match element.element_type {
        ElementType::Title => container(content.size(theme::fs(22.0)).font(Font {
            weight: font::Weight::Bold,
            ..Font::default()
        }))
        .padding(Padding {
            bottom: 14.0,
            ..Padding::default()
        })
        .into(),
        ElementType::Heading => container(content.size(theme::fs(16.0)).font(Font {
            weight: font::Weight::Bold,
            ..Font::default()
        }))
        .padding(Padding {
            top: 10.0,
            bottom: 6.0,
            ..Padding::default()
        })
        .into(),
        ElementType::ListItem => row![text(" • ").size(theme::fs(13.0)), content]
            .spacing(8)
            .into(),
        ElementType::CodeBlock => container(content)
            .padding(12)
            .style(theme::code_block_container)
            .width(Length::Fill)
            .into(),
        ElementType::Table => container(content)
            .padding(10)
            .style(theme::badge_container)
            .width(Length::Fill)
            .into(),
        _ => content.into(),
    }
}

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

/// Right-click menu for a single result.
///
/// Positioned in the upper-left of the results pane rather than at the cursor:
/// Iced 0.14 does not expose pointer coordinates through `MouseArea`, and a menu
/// pinned to a known corner is predictable and cannot fall off-screen.
fn context_menu_overlay(state: &super::ContextMenuState) -> Element<'_, Message> {
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

fn filter_chips(app: &App) -> Element<'_, Message> {
    if app.filter_extensions.is_empty() {
        return Space::new().height(0).into();
    }

    let mut chips_row = row![
        load_icon_size("filter", 14.0),
        text("Active Filters:")
            .size(theme::fs(12.0))
            .style(theme::dim_text_style())
    ]
    .spacing(8)
    .padding(Padding {
        top: 6.0,
        bottom: 6.0,
        left: 16.0,
        right: 16.0,
    })
    .align_y(Alignment::Center);

    for ext in &app.filter_extensions {
        let ext_clone = ext.clone();
        chips_row = chips_row.push(
            container(
                row![
                    text(ext).size(theme::fs(11.0)).font(Font {
                        weight: font::Weight::Bold,
                        ..Font::default()
                    }),
                    mouse_area(load_icon_size("x", 12.0))
                        .on_press(Message::ToggleFilterExtension(ext_clone))
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([3, 8]))
            .style(|t| theme::file_badge_container(t, Some(ext))),
        );
    }

    container(chips_row)
        .width(Length::Fill)
        .style(theme::header_container)
        .into()
}

fn collapsed_sidebar(_app: &App) -> Element<'_, Message> {
    container(
        column![
            button(load_icon_size("filter", 18.0))
                .on_press(Message::ToggleSidebar)
                .style(theme::ghost_button())
                .padding(Padding::new(12.0)),
        ]
        .spacing(16)
        .padding(Padding::new(4.0))
        .align_x(Alignment::Center),
    )
    .style(theme::sidebar_container)
    .height(Length::Fill)
    .width(Length::Fixed(56.0))
    .into()
}

fn left_sidebar(app: &App) -> Element<'_, Message> {
    let filter_header = row![
        load_icon_size("filter", 16.0),
        text("Filter Options").size(theme::fs(15.0)).font(Font {
            weight: font::Weight::Bold,
            ..Font::default()
        }),
        Space::new().width(Length::Fill),
        button(load_icon_size("chevron-left", 16.0))
            .on_press(Message::ToggleSidebar)
            .style(theme::ghost_button())
            .padding(Padding::new(6.0))
    ]
    .align_y(Alignment::Center)
    .spacing(8);

    let filter_content = scrollable(
        column![
            category_filter_section(app),
            sort_order_section(app),
            extension_filter_section(app),
            size_filter_section(app),
            date_filter_section(app),
            match_options_section(app),
            Space::new().height(Length::Fill),
            button(
                row![
                    load_icon_size("x", 14.0),
                    text("Reset All Filters").size(theme::fs(12.0))
                ]
                .spacing(6)
                .align_y(Alignment::Center)
            )
            .on_press(Message::ClearFilters)
            .style(theme::secondary_button())
            .width(Length::Fill)
            .padding(Padding::new(8.0)),
        ]
        .spacing(20),
    )
    .height(Length::Fill);

    let filter_panel = column![
        filter_header,
        Space::new().height(Length::Fixed(4.0)),
        filter_content,
    ]
    .spacing(14)
    .padding(Padding::new(18.0));

    container(filter_panel)
        .style(theme::sidebar_container)
        .width(Length::Fixed(290.0))
        .height(Length::Fill)
        .into()
}

fn extension_filter_section(app: &App) -> Element<'_, Message> {
    sidebar_section(
        "File Extension",
        column![
            row![
                extension_checkbox("pdf", app),
                extension_checkbox("docx", app),
            ]
            .spacing(12),
            row![
                extension_checkbox("md", app),
                extension_checkbox("txt", app),
            ]
            .spacing(12),
            row![extension_checkbox("rs", app), extension_checkbox("py", app),].spacing(12),
            row![
                extension_checkbox("json", app),
                extension_checkbox("csv", app),
            ]
            .spacing(12),
            row![
                extension_checkbox("log", app),
                extension_checkbox("cpp", app),
            ]
            .spacing(12),
        ]
        .spacing(8),
    )
}

fn size_filter_section(app: &App) -> Element<'_, Message> {
    sidebar_section(
        "Size Range",
        column![
            row![
                TextInput::new("Min", &app.min_size)
                    .on_input(Message::MinSizeChanged)
                    .padding(Padding::new(7.0))
                    .size(theme::fs(12.0))
                    .style(theme::search_input())
                    .width(Length::Fill),
                text("-")
                    .size(theme::fs(14.0))
                    .style(theme::dim_text_style()),
                TextInput::new("Max", &app.max_size)
                    .on_input(Message::MaxSizeChanged)
                    .padding(Padding::new(7.0))
                    .size(theme::fs(12.0))
                    .style(theme::search_input())
                    .width(Length::Fill),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            row![
                size_unit_button("KB", app),
                size_unit_button("MB", app),
                size_unit_button("GB", app),
            ]
            .spacing(4)
        ]
        .spacing(10),
    )
}

fn date_filter_section(app: &App) -> Element<'_, Message> {
    sidebar_section(
        "Last Modified",
        column![
            date_filter_button("Anytime", DateFilter::Anytime, app),
            date_filter_button("Today", DateFilter::Today, app),
            date_filter_button("Past Week", DateFilter::Last7Days, app),
            date_filter_button("Past Month", DateFilter::Last30Days, app),
        ]
        .spacing(4),
    )
}

fn match_options_section(app: &App) -> iced::widget::Column<'_, Message> {
    column![
        text("Search Scope")
            .size(theme::fs(12.0))
            .font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            })
            .style(theme::muted_text_style()),
        container(
            row![
                search_mode_button("Full Text", SearchMode::FullText, app),
                search_mode_button("Filename", SearchMode::Filename, app),
            ]
            .spacing(4)
        )
        .padding(Padding::new(4.0))
        .style(theme::sidebar_panel_container)
        .width(Length::Fill),
        Space::new().height(Length::Fixed(6.0)),
        text("Match Flags")
            .size(theme::fs(12.0))
            .font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            })
            .style(theme::muted_text_style()),
        container(
            column![
                checkbox(app.settings.case_sensitive)
                    .label("Match Case")
                    .on_toggle(Message::ToggleCaseSensitive)
                    .size(theme::fs(16.0))
                    .text_size(12),
                checkbox(app.settings.whole_word)
                    .label("Whole Word")
                    .on_toggle(Message::ToggleWholeWord)
                    .size(theme::fs(16.0))
                    .text_size(12),
            ]
            .spacing(8)
        )
        .padding(Padding::new(10.0))
        .style(theme::sidebar_panel_container)
        .width(Length::Fill),
    ]
    .spacing(6)
}

fn search_mode_button<'a>(label: &'a str, mode: SearchMode, app: &App) -> Element<'a, Message> {
    let is_active = app.search_mode == mode;
    button(text(label).size(theme::fs(11.0)).font(Font {
        weight: font::Weight::Bold,
        ..Font::default()
    }))
    .on_press(Message::SearchModeChanged(mode))
    .style(move |t: &iced::Theme, s| {
        if is_active {
            theme::primary_button()(t, s)
        } else {
            theme::secondary_button()(t, s)
        }
    })
    .width(Length::Fill)
    .padding(Padding::from([5, 10]))
    .into()
}

fn results_panel(app: &App) -> Element<'_, Message> {
    if app.results.is_empty() {
        if app.search_query.is_empty() {
            return welcome_hero_view(app);
        }
        return no_results_view(app);
    }

    let max_display = app.settings.results_per_page.clamp(25, 200);
    let mut result_elements: Vec<Element<Message>> = app
        .results
        .iter()
        .take(max_display)
        .enumerate()
        .map(|(i, res)| result_item_view(app.selected_index, app.hovered_item_index, i, res))
        .collect();

    if app.results.len() > max_display {
        result_elements.push(
            container(
                text(format!(
                    "Showing top {} of {} matches. Refine query to narrow results.",
                    max_display,
                    app.results.len()
                ))
                .size(theme::fs(11.0))
                .style(theme::dim_text_style()),
            )
            .padding(12)
            .center_x(Length::Fill)
            .into(),
        );
    }

    let results = scrollable(column(result_elements)).height(Length::Fill);

    container(results)
        .width(Length::FillPortion(2))
        .height(Length::Fill)
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

fn welcome_hero_view(app: &App) -> Element<'_, Message> {
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

fn no_results_view(_app: &App) -> Element<'_, Message> {
    container(
        column![
            load_icon_size("warning", 40.0),
            text("No matching results found")
                .size(theme::fs(17.0))
                .font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                }),
            text("Try adjusting your query or expanding search filters")
                .size(theme::fs(13.0))
                .style(theme::dim_text_style()),
            Space::new().height(Length::Fixed(12.0)),
            container(
                column![
                    text("Troubleshooting Suggestions:")
                        .size(theme::fs(12.0))
                        .font(Font {
                            weight: font::Weight::Bold,
                            ..Font::default()
                        }),
                    text("• Check spelling or try simpler keywords")
                        .size(theme::fs(12.0))
                        .style(theme::muted_text_style()),
                    text("• Switch between Full Text and Filename search modes")
                        .size(theme::fs(12.0))
                        .style(theme::muted_text_style()),
                    text("• Clear active file extension filters in the left sidebar")
                        .size(theme::fs(12.0))
                        .style(theme::muted_text_style()),
                ]
                .spacing(6)
            )
            .padding(16)
            .style(theme::padded_card_container)
            .max_width(500.0)
        ]
        .spacing(12)
        .align_x(Alignment::Center),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .width(Length::FillPortion(2))
    .into()
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::elidable_lifetime_names)]
fn result_item_view<'a>(
    selected_index: Option<usize>,
    hovered_item_index: Option<usize>,
    i: usize,
    res: &'a super::FileItem,
) -> Element<'a, Message> {
    let is_selected = selected_index == Some(i);
    let is_hovered = hovered_item_index == Some(i);

    let mut actions_row = row![].spacing(8);
    if is_hovered || is_selected {
        actions_row = actions_row.push(
            row![
                button(
                    row![
                        load_icon_size("external-link", 13.0),
                        text("Open").size(theme::fs(11.0))
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center)
                )
                .on_press(Message::OpenFile(res.path.clone()))
                .style(theme::ghost_button())
                .padding(Padding::from([4, 8])),
                button(
                    row![
                        load_icon_size("folder-open", 13.0),
                        text("Folder").size(theme::fs(11.0))
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center)
                )
                .on_press(Message::OpenFolder(res.path.clone()))
                .style(theme::ghost_button())
                .padding(Padding::from([4, 8])),
                button(load_icon_size("copy", 14.0))
                    .on_press(Message::CopyPath(res.path.clone()))
                    .style(theme::ghost_button())
                    .padding(Padding::new(5.0)),
            ]
            .spacing(4),
        );
    }

    let ext_str = res.extension.as_deref().unwrap_or("FILE");
    let file_icon_name = match ext_str.to_lowercase().as_str() {
        "pdf" | "txt" | "md" | "doc" | "docx" => "file-text",
        "rs" | "py" | "js" | "ts" | "cpp" | "c" | "cs" | "java" | "go" | "html" | "css"
        | "json" | "toml" => "file-code",
        "png" | "jpg" | "jpeg" | "svg" | "gif" => "file-image",
        "mp4" | "mkv" | "avi" => "file-video",
        "mp3" | "wav" | "flac" => "file-audio",
        _ => "file",
    };

    let card_content = column![
        row![
            load_icon_size(file_icon_name, 18.0),
            text(&*res.title).size(theme::fs(14.0)).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }),
            Space::new().width(Length::Fill),
            actions_row,
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        text(&res.path)
            .size(theme::fs(12.0))
            .style(theme::dim_text_style()),
        row![
            container(
                text(ext_str.to_uppercase())
                    .size(theme::fs(10.0))
                    .font(Font {
                        weight: font::Weight::Bold,
                        ..Font::default()
                    })
            )
            .padding(Padding::from([2, 6]))
            .style(|t| theme::file_badge_container(t, res.extension.as_deref())),
            container(
                text(
                    res.size
                        .map_or_else(|| "Unknown size".to_string(), crate::iced_ui::format_size)
                )
                .size(theme::fs(10.0))
            )
            .padding(Padding::from([2, 6]))
            .style(theme::badge_container),
            container(
                text(
                    res.modified
                        .map_or_else(|| "Unknown date".to_string(), crate::iced_ui::format_date)
                )
                .size(theme::fs(10.0))
            )
            .padding(Padding::from([2, 6]))
            .style(theme::badge_container),
        ]
        .spacing(6),
        if res.snippets.is_empty() {
            Element::from(Space::new().height(0))
        } else {
            let mut snippet_col = column![].spacing(6);
            for snippet in res.snippets.iter().take(3) {
                snippet_col = snippet_col.push(
                    container(parse_snippet(snippet))
                        .padding(Padding::new(8.0))
                        .width(Length::Fill)
                        .style(theme::hit_highlight_container),
                );
            }
            Element::from(snippet_col)
        }
    ]
    .spacing(8);

    let card_body = if is_selected {
        let accent_strip = container(Space::new().width(Length::Fixed(4.0)).height(Length::Fill))
            .style(|_t| container::Style {
                background: Some(iced::Background::Color(theme::ACCENT_BLUE)),
                border: iced::Border {
                    color: iced::Color::TRANSPARENT,
                    width: 0.0,
                    radius: iced::border::Radius::from(2.0),
                },
                ..Default::default()
            });

        row![
            accent_strip,
            container(card_content)
                .padding(Padding {
                    left: 10.0,
                    ..Padding::default()
                })
                .width(Length::Fill)
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill)
    } else {
        row![container(card_content).width(Length::Fill)]
            .align_y(Alignment::Center)
            .width(Length::Fill)
    };

    let mut item_area = container(card_body)
        .padding(Padding::new(14.0))
        .style(if is_selected {
            theme::result_card_selected
        } else {
            theme::result_card_normal
        })
        .width(Length::Fill);

    if is_hovered && !is_selected {
        item_area = item_area.style(theme::result_card_hover);
    }

    let mouse_wrapper = mouse_area(item_area)
        .on_press(Message::ResultSelected(i))
        .on_right_press(Message::ShowContextMenu(i))
        .on_enter(Message::ItemHovered(Some(i)))
        .on_exit(Message::ItemHovered(None));

    container(mouse_wrapper)
        .padding(Padding {
            top: 3.0,
            bottom: 3.0,
            left: 10.0,
            right: 10.0,
        })
        .into()
}

fn parse_snippet<'a>(content: &'a str) -> Element<'a, Message> {
    let mut spans: Vec<iced::widget::text::Span<'a, Message>> = Vec::new();
    let mut current_pos = 0;

    while let Some(start) = content[current_pos..].find("<b>") {
        let absolute_start = current_pos + start;

        if absolute_start > current_pos {
            spans.push(span(&content[current_pos..absolute_start]).size(theme::fs(13.0)));
        }

        current_pos = absolute_start + 3;

        if let Some(end) = content[current_pos..].find("</b>") {
            let absolute_end = current_pos + end;
            spans.push(
                span(&content[current_pos..absolute_end])
                    .size(theme::fs(13.0))
                    .font(Font {
                        weight: font::Weight::Bold,
                        ..Font::default()
                    })
                    .color(theme::HIT_AMBER),
            );
            current_pos = absolute_end + 4;
        } else {
            spans.push(
                span(&content[current_pos..])
                    .size(theme::fs(13.0))
                    .font(Font {
                        weight: font::Weight::Bold,
                        ..Font::default()
                    }),
            );
            current_pos = content.len();
            break;
        }
    }

    if current_pos < content.len() {
        spans.push(span(&content[current_pos..]).size(theme::fs(13.0)));
    }

    if spans.is_empty() {
        return text(content).size(theme::fs(13.0)).into();
    }

    rich_text(spans).into()
}

#[allow(clippy::too_many_lines)]
fn right_panel(app: &App) -> Element<'_, Message> {
    app.preview_result.as_ref().map_or_else(
        || {
            container(
                column![
                    load_icon_size("file-text", 44.0),
                    text(if app.is_loading_preview {
                        "Loading document contents..."
                    } else {
                        "Select a search result to preview"
                    })
                    .size(theme::fs(16.0))
                    .font(Font {
                        weight: font::Weight::Bold,
                        ..Font::default()
                    }),
                    text("Snippets and document preview will appear here")
                        .size(theme::fs(12.0))
                        .style(theme::dim_text_style()),
                ]
                .spacing(12)
                .align_x(Alignment::Center),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        },
        |preview_result| {
            let res = app.selected_index.and_then(|i| app.results.get(i));

            let ext = res.and_then(|r| r.extension.as_deref()).unwrap_or("txt");
            let title = res.map_or("Document Preview", |r| &*r.title);

            let file_icon = match ext.to_lowercase().as_str() {
                "pdf" | "txt" | "md" | "doc" => "file-text",
                "rs" | "py" | "js" | "ts" | "cpp" | "c" | "json" => "file-code",
                "png" | "jpg" | "jpeg" | "svg" => "file-image",
                _ => "file",
            };

            let quick_actions: Element<'_, Message> = res.map_or_else(
                || row![].into(),
                |r| {
                    row![
                        button(
                            row![
                                load_icon_size("external-link", 13.0),
                                text("Open").size(theme::fs(11.0))
                            ]
                            .spacing(4)
                            .align_y(Alignment::Center)
                        )
                        .on_press(Message::OpenFile(r.path.clone()))
                        .style(theme::ghost_button())
                        .padding(Padding::from([4, 8])),
                        button(
                            row![
                                load_icon_size("folder-open", 13.0),
                                text("Folder").size(theme::fs(11.0))
                            ]
                            .spacing(4)
                            .align_y(Alignment::Center)
                        )
                        .on_press(Message::OpenFolder(r.path.clone()))
                        .style(theme::ghost_button())
                        .padding(Padding::from([4, 8])),
                        button(load_icon_size("copy", 14.0))
                            .on_press(Message::CopyPath(r.path.clone()))
                            .style(theme::ghost_button())
                            .padding(Padding::new(5.0)),
                    ]
                    .spacing(4)
                    .into()
                },
            );

            let header = container(
                row![
                    load_icon_size(file_icon, 20.0),
                    column![
                        text(title).size(theme::fs(14.0)).font(Font {
                            weight: font::Weight::Bold,
                            ..Font::default()
                        }),
                        text(res.map_or("", |r| &*r.path))
                            .size(theme::fs(11.0))
                            .style(theme::dim_text_style()),
                    ]
                    .spacing(2)
                    .width(Length::Fill),
                    quick_actions,
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding(Padding {
                top: 12.0,
                bottom: 12.0,
                left: 18.0,
                right: 18.0,
            })
            .style(theme::header_container)
            .width(Length::Fill);

            let content: Element<'_, Message> =
                column(preview_result.elements.iter().map(render_element))
                    .spacing(10)
                    .into();

            let snippets: Element<'_, Message> = res.map_or_else(
                || column![].into(),
                |r| {
                    if r.snippets.is_empty() {
                        column![].into()
                    } else {
                        column![
                            row![
                                load_icon_size("sparkles", 14.0),
                                text("Matching Snippets")
                                    .size(theme::fs(13.0))
                                    .font(Font {
                                        weight: font::Weight::Bold,
                                        ..Font::default()
                                    })
                                    .style(theme::muted_text_style()),
                            ]
                            .spacing(6)
                            .align_y(Alignment::Center),
                            column(
                                r.snippets
                                    .iter()
                                    .enumerate()
                                    .map(|(i, s)| hit_row(i + 1, s))
                            )
                            .spacing(8)
                        ]
                        .spacing(10)
                        .into()
                    }
                },
            );

            let body = scrollable(
                column![
                    container(
                        row![
                            load_icon("file-text"),
                            text(format!(
                                "{} structural elements parsed",
                                preview_result.elements.len()
                            ))
                            .size(theme::fs(11.0))
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center)
                    )
                    .style(theme::badge_container)
                    .padding(Padding {
                        top: 5.0,
                        bottom: 5.0,
                        left: 10.0,
                        right: 10.0,
                    }),
                    snippets,
                    Space::new().height(6.0),
                    text("Document Content")
                        .size(theme::fs(13.0))
                        .font(Font {
                            weight: font::Weight::Bold,
                            ..Font::default()
                        })
                        .style(theme::muted_text_style()),
                    container(content)
                        .padding(Padding::new(18.0))
                        .style(theme::main_content_container),
                ]
                .spacing(18)
                .padding(Padding::new(18.0)),
            )
            .height(Length::Fill);

            column![header, body]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        },
    )
}

fn hit_row(idx: usize, content: &str) -> Element<'_, Message> {
    container(
        row![
            text(format!("{idx}."))
                .size(theme::fs(12.0))
                .font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                })
                .style(theme::dim_text_style()),
            container(parse_snippet(content)).width(Length::Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Start),
    )
    .padding(Padding::new(10.0))
    .style(theme::hit_highlight_container)
    .width(Length::Fill)
    .into()
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

fn extension_checkbox<'a>(ext: &'a str, app: &App) -> Element<'a, Message> {
    checkbox(app.filter_extensions.contains(ext))
        .label(ext)
        .on_toggle(move |_| Message::ToggleFilterExtension(ext.to_string()))
        .size(theme::fs(16.0))
        .text_size(12)
        .into()
}

fn size_unit_button<'a>(unit: &'a str, app: &App) -> Element<'a, Message> {
    let is_active = app.size_unit == unit;
    button(text(unit).size(theme::fs(11.0)).font(Font {
        weight: font::Weight::Bold,
        ..Font::default()
    }))
    .on_press(Message::SizeUnitChanged(unit.to_string()))
    .style(move |t: &iced::Theme, s| {
        if is_active {
            theme::primary_button()(t, s)
        } else {
            theme::secondary_button()(t, s)
        }
    })
    .padding(Padding::from([4, 10]))
    .into()
}

fn date_filter_button<'a>(label: &'a str, filter: DateFilter, app: &App) -> Element<'a, Message> {
    let is_active = app.date_filter == filter;
    button(text(label).size(theme::fs(12.0)))
        .on_press(Message::DateFilterChanged(filter))
        .style(move |t: &iced::Theme, s| {
            if is_active {
                theme::nav_button(true)(t, s)
            } else {
                theme::ghost_button()(t, s)
            }
        })
        .width(Length::Fill)
        .padding(Padding::new(7.0))
        .into()
}

fn sort_order_section(app: &App) -> Element<'_, Message> {
    sidebar_section(
        "Sort Results By",
        column![
            sort_button("Relevance Score", SortBy::Relevance, app),
            sort_button("Date Modified", SortBy::DateModified, app),
            sort_button("File Size", SortBy::Size, app),
            sort_button("File Name", SortBy::Name, app),
        ]
        .spacing(4),
    )
}

fn sort_button<'a>(label: &'a str, sort: SortBy, app: &App) -> Element<'a, Message> {
    let is_active = app.sort_by == sort;
    button(text(label).size(theme::fs(12.0)))
        .on_press(Message::SortByChanged(sort))
        .style(move |t: &iced::Theme, s| {
            if is_active {
                theme::nav_button(true)(t, s)
            } else {
                theme::ghost_button()(t, s)
            }
        })
        .width(Length::Fill)
        .padding(Padding::new(7.0))
        .into()
}

fn category_filter_section(app: &App) -> Element<'_, Message> {
    sidebar_section(
        "Quick Categories",
        column![
            category_preset_button("📄 Documents", &["pdf", "docx", "md", "txt"], app),
            category_preset_button("💻 Source Code", &["rs", "py", "js", "ts", "cpp"], app),
            category_preset_button("📊 Data & Logs", &["json", "csv", "log", "xml"], app),
            category_preset_button("🖼️ Images", &["png", "jpg", "jpeg", "svg"], app),
        ]
        .spacing(4),
    )
}

fn category_preset_button<'a>(
    label: &'a str,
    exts: &'static [&'static str],
    app: &App,
) -> Element<'a, Message> {
    let is_active = exts.iter().all(|e| app.filter_extensions.contains(*e));
    let exts_vec: Vec<String> = exts.iter().map(|s| (*s).to_string()).collect();

    button(text(label).size(theme::fs(12.0)))
        .on_press(Message::ToggleCategory(exts_vec))
        .style(move |t: &iced::Theme, s| {
            if is_active {
                theme::nav_button(true)(t, s)
            } else {
                theme::ghost_button()(t, s)
            }
        })
        .width(Length::Fill)
        .padding(Padding::new(7.0))
        .into()
}
