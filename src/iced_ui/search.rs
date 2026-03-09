use super::{theme, App, Message, SearchMode, Tab};
use iced::widget::{button, column, container, row, scrollable, svg, text, Space, TextInput};
use iced::{font, Alignment, Element, Font, Length, Padding};

// --- SVG Icons (Simplified for Iced) ---
const SEARCH_ICON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>"#;
const FOLDER_ICON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>"#;
const FILE_ICON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/></svg>"#;
const OCR_ICON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M7 7h10v10H7z"/></svg>"#;
const SETTINGS_ICON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>"#;

fn load_icon(svg_data: &'static str) -> Element<'static, Message> {
    svg::Svg::new(svg::Handle::from_memory(svg_data.as_bytes()))
        .width(Length::Fixed(16.0))
        .height(Length::Fixed(16.0))
        .into()
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
    let logo = row![
        text("Flash Search").size(16),
        text(" - Anytxt Inspired")
            .size(12)
            .style(theme::dim_text_style()),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let menu_items = row![
        button(row![load_icon(FOLDER_ICON_SVG), text("Open")].spacing(4))
            .on_press(Message::NotImplemented("Open File/Folder".to_string()))
            .style(theme::top_menu_button()),
        button(row![load_icon(OCR_ICON_SVG), text("OCR")].spacing(4))
            .on_press(Message::NotImplemented("OCR".to_string()))
            .style(theme::top_menu_button()),
        button(row![load_icon(SEARCH_ICON_SVG), text("Advanced Search")].spacing(4))
            .on_press(Message::NotImplemented("Advanced Search".to_string()))
            .style(theme::top_menu_button()),
        button(row![load_icon(SETTINGS_ICON_SVG), text("Settings")].spacing(4))
            .on_press(Message::TabChanged(Tab::Settings))
            .style(theme::top_menu_button()),
        button(text("File Actions"))
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
    .style(theme::top_bar_container)
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

    let mode_toggle = button(
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
        row![input, mode_toggle]
            .spacing(8)
            .padding(Padding {
                top: 8.0,
                bottom: 8.0,
                left: 16.0,
                right: 16.0,
            })
            .align_y(Alignment::Center),
    )
    .style(theme::top_bar_container)
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
    let nav_tree = column![
        button(row![load_icon(FOLDER_ICON_SVG), text("All Folders").size(13)].spacing(8))
            .on_press(Message::NotImplemented("Filter: All Folders".to_string()))
            .width(Length::Fill)
            .style(theme::nav_button(true)),
        button(row![load_icon(FILE_ICON_SVG), text("File Types").size(13)].spacing(8))
            .on_press(Message::NotImplemented("Filter: File Types".to_string()))
            .width(Length::Fill)
            .style(theme::nav_button(false)),
        button(row![load_icon(SEARCH_ICON_SVG), text("All Files").size(13)].spacing(8))
            .on_press(Message::NotImplemented("Filter: All Files".to_string()))
            .width(Length::Fill)
            .style(theme::nav_button(false)),
    ]
    .spacing(4)
    .padding(Padding::new(8.0));

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
                button(
                    container(
                        row![
                            row![load_icon(FILE_ICON_SVG), text(&res.title).size(13)]
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
                        top: 4.0,
                        bottom: 4.0,
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
                .style(theme::ghost_button())
                .width(Length::Fill)
                .into()
            })
            .collect::<Vec<Element<Message>>>(),
    ))
    .height(Length::Fill);

    container(column![nav_tree, table_header, results].width(Length::Fill))
        .style(theme::sidebar_container)
        .width(Length::FillPortion(2))
        .height(Length::Fill)
        .into()
}

fn right_panel(app: &App) -> Element<'_, Message> {
    let preview_tabs = container(
        row![
            button(text("Preview").size(12)).style(theme::nav_button(true)),
            Space::new().width(Length::Fill),
            load_icon(OCR_ICON_SVG),
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
        let content = &preview_result.content;
        let terms = &preview_result.matched_terms;
        
        // Simple highlighting: wrap matched terms in bold
        // This is a basic implementation; for better performance, we might need a custom widget
        let highlighted_text = if terms.is_empty() {
            text(content).size(14)
        } else {
            // For now, just show the content without highlighting
            // Implementing proper highlighting requires more complex logic
            text(content).size(14)
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
                .spacing(4)
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

    container(column![
        container(
            row![
                text("Search Hits").size(14).font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                }),
                Space::new().width(Length::Fill),
                text(format!(
                    "{} total",
                    result.map(|r| r.snippets.len()).unwrap_or(0)
                ))
                .size(12)
                .style(theme::muted_text_style()),
            ]
            .align_y(Alignment::Center)
            .padding(Padding::new(4.0))
        )
        .style(theme::table_header_container)
        .width(Length::Fill),
        hits_content,
    ])
    .style(theme::sidebar_container)
    .width(Length::Fill)
    .height(Length::Fixed(200.0))
    .into()
}

fn hit_row(_idx: usize, content: &str) -> Element<'_, Message> {
    row![
        text(_idx.to_string())
            .size(11)
            .style(theme::dim_text_style()),
        container(text(content).size(12)).padding(Padding::new(4.0)),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn status_bar(app: &App) -> Element<'_, Message> {
    container(
        row![
            text(format!("{} files indexed", app.files_indexed)).size(11),
            Space::new().width(Length::Fixed(16.0)),
            text(&app.index_size).size(11),
            Space::new().width(Length::Fill),
            if let Some(status) = &app.rebuild_status {
                Element::from(text(status).size(11))
            } else {
                Element::from(Space::new().width(Length::Fixed(0.0)))
            },
        ]
        .padding(Padding {
            top: 4.0,
            bottom: 4.0,
            left: 16.0,
            right: 16.0,
        }),
    )
    .style(theme::top_bar_container)
    .width(Length::Fill)
    .into()
}
