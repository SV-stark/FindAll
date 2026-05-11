use super::{App, DateFilter, Message, Tab, theme};
use iced::widget::{
    Space, TextInput, button, checkbox, column, container, mouse_area, rich_text, row, scrollable,
    span, text,
};
use iced::{Alignment, Element, Font, Length, Padding, font};

// --- Icons from TTF Font ---
use crate::iced_ui::icons::{load_icon, load_icon_size};

use iced::widget::text::Highlighter as _;
use iced_highlighter::{Highlighter, Settings};
use std::ops::Range;

#[allow(dead_code)]
struct TermHighlighter {
    terms: Vec<String>,
}

impl TermHighlighter {
    #[allow(dead_code)]
    const fn new(terms: Vec<String>) -> Self {
        Self { terms }
    }

    #[allow(dead_code)]
    fn highlight_line(&self, line: &str) -> Vec<(Range<usize>, iced::Color)> {
        if self.terms.is_empty() {
            return Vec::new();
        }

        let mut matches = Vec::new();
        let lower_line = line.to_lowercase();

        for term in &self.terms {
            if term.is_empty() {
                continue;
            }
            let lower_term = term.to_lowercase();
            let mut start = 0;
            while let Some(idx) = lower_line[start..].find(&lower_term) {
                let abs_idx = start + idx;
                matches.push(abs_idx..abs_idx + term.len());
                start = abs_idx + term.len();
            }
        }

        if matches.is_empty() {
            return Vec::new();
        }
        matches.sort_by_key(|r| r.start);

        let mut merged: Vec<Range<usize>> = Vec::new();
        for m in matches {
            #[allow(clippy::collapsible_if)]
            if let Some(last) = merged.last_mut() {
                if m.start <= last.end {
                    last.end = last.end.max(m.end);
                    continue;
                }
            }
            merged.push(m);
        }

        merged.into_iter().map(|r| (r, theme::HIT_AMBER)).collect()
    }
}

fn sidebar_section<'a>(
    title: &'a str,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    column![
        text(title).size(14).font(Font {
            weight: font::Weight::Bold,
            ..Font::default()
        }),
        content.into()
    ]
    .spacing(12)
    .into()
}

fn highlight_to_spans<H>(
    content: &str,
    mut highlighter: impl FnMut(&str) -> Vec<(Range<usize>, H)>,
    map_highlight: impl Fn(H) -> Option<iced::Color>,
) -> Vec<iced::widget::text::Span<'_, Message>> {
    content
        .lines()
        .flat_map(|line| {
            let mut line_spans: Vec<_> = highlighter(line)
                .into_iter()
                .map(|(range, highlight)| {
                    let mut s = span(&line[range]).size(13).font(Font::MONOSPACE);

                    if let Some(color) = map_highlight(highlight) {
                        s = s.color(color);
                    }
                    s
                })
                .collect::<Vec<_>>();
            line_spans.push(span("\n"));
            line_spans
        })
        .collect()
}

fn render_code_preview<'a>(
    content: &'a str,
    extension: &str,
    _is_dark: bool,
) -> Element<'a, Message> {
    let mut highlighter = Highlighter::new(&Settings {
        #[allow(clippy::if_same_then_else)]
        theme: iced_highlighter::Theme::Base16Ocean,
        token: extension.to_string(),
    });

    let spans = highlight_to_spans(
        content,
        |line| highlighter.highlight_line(line).collect(),
        |h| h.color(),
    );

    if spans.is_empty() {
        return text(content).size(13).into();
    }

    rich_text(spans).into()
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
                        text("⚠️ Metadata database was corrupted and has been reset. Full re-index recommended.")
                            .size(14)
                            .style(theme::danger_text_style()),
                        Space::new().width(Length::Fill),
                        button(text("Dismiss").size(12))
                            .on_press(Message::DismissError)
                            .padding(Padding::from([4, 8]))
                    ]
                    .align_y(Alignment::Center)
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
                    text(format!("Error: {err}"))
                        .size(14)
                        .style(theme::danger_text_style()),
                    Space::new().width(Length::Fill),
                    button(text("Dismiss").size(12))
                        .on_press(Message::DismissError)
                        .padding(Padding::from([4, 8]))
                ]
                .align_y(Alignment::Center),
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

fn top_navigation(app: &App) -> Element<'_, Message> {
    let logo = row![
        load_icon_size("search", 20.0),
        text("Flash Search").size(18).font(Font {
            weight: font::Weight::Bold,
            ..Font::default()
        }),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let search_bar = container(
        row![
            TextInput::new("Type to search everything...", &app.search_query)
                .id(crate::iced_ui::get_search_input_id())
                .on_input(Message::SearchQueryChanged)
                .on_submit(Message::SearchSubmitted)
                .padding(Padding::new(12.0))
                .size(16)
                .style(theme::search_input())
                .width(Length::Fill),
            if app.is_searching {
                Element::from(
                    container(text("Searching...").size(12).style(theme::dim_text_style()))
                        .padding(Padding::new(12.0)),
                )
            } else {
                Element::from(
                    button(load_icon("arrow-right"))
                        .on_press(Message::SearchSubmitted)
                        .style(theme::search_button())
                        .padding(Padding::new(10.0)),
                )
            }
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .style(theme::input_container)
    .width(Length::FillPortion(2))
    .max_width(800.0);

    let menu_items = row![
        button(load_icon_size("settings", 18.0))
            .on_press(Message::TabChanged(Tab::Settings))
            .style(theme::ghost_button())
            .padding(10.0),
    ]
    .spacing(8);

    container(
        row![
            logo,
            Space::new().width(Length::Fill),
            search_bar,
            Space::new().width(Length::Fill),
            menu_items,
        ]
        .padding(Padding {
            top: 12.0,
            bottom: 12.0,
            left: 20.0,
            right: 20.0,
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

    row![
        sidebar,
        column![
            filter_chips(app),
            row![
                results_panel(app),
                container(right_panel(app)).width(Length::FillPortion(3)),
            ]
            .height(Length::Fill)
        ]
        .width(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn filter_chips(app: &App) -> Element<'_, Message> {
    if app.filter_extensions.is_empty() {
        return Space::new().height(0).into();
    }

    let mut chips_row = row![].spacing(8).padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 16.0,
        right: 16.0,
    });

    for ext in &app.filter_extensions {
        let ext_clone = ext.clone();
        chips_row = chips_row.push(
            container(
                row![
                    text(ext).size(12),
                    mouse_area(load_icon_size("x", 12.0))
                        .on_press(Message::ToggleFilterExtension(ext_clone))
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([4, 8]))
            .style(theme::badge_container),
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
        text("Search Filters").size(16).font(Font {
            weight: font::Weight::Bold,
            ..Font::default()
        }),
        Space::new().width(Length::Fill),
        button(load_icon_size("chevron-left", 16.0))
            .on_press(Message::ToggleSidebar)
            .style(theme::ghost_button())
            .padding(Padding::new(6.0))
    ]
    .align_y(Alignment::Center);

    let filter_content = scrollable(
        column![
            extension_filter_section(app),
            size_filter_section(app),
            date_filter_section(app),
            match_options_section(app),
            Space::new().height(Length::Fill),
            button(text("Clear Filters").size(13))
                .on_press(Message::ClearFilters)
                .style(theme::secondary_button())
                .width(Length::Fill)
                .padding(Padding::new(10.0)),
        ]
        .spacing(24),
    )
    .height(Length::Fill);

    let filter_panel = column![
        filter_header,
        Space::new().height(Length::Fixed(8.0)),
        filter_content,
    ]
    .spacing(16)
    .padding(Padding::new(20.0));

    container(filter_panel)
        .style(theme::sidebar_container)
        .width(Length::Fixed(300.0))
        .height(Length::Fill)
        .into()
}

fn extension_filter_section(app: &App) -> Element<'_, Message> {
    sidebar_section(
        "File Extension",
        column![
            extension_checkbox("pdf", app),
            extension_checkbox("md", app),
            extension_checkbox("txt", app),
            extension_checkbox("py", app),
            extension_checkbox("json", app),
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
                    .padding(Padding::new(8.0))
                    .size(13)
                    .width(Length::Fill),
                text("-").size(14),
                TextInput::new("Max", &app.max_size)
                    .on_input(Message::MaxSizeChanged)
                    .padding(Padding::new(8.0))
                    .size(13)
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![
                size_unit_button("KB", app),
                size_unit_button("MB", app),
                size_unit_button("GB", app),
            ]
            .spacing(4)
        ]
        .spacing(12),
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
        .spacing(6),
    )
}

fn match_options_section(app: &App) -> iced::widget::Column<'_, Message> {
    column![
        text("Match Options").size(14).font(Font {
            weight: font::Weight::Bold,
            ..Font::default()
        }),
        checkbox(app.settings.case_sensitive)
            .label("Match Case")
            .on_toggle(Message::ToggleCaseSensitive)
            .size(18)
            .text_size(13),
        checkbox(app.settings.whole_word)
            .label("Whole Word")
            .on_toggle(Message::ToggleWholeWord)
            .size(18)
            .text_size(13),
    ]
    .spacing(8)
}

fn results_panel(app: &App) -> Element<'_, Message> {
    if app.results.is_empty() {
        let (icon, msg) = if app.search_query.is_empty() {
            ("search", "Type something to begin searching...")
        } else {
            ("warning", "No results found for your query.")
        };

        return container(
            column![
                load_icon_size(icon, 48.0),
                text(msg).size(18).style(theme::dim_text_style()),
            ]
            .spacing(16)
            .align_x(Alignment::Center),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::FillPortion(2))
        .into();
    }

    let results = scrollable(column(
        app.results
            .iter()
            .enumerate()
            .map(|(i, res)| result_item_view(app.selected_index, app.hovered_item_index, i, res))
            .collect::<Vec<Element<Message>>>(),
    ))
    .height(Length::Fill);

    container(results)
        .width(Length::FillPortion(2))
        .height(Length::Fill)
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

    let mut actions_row = row![].spacing(10);
    if is_hovered || is_selected {
        actions_row = actions_row.push(
            row![
                button(load_icon_size("external-link", 16.0))
                    .on_press(Message::OpenFile(res.path.clone()))
                    .style(theme::ghost_button())
                    .padding(Padding::new(6.0)),
                button(load_icon_size("folder-open", 16.0))
                    .on_press(Message::OpenFolder(res.path.clone()))
                    .style(theme::ghost_button())
                    .padding(Padding::new(6.0)),
                button(load_icon_size("copy", 16.0))
                    .on_press(Message::CopyPath(res.path.clone()))
                    .style(theme::ghost_button())
                    .padding(Padding::new(6.0)),
            ]
            .spacing(4),
        );
    }

    let card_content = column![
        row![
            load_icon_size(
                if res.extension.as_deref() == Some("pdf") {
                    "file-text"
                } else {
                    "file"
                },
                18.0
            ),
            text(&*res.title).size(15).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }),
            Space::new().width(Length::Fill),
            actions_row,
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        text(&res.path).size(12).style(theme::dim_text_style()),
        row![
            container(
                text(res.extension.as_deref().unwrap_or("FILE"))
                    .size(10)
                    .font(Font {
                        weight: font::Weight::Bold,
                        ..Font::default()
                    })
            )
            .padding(Padding::from([2, 6]))
            .style(theme::accent_badge_container),
            container(
                text(
                    res.size
                        .map_or_else(|| "Unknown".to_string(), crate::iced_ui::format_size)
                )
                .size(10)
            )
            .padding(Padding::from([2, 6]))
            .style(theme::badge_container),
            container(
                text(
                    res.modified
                        .map_or_else(|| "Unknown".to_string(), crate::iced_ui::format_date)
                )
                .size(10)
            )
            .padding(Padding::from([2, 6]))
            .style(theme::badge_container),
        ]
        .spacing(8),
        res.snippets.first().map_or_else(
            || Element::from(Space::new().height(0)),
            |snippet| {
                Element::from(
                    container(parse_snippet(snippet))
                        .padding(Padding::new(8.0))
                        .width(Length::Fill)
                        .style(theme::hit_highlight_container),
                )
            },
        ),
    ]
    .spacing(8);

    let mut item_area = container(card_content)
        .padding(Padding::new(16.0))
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
            top: 2.0,
            bottom: 2.0,
            left: 12.0,
            right: 12.0,
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
        if let Some(last) = merged.last_mut()
            && m.0 <= last.1
        {
            last.1 = last.1.max(m.1);
            continue;
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
                .color(iced::Color::from_rgb8(234, 179, 8)),
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
            spans.push(span(&content[current_pos..absolute_start]).size(13));
        }

        current_pos = absolute_start + 3;

        if let Some(end) = content[current_pos..].find("</b>") {
            let absolute_end = current_pos + end;
            spans.push(
                span(&content[current_pos..absolute_end])
                    .size(13)
                    .font(Font {
                        weight: font::Weight::Bold,
                        ..Font::default()
                    })
                    .color(iced::Color::from_rgb8(234, 179, 8)),
            );
            current_pos = absolute_end + 4;
        } else {
            spans.push(span(&content[current_pos..]).size(13).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }));
            current_pos = content.len();
            break;
        }
    }

    if current_pos < content.len() {
        spans.push(span(&content[current_pos..]).size(13));
    }

    if spans.is_empty() {
        return text(content).size(13).into();
    }

    rich_text(spans).into()
}

#[allow(clippy::too_many_lines)]
fn right_panel(app: &App) -> Element<'_, Message> {
    app.preview_result.as_ref().map_or_else(
        || {
            container(
                column![
                    load_icon_size("file-text", 48.0),
                    text(if app.is_loading_preview {
                        "Loading document contents..."
                    } else {
                        "Select a file to preview"
                    })
                    .size(18)
                    .style(theme::dim_text_style()),
                ]
                .spacing(16)
                .align_x(Alignment::Center),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        },
        |preview_result| {
            let _ext = app
                .selected_index
                .and_then(|i| app.results.get(i))
                .and_then(|r| r.extension.as_deref())
                .unwrap_or("txt");

            let spans = preview_result
                .cached_spans
                .iter()
                .map(|(text_part, color_opt)| {
                    let mut s: iced::widget::text::Span<'_, Message> =
                        span(text_part).size(13).font(Font::MONOSPACE);
                    if let Some([r, g, b, a]) = color_opt {
                        s = s.color(iced::Color::from_rgba(*r, *g, *b, *a));
                    }
                    s
                })
                .collect::<Vec<_>>();
            let content: Element<'_, Message> = rich_text(spans).into();

            let snippets: Element<'_, Message> = app
                .selected_index
                .and_then(|i| app.results.get(i))
                .map_or_else(
                    || column![].into(),
                    |res| {
                        if res.snippets.is_empty() {
                            column![].into()
                        } else {
                            column![
                                text("Context Highlights")
                                    .size(14)
                                    .font(Font {
                                        weight: font::Weight::Bold,
                                        ..Font::default()
                                    })
                                    .style(theme::dim_text_style()),
                                column(
                                    res.snippets
                                        .iter()
                                        .enumerate()
                                        .map(|(i, s)| hit_row(i + 1, s))
                                )
                                .spacing(8)
                            ]
                            .spacing(12)
                            .into()
                        }
                    },
                );

            container(scrollable(
                column![
                    container(
                        row![
                            load_icon("file-text"),
                            text(preview_result.content.len().to_string()).size(12)
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center)
                    )
                    .style(theme::badge_container)
                    .padding(8.0),
                    snippets,
                    Space::new().height(16.0),
                    text("Full File Preview")
                        .size(14)
                        .font(Font {
                            weight: font::Weight::Bold,
                            ..Font::default()
                        })
                        .style(theme::dim_text_style()),
                    container(content)
                        .padding(Padding::new(20.0))
                        .style(theme::main_content_container),
                ]
                .spacing(20)
                .padding(Padding::new(24.0)),
            ))
            .height(Length::Fill)
            .into()
        },
    )
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
    .padding(Padding::new(12.0))
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

        if let Some(eta) = app.rebuild_eta {
            let eta_str = if eta >= 3600 {
                format!("ETA: {}h {}m", eta / 3600, (eta % 3600) / 60)
            } else if eta >= 60 {
                format!("ETA: {}m {}s", eta / 60, eta % 60)
            } else {
                format!("ETA: {eta}s")
            };
            status_row = status_row.push(text(eta_str).size(11));
            status_row = status_row.push(Space::new().width(Length::Fixed(8.0)));
        }
    }

    if let Some(status) = &app.rebuild_status {
        status_row = status_row.push(text(status).size(11));
    }

    container(status_row.padding(Padding {
        top: 6.0,
        bottom: 6.0,
        left: 20.0,
        right: 20.0,
    }))
    .style(theme::top_bar_container)
    .width(Length::Fill)
    .into()
}

fn extension_checkbox<'a>(ext: &'a str, app: &App) -> Element<'a, Message> {
    checkbox(app.filter_extensions.contains(ext))
        .label(ext)
        .on_toggle(move |_| Message::ToggleFilterExtension(ext.to_string()))
        .size(18)
        .text_size(13)
        .into()
}

fn size_unit_button<'a>(unit: &'a str, app: &App) -> Element<'a, Message> {
    let is_active = app.size_unit == unit;
    button(text(unit).size(11).font(Font {
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
    button(text(label).size(12))
        .on_press(Message::DateFilterChanged(filter))
        .style(move |t: &iced::Theme, s| {
            if is_active {
                theme::nav_button(true)(t, s)
            } else {
                theme::ghost_button()(t, s)
            }
        })
        .width(Length::Fill)
        .padding(Padding::new(8.0))
        .into()
}
