//! The results list and its per-row rendering.
//!
//! The empty state (`no_results_view`) lives here too: deciding what the panel
//! shows is inseparable from building it.

use super::super::{App, FileItem, Message, theme};
use super::snippet::parse_snippet;
use super::welcome::welcome_hero_view;
use crate::iced_ui::icons::load_icon_size;
use iced::widget::{Space, button, column, container, mouse_area, row, scrollable, text};
use iced::{Alignment, Element, Font, Length, Padding, font};

pub(super) fn results_panel(app: &App) -> Element<'_, Message> {
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
    res: &'a FileItem,
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
