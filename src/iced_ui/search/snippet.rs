//! Snippet rendering for a search hit.
//!
//! Its own module because it is the one piece of presentation shared by the
//! results list and the preview pane: both render the same `<b>`-marked
//! fragments, so a change to escaping or wrapping has to apply to both at once.

use super::super::{Message, theme};
use iced::widget::{rich_text, span, text};
use iced::{Element, Font, font};

pub(super) fn parse_snippet<'a>(content: &'a str) -> Element<'a, Message> {
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
