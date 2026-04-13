use super::{App, DateFilter, Message, Tab, theme};
use iced::widget::{
    Space, TextInput, button, checkbox, column, container, mouse_area, rich_text, row, scrollable,
    span, text,
};
use iced::{Alignment, Element, Font, Length, Padding, font};

// --- Icons from TTF Font ---
use crate::iced_ui::icons::{load_icon, load_icon_size};

use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

fn get_syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn get_theme_set() -> &'static ThemeSet {
    static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

fn render_code_preview<'a>(
    content: &'a str,
    extension: &str,
    is_dark: bool,
) -> Element<'a, Message> {
    let ps = get_syntax_set();
    let ts = get_theme_set();

    let syntax = ps
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| ps.find_syntax_plain_text());

    let theme_name = if is_dark {
        "base16-ocean.dark"
    } else {
        "base16-ocean.light"
    };
    let theme = &ts.themes[theme_name];

    let mut h = HighlightLines::new(syntax, theme);
    let mut spans: Vec<iced::widget::text::Span<'a, Message>> = Vec::new();

    // Limit to first 1000 lines to prevent UI freezing on massive files
    for line in LinesWithEndings::from(content).take(1000) {
        if let Ok(ranges) = h.highlight_line(line, ps) {
            for (style, text) in ranges {
                spans.push(
                    span(text)
                        .size(13)
                        .font(Font {
                            family: font::Family::Monospace,
                            ..Font::default()
                        })
                        .color(iced::Color::from_rgb8(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        )),
                );
            }
        } else {
            spans.push(span(line).size(13).font(Font {
                family: font::Family::Monospace,
                ..Font::default()
            }));
        }
    }

    if spans.is_empty() {
        return text(content).size(13).into();
    }

    rich_text(spans).into()
}

pub fn search_view(app: &App) -> Element<'_, Message> {
    column![
        top_navigation(app),
        search_input_bar(app),
        main_layout(app),
        status_bar(app),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn top_navigation(_app: &App) -> Element<'_, Message> {
    let logo = row![text("Flash Search").size(16).font(Font {
        weight: font::Weight::Bold,
        ..Font::default()
    }),]
    .spacing(8)
    .align_y(Alignment::Center);

    let menu_items = row![
        button(row![load_icon("settings"), text("Settings")].spacing(4))
            .on_press(Message::TabChanged(Tab::Settings))
            .style(theme::top_menu_button()),
    ]
    .spacing(16);

    container(
        row![logo, Space::new().width(Length::Fill), menu_items,]
            .padding(Padding {
                top: 8.0,
                bottom: 8.0,
                left: 16.0,
                right: 16.0,
            })
            .align_y(Alignment::Center),
    )
    .style(theme::header_container)
    .width(Length::Fill)
    .into()
}

fn search_input_bar(app: &App) -> Element<'_, Message> {
    let input = TextInput::new("Enter search keywords...", &app.search_query)
        .id(crate::iced_ui::get_search_input_id())
        .on_input(Message::SearchQueryChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(Padding::new(10.0))
        .style(theme::search_input())
        .width(Length::Fill);

    let search_button = if app.is_searching {
        button(row![text("Searching...").size(12)].spacing(8))
            .style(theme::secondary_button())
            .padding(Padding {
                top: 10.0,
                bottom: 10.0,
                left: 20.0,
                right: 20.0,
            })
    } else {
        button(row![load_icon("search"), text("Search")].spacing(8))
            .on_press(Message::SearchSubmitted)
            .style(theme::search_button())
            .padding(Padding {
                top: 10.0,
                bottom: 10.0,
                left: 20.0,
                right: 20.0,
            })
    };

    container(
        row![input, search_button]
            .spacing(12)
            .padding(Padding {
                top: 12.0,
                bottom: 12.0,
                left: 16.0,
                right: 16.0,
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
        left_sidebar(app) // Now ONLY filters
    };

    row![
        sidebar,
        column![filter_chips(app), results_panel(app)].width(Length::Fill),
        container(right_panel(app)).width(Length::FillPortion(2)),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn filter_chips(app: &App) -> Element<'_, Message> {
    if app.filter_extensions.is_empty() {
        return Space::new().height(0).into();
    }

    let mut chips_row = row![].spacing(8).padding(Padding::new(8.0));

    for ext in &app.filter_extensions {
        let ext_clone = ext.clone();
        chips_row = chips_row.push(
            button(
                row![
                    text(ext).size(12),
                    load_icon_size("x", 12.0), // Assuming 'x' is close icon
                ]
                .spacing(4),
            )
            .style(theme::secondary_button())
            .padding(Padding::new(4.0))
            .on_press(Message::ToggleFilterExtension(ext_clone)),
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
            button(load_icon_size("filter", 16.0))
                .on_press(Message::ToggleSidebar)
                .style(theme::ghost_button())
                .padding(Padding::new(8.0)),
        ]
        .spacing(16)
        .padding(Padding::new(8.0))
        .align_x(Alignment::Center),
    )
    .style(theme::sidebar_container)
    .height(Length::Fill)
    .width(Length::Fixed(48.0))
    .into()
}

fn left_sidebar(app: &App) -> Element<'_, Message> {
    let filter_header = row![
        text("Search Filters").size(14).font(Font {
            weight: font::Weight::Bold,
            ..Font::default()
        }),
        Space::new().width(Length::Fill),
        button(load_icon_size("chevron-left", 16.0))
            .on_press(Message::ToggleSidebar)
            .style(theme::ghost_button())
            .padding(Padding::new(4.0))
    ]
    .align_y(Alignment::Center);

    let filter_content = row![
        extension_filter_section(app).width(Length::FillPortion(1)),
        column![
            size_filter_section(app),
            date_filter_section(app),
            match_options_section(app)
        ]
        .width(Length::FillPortion(1))
        .spacing(12),
    ]
    .spacing(16);

    let clear_button = button(text("Clear Filters").size(12))
        .on_press(Message::ClearFilters)
        .style(theme::clear_filter_button())
        .width(Length::Fill)
        .padding(Padding::new(8.0));

    let filter_panel = column![
        filter_header,
        Space::new().height(Length::Fixed(4.0)),
        filter_content,
        Space::new().height(Length::Fixed(8.0)),
        clear_button,
    ]
    .spacing(12)
    .padding(Padding::new(12.0));

    container(filter_panel)
        .style(theme::sidebar_container)
        .width(Length::Fixed(260.0))
        .height(Length::Fill)
        .into()
}

fn extension_filter_section(app: &App) -> iced::widget::Column<'_, Message> {
    column![
        text("File Extension")
            .size(12)
            .style(theme::dim_text_style()),
        container(column![
            row![
                extension_checkbox("pdf", app),
                extension_checkbox("md", app),
                extension_checkbox("txt", app),
                extension_checkbox("py", app),
            ]
            .spacing(12),
            row![
                extension_checkbox("json", app),
                extension_checkbox("txt", app),
                text("etc...").size(11).style(theme::dim_text_style()),
            ]
            .spacing(12),
        ])
    ]
    .spacing(8)
}

fn size_filter_section(app: &App) -> iced::widget::Column<'_, Message> {
    column![
        text("Size Range").size(12).style(theme::dim_text_style()),
        row![
            TextInput::new("min", &app.min_size)
                .on_input(Message::MinSizeChanged)
                .padding(Padding::new(6.0))
                .size(12)
                .width(Length::Fixed(60.0)),
            text("-").size(12),
            TextInput::new("max", &app.max_size)
                .on_input(Message::MaxSizeChanged)
                .padding(Padding::new(6.0))
                .size(12)
                .width(Length::Fixed(60.0)),
            row![
                size_unit_button("KB", app),
                size_unit_button("MB", app),
                size_unit_button("GB", app),
            ]
            .spacing(2)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .spacing(8)
}

fn date_filter_section(app: &App) -> iced::widget::Column<'_, Message> {
    column![
        text("Date Modified")
            .size(12)
            .style(theme::dim_text_style()),
        row![
            date_filter_button("Anytime", DateFilter::Anytime, app),
            date_filter_button("Today", DateFilter::Today, app),
            date_filter_button("7 Days", DateFilter::Last7Days, app),
            date_filter_button("30 Days", DateFilter::Last30Days, app),
        ]
        .spacing(4)
    ]
    .spacing(8)
}

fn match_options_section(app: &App) -> iced::widget::Column<'_, Message> {
    column![
        text("Match Options")
            .size(12)
            .style(theme::dim_text_style()),
        checkbox(app.settings.case_sensitive)
            .label("Match Case")
            .on_toggle(Message::ToggleCaseSensitive)
            .size(14)
            .text_size(11),
        checkbox(app.settings.whole_word)
            .label("Whole Word")
            .on_toggle(Message::ToggleWholeWord)
            .size(14)
            .text_size(11),
    ]
    .spacing(6)
}

fn results_panel(app: &App) -> Element<'_, Message> {
    if app.results.is_empty() {
        let msg = if app.search_query.is_empty() {
            "Enter search keywords to begin..."
        } else {
            "No files found matching criteria."
        };
        return container(text(msg).style(theme::dim_text_style()))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
    }

    let results = scrollable(column(
        app.results
            .iter()
            .enumerate()
            .map(|(i, res)| result_item_view(app, i, res))
            .collect::<Vec<Element<Message>>>(),
    ))
    .height(Length::Fill);

    container(results)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn result_item_view<'a>(app: &App, i: usize, res: &'a super::FileItem) -> Element<'a, Message> {
    let is_selected = app.selected_index == Some(i);
    let is_hovered = app.hovered_item_index == Some(i);

    let mut actions_row = row![].spacing(6);
    if is_hovered {
        actions_row = actions_row.push(row![
            button(load_icon_size("external-link", 14.0))
                .on_press(Message::OpenFile(res.path.clone()))
                .style(theme::ghost_button())
                .padding(Padding::new(4.0)),
            button(load_icon_size("folder-open", 14.0))
                .on_press(Message::OpenFolder(res.path.clone()))
                .style(theme::ghost_button())
                .padding(Padding::new(4.0)),
            button(load_icon_size("copy", 14.0))
                .on_press(Message::CopyPath(res.path.clone()))
                .style(theme::ghost_button())
                .padding(Padding::new(4.0)),
        ]);
    }

    let card_content = column![
        row![
            load_icon("file"),
            text(&*res.title).size(14).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }),
            Space::new().width(Length::Fill),
            actions_row,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        text(&res.path).size(11).style(theme::dim_text_style()),
        row![
            text(res.extension.as_deref().unwrap_or("File"))
                .size(11)
                .style(theme::muted_text_style()),
            text("|").size(11).style(theme::dim_text_style()),
            text(
                res.size
                    .map_or_else(|| "Unknown size".to_string(), crate::iced_ui::format_size)
            )
            .size(11)
            .style(theme::muted_text_style()),
            text("|").size(11).style(theme::dim_text_style()),
            text(
                res.modified
                    .map_or_else(|| "Unknown date".to_string(), crate::iced_ui::format_date)
            )
            .size(11)
            .style(theme::muted_text_style()),
        ]
        .spacing(8),
        res.snippets.first().map_or_else(
            || Element::from(container(Space::new().height(0))),
            |snippet| {
                Element::from(
                    container(parse_snippet(snippet))
                        .padding(Padding::new(4.0))
                        .style(theme::hit_highlight_container),
                )
            },
        ),
    ]
    .spacing(4);

    let mut item_area = container(card_content)
        .padding(Padding::new(12.0))
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
            top: 4.0,
            bottom: 4.0,
            left: 8.0,
            right: 8.0,
        })
        .into()
}

fn highlight_text<'a>(content: &'a str, terms: &[String]) -> Element<'a, Message> {
    if terms.is_empty() || content.is_empty() {
        return text(content).size(14).into();
    }

    let lower_content = content.to_lowercase();
    let mut matches = Vec::new();

    for term in terms {
        if term.is_empty() {
            continue;
        }
        let lower_term = term.to_lowercase();
        let mut start = 0;
        while let Some(idx) = lower_content[start..].find(&lower_term) {
            let abs_idx = start + idx;
            matches.push((abs_idx, abs_idx + term.len()));
            start = abs_idx + term.len();
        }
    }

    if matches.is_empty() {
        return text(content).size(14).into();
    }

    matches.sort_by_key(|m| m.0);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for m in matches {
        if let Some(last) = merged.last_mut() {
            if m.0 <= last.1 {
                last.1 = last.1.max(m.1);
                continue;
            }
        }
        merged.push(m);
    }

    let mut spans: Vec<iced::widget::text::Span<'a, Message>> = Vec::new();
    let mut current = 0;

    for (start, end) in merged {
        if start > current {
            spans.push(span(&content[current..start]).size(14));
        }
        spans.push(
            span(&content[start..end])
                .size(14)
                .font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                })
                .color(iced::Color::from_rgb8(234, 179, 8)), // Highlight color (yellow)
        );
        current = end;
    }

    if current < content.len() {
        spans.push(span(&content[current..]).size(14));
    }

    rich_text(spans).into()
}

fn parse_snippet<'a>(content: &'a str) -> Element<'a, Message> {
    let mut spans: Vec<iced::widget::text::Span<'a, Message>> = Vec::new();
    let mut current_pos = 0;

    while let Some(start) = content[current_pos..].find("<b>") {
        let absolute_start = current_pos + start;

        if absolute_start > current_pos {
            spans.push(span(&content[current_pos..absolute_start]).size(12));
        }

        current_pos = absolute_start + 3; // length of <b>

        if let Some(end) = content[current_pos..].find("</b>") {
            let absolute_end = current_pos + end;
            spans.push(
                span(&content[current_pos..absolute_end])
                    .size(12)
                    .font(Font {
                        weight: font::Weight::Bold,
                        ..Font::default()
                    })
                    .color(iced::Color::from_rgb8(234, 179, 8)),
            );
            current_pos = absolute_end + 4; // length of </b>
        } else {
            spans.push(span(&content[current_pos..]).size(12).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }));
            current_pos = content.len();
            break;
        }
    }

    if current_pos < content.len() {
        spans.push(span(&content[current_pos..]).size(12));
    }

    if spans.is_empty() {
        return text(content).size(12).into();
    }

    rich_text(spans).into()
}

fn right_panel(app: &App) -> Element<'_, Message> {
    let preview_tabs = container(
        row![
            button(text("Preview").size(12)).style(theme::nav_button(true)),
            Space::new().width(Length::Fill),
            load_icon("text"),
        ]
        .padding(Padding::new(8.0))
        .align_y(Alignment::Center),
    )
    .style(theme::table_header_container)
    .width(Length::Fill);

    let preview_content: Element<'_, Message> = if app.is_loading_preview {
        container(text("Loading preview...").style(theme::dim_text_style()))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if let Some(preview_result) = &app.preview_result {
        let ext = app
            .selected_index
            .and_then(|i| app.results.get(i))
            .and_then(|r| r.extension.as_deref())
            .unwrap_or("txt");

        // Prefer syntax highlighting, but fall back to term highlighting if there are matches
        let highlighted_text = if preview_result.matched_terms.is_empty() {
            render_code_preview(&preview_result.content, ext, app.is_dark)
        } else {
            // For now, if there's a search term, we use the plain text highlighter
            // so we can see the yellow background highlights.
            highlight_text(&preview_result.content, &preview_result.matched_terms)
        };

        container(scrollable(
            container(highlighted_text).padding(Padding::new(20.0)),
        ))
        .into()
    } else {
        container(text("Select a file to preview").style(theme::dim_text_style()))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    };

    container(column![
        preview_tabs,
        container(preview_content).height(Length::Fill),
        hits_panel(app),
    ])
    .width(Length::FillPortion(3))
    .height(Length::Fill)
    .into()
}

fn hits_panel(app: &App) -> Element<'_, Message> {
    let result = app.selected_index.and_then(|i| app.results.get(i));

    let hits_content: Element<'_, Message> = result.map_or_else(
        || {
            container(text("Select a file to see context hits").style(theme::muted_text_style()))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        },
        |res| {
            if res.snippets.is_empty() || res.snippets.iter().all(std::string::String::is_empty) {
                container(text("No preview context available").style(theme::muted_text_style()))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into()
            } else {
                scrollable(
                    column(
                        res.snippets
                            .iter()
                            .enumerate()
                            .map(|(i, s)| hit_row(i + 1, s)),
                    )
                    .spacing(8)
                    .padding(8),
                )
                .height(Length::Fill)
                .into()
            }
        },
    );

    let header_text = result.map_or_else(
        || "Search Hits".to_string(),
        |res| {
            format!(
                "Context Highlights for '{}' in {}",
                app.search_query, res.title
            )
        },
    );

    container(column![
        container(
            row![
                text(header_text).size(13).font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                }),
                Space::new().width(Length::Fill),
                text(format!("{} total", result.map_or(0, |r| r.snippets.len())))
                    .size(11)
                    .style(theme::muted_text_style()),
            ]
            .align_y(Alignment::Center)
            .padding(Padding::new(8.0))
        )
        .style(theme::header_container)
        .width(Length::Fill),
        hits_content,
    ])
    .style(theme::sidebar_container)
    .width(Length::Fill)
    .height(Length::Fixed(220.0))
    .into()
}

fn hit_row(idx: usize, content: &str) -> Element<'_, Message> {
    container(
        row![
            text(format!("{idx}."))
                .size(12)
                .font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                })
                .style(theme::dim_text_style()),
            container(parse_snippet(content)).width(Length::Fill),
        ]
        .spacing(12)
        .align_y(Alignment::Start),
    )
    .padding(Padding::new(8.0))
    .style(theme::hit_highlight_container)
    .width(Length::Fill)
    .into()
}

fn status_bar(app: &App) -> Element<'_, Message> {
    let mut status_row = row![
        text(format!("{} files indexed", app.files_indexed)).size(11),
        Space::new().width(Length::Fixed(16.0)),
        text(&app.index_size).size(11),
        Space::new().width(Length::Fill),
    ];

    if let Some(p) = app.rebuild_progress {
        status_row = status_row
            .push(container(iced::widget::progress_bar(0.0..=1.0, p)).width(Length::Fixed(100.0)));
        status_row = status_row.push(Space::new().width(Length::Fixed(8.0)));
    }

    if let Some(status) = &app.rebuild_status {
        status_row = status_row.push(text(status).size(11));
    }

    container(status_row.padding(Padding {
        top: 4.0,
        bottom: 4.0,
        left: 16.0,
        right: 16.0,
    }))
    .style(theme::top_bar_container)
    .width(Length::Fill)
    .into()
}

fn extension_checkbox<'a>(ext: &'a str, app: &App) -> Element<'a, Message> {
    checkbox(app.filter_extensions.contains(ext))
        .label(ext)
        .on_toggle(move |_| Message::ToggleFilterExtension(ext.to_string()))
        .size(14)
        .text_size(11)
        .into()
}

fn size_unit_button<'a>(unit: &'a str, app: &App) -> Element<'a, Message> {
    let is_active = app.size_unit == unit;
    button(text(unit).size(10))
        .on_press(Message::SizeUnitChanged(unit.to_string()))
        .style(move |t: &iced::Theme, s| {
            if is_active {
                theme::primary_button()(t, s)
            } else {
                theme::secondary_button()(t, s)
            }
        })
        .padding(Padding::new(4.0))
        .into()
}

fn date_filter_button<'a>(label: &'a str, filter: DateFilter, app: &App) -> Element<'a, Message> {
    let is_active = app.date_filter == filter;
    button(text(label).size(10))
        .on_press(Message::DateFilterChanged(filter))
        .style(move |t: &iced::Theme, s| {
            if is_active {
                theme::primary_button()(t, s)
            } else {
                theme::secondary_button()(t, s)
            }
        })
        .padding(Padding::new(4.0))
        .into()
}
