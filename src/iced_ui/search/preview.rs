//! The right-hand preview pane: highlighted document rendering.
//!
//! `render_element` maps a parsed document element to a styled widget. It is
//! preview-specific presentation rather than result presentation, so it lives
//! here rather than in `results`.

use super::super::{App, Message, theme};
use super::snippet::parse_snippet;
use crate::iced_ui::icons::{load_icon, load_icon_size};
use crate::models::{DocumentElementHighlight, ElementType};
use iced::widget::{Space, button, column, container, rich_text, row, scrollable, span, text};
use iced::{Alignment, Element, Font, Length, Padding, font};

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

#[allow(clippy::too_many_lines)]
pub(super) fn right_panel(app: &App) -> Element<'_, Message> {
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
