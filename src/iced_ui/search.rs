use super::{theme, App, Message, SearchMode, Tab};
use iced::widget::{
    button, checkbox, column, container, mouse_area, rich_text, row, scrollable, span, text, Space,
    TextInput,
};
use iced::{font, Alignment, Element, Font, Length, Padding};

// --- Icons from TTF Font ---
use crate::iced_ui::icons::load_icon;

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
        button(row![load_icon("folder"), text("Open")].spacing(4))
            .on_press(Message::NotImplemented("Open File/Folder".to_string()))
            .style(theme::top_menu_button()),
        button(row![load_icon("text"), text("OCR")].spacing(4))
            .on_press(Message::NotImplemented("OCR".to_string()))
            .style(theme::top_menu_button()),
        button(row![load_icon("search"), text("Advanced Search")].spacing(4))
            .on_press(Message::NotImplemented("Advanced Search".to_string()))
            .style(theme::top_menu_button()),
        button(row![load_icon("settings"), text("Settings")].spacing(4))
            .on_press(Message::TabChanged(Tab::Settings))
            .style(theme::top_menu_button()),
        button(row![load_icon("plus"), text("File Actions")].spacing(4))
            .on_press(Message::NotImplemented("File Actions".to_string()))
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
        .on_input(Message::SearchQueryChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(Padding::new(10.0))
        .style(theme::search_input())
        .width(Length::Fill);

    let search_button = button(row![load_icon("search"), text("Search")].spacing(8))
        .on_press(Message::SearchSubmitted)
        .style(theme::search_button())
        .padding(Padding {
            top: 10.0,
            bottom: 10.0,
            left: 20.0,
            right: 20.0,
        });

    let _mode_toggle = button(
        text(if app.search_mode == SearchMode::FullText {
            "Full Text"
        } else {
            "Filename"
        })
        .size(12),
    )
    .on_press(Message::ToggleSearchMode)
    .style(theme::secondary_button())
    .padding(Padding::new(8.0));

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
    row![left_sidebar(app), right_panel(app),]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn left_sidebar(app: &App) -> Element<'_, Message> {
    let filter_header = text("Search Filters").size(14).font(Font {
        weight: font::Weight::Bold,
        ..Font::default()
    });

    let extension_section = column![
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
                extension_checkbox("txe", app),
                // "etc..." etc
                text("etc...").size(11).style(theme::dim_text_style()),
            ]
            .spacing(12),
            Space::new().height(Length::Fixed(4.0)),
            row![
                checkbox(app.filter_extensions.len() == 5 && app.filter_extension.is_empty()) // Simplified "All" check
                    .label("All")
                    .on_toggle(|_| Message::NotImplemented("Select All Extensions".to_string()))
                    .size(14)
                    .text_size(11),
                checkbox(false)
                    .label("Word")
                    .on_toggle(|_| Message::NoOp)
                    .size(14)
                    .text_size(11),
            ]
            .spacing(12),
        ])
    ]
    .spacing(8);

    let size_section = column![
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
    .spacing(8);

    let match_options = column![
        text("Match Options").size(12).style(theme::dim_text_style()),
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
    .spacing(6);

    let clear_button = button(text("Clear Filters").size(12))
        .on_press(Message::ClearFilters)
        .style(theme::clear_filter_button())
        .width(Length::Fill)
        .padding(Padding::new(8.0));

    let filter_panel = column![
        filter_header,
        Space::new().height(Length::Fixed(4.0)),
        row![
            extension_section.width(Length::FillPortion(1)),
            column![size_section, match_options].width(Length::FillPortion(1)).spacing(12),
        ].spacing(16),
        Space::new().height(Length::Fixed(8.0)),
        clear_button,
    ]
    .spacing(12)
    .padding(Padding::new(12.0));

    let table_header = container(
        row![
            text("Name").width(Length::FillPortion(2)).size(12),
            text("Modified").width(Length::FillPortion(1)).size(12),
            text("Type").width(Length::FillPortion(1)).size(12),
            text("Size").width(Length::FillPortion(1)).size(12),
        ]
        .padding(Padding::new(8.0)),
    )
    .style(theme::table_header_container)
    .width(Length::Fill);

    let results = scrollable(column(
        app.results
            .iter()
            .enumerate()
            .map(|(i, res)| {
                let is_selected = app.selected_index == Some(i);

                let item_area = mouse_area(
                    container(
                        row![
                            row![load_icon("file"), text(&res.title).size(13)]
                                .spacing(8)
                                .width(Length::FillPortion(2)),
                            text(
                                res.modified
                                    .map(crate::iced_ui::format_date)
                                    .unwrap_or_else(|| "Unknown".to_string())
                            )
                            .size(12)
                            .style(theme::muted_text_style())
                            .width(Length::FillPortion(1)),
                            text(res.extension.as_deref().unwrap_or("File"))
                                .size(12)
                                .style(theme::muted_text_style())
                                .width(Length::FillPortion(1)),
                            text(
                                res.size
                                    .map(crate::iced_ui::format_size)
                                    .unwrap_or_else(|| "Unknown".to_string())
                            )
                            .size(12)
                            .style(theme::muted_text_style())
                            .width(Length::FillPortion(1)),
                        ]
                        .align_y(Alignment::Center),
                    )
                    .padding(Padding {
                        top: 6.0,
                        bottom: 6.0,
                        left: 8.0,
                        right: 8.0,
                    })
                    .style(if is_selected {
                        theme::result_card_selected
                    } else {
                        theme::result_card_normal
                    })
                    .width(Length::Fill),
                )
                .on_press(Message::ResultSelected(i))
                .on_right_press(Message::ShowContextMenu(i));

                let mut col = column![item_area];

                if app.context_menu_index == Some(i) {
                    let ctx_menu = container(
                        row![
                            button(text("Open File").size(11))
                                .on_press(Message::OpenFile(res.path.clone()))
                                .style(theme::secondary_button())
                                .padding(Padding::new(4.0)),
                            button(text("Open Location").size(11))
                                .on_press(Message::OpenFolder(res.path.clone()))
                                .style(theme::secondary_button())
                                .padding(Padding::new(4.0)),
                            button(text("Copy Path").size(11))
                                .on_press(Message::CopyPath(res.path.clone()))
                                .style(theme::secondary_button())
                                .padding(Padding::new(4.0)),
                            Space::new().width(Length::Fill),
                            button(text("Close").size(11))
                                .on_press(Message::CloseContextMenu)
                                .style(theme::ghost_button())
                                .padding(Padding::new(4.0)),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    )
                    .padding(Padding {
                        top: 6.0,
                        bottom: 6.0,
                        left: 32.0,
                        right: 8.0,
                    })
                    .style(theme::table_header_container)
                    .width(Length::Fill);

                    col = col.push(ctx_menu);
                }

                col.width(Length::Fill).into()
            })
            .collect::<Vec<Element<Message>>>(),
    ))
    .height(Length::Fill);

    container(column![filter_panel, table_header, results].width(Length::Fill))
        .style(theme::sidebar_container)
        .width(Length::FillPortion(2))
        .height(Length::Fill)
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
        let highlighted_text =
            highlight_text(&preview_result.content, &preview_result.matched_terms);

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

    let hits_content: Element<'_, Message> = if let Some(res) = result {
        if res.snippets.is_empty() || res.snippets.iter().all(|s| s.is_empty()) {
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
                        .map(|(i, s)| hit_row(i + 1, s))
                        .collect::<Vec<_>>(),
                )
                .spacing(8)
                .padding(8),
            )
            .height(Length::Fill)
            .into()
        }
    } else {
        container(text("Select a file to see context hits").style(theme::muted_text_style()))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    };

    let header_text = if let Some(res) = result {
        format!("Context Highlights for '{}' in {}", app.search_query, res.title)
    } else {
        "Search Hits".to_string()
    };

    container(column![
        container(
            row![
                text(header_text).size(13).font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                }),
                Space::new().width(Length::Fill),
                text(format!(
                    "{} total",
                    result.map(|r| r.snippets.len()).unwrap_or(0)
                ))
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

fn hit_row(_idx: usize, content: &str) -> Element<'_, Message> {
    container(
        row![
            text(format!("{}.", _idx))
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

fn extension_checkbox<'a>(ext: &str, app: &App) -> Element<'a, Message> {
    checkbox(app.filter_extensions.contains(ext))
        .label(ext)
        .on_toggle(move |_| Message::ToggleFilterExtension(ext.to_string()))
        .size(14)
        .text_size(11)
        .into()
}

fn size_unit_button<'a>(unit: &str, app: &App) -> Element<'a, Message> {
    let is_active = app.size_unit == unit;
    button(text(unit).size(10))
        .on_press(Message::SizeUnitChanged(unit.to_string()))
        .style(if is_active {
            theme::primary_button()
        } else {
            theme::secondary_button()
        })
        .padding(Padding::new(4.0))
        .into()
}
