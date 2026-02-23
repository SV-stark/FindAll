use super::{theme, App, Message, SearchMode, Tab};
use iced::widget::{button, column, container, row, text, Scrollable, Space, TextInput};
use iced::{Alignment, Color, Element, Length, Padding};
use iced_aw::Card;

pub fn search_view(app: &App) -> Element<Message> {
    let mode_text = match app.search_mode {
        SearchMode::FullText => "Full Text",
        SearchMode::Filename => "Filename Only",
    };

    // Modern Search Bar
    let input = TextInput::new("Search files...", &app.search_query)
        .on_input(Message::SearchQueryChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(Padding::new(12.0))
        .size(20);

    let search_btn = button(text("Search").size(16))
        .on_press(Message::SearchSubmitted)
        .padding(Padding::from([12.0, 24.0]));

    let search_row = row![input, search_btn]
        .spacing(12)
        .width(Length::Fill)
        .align_y(Alignment::Center);

    // Search error display
    let error_display: Element<Message> = if let Some(ref err) = app.search_error {
        container(text(err).size(14))
            .padding(Padding::from(8.0))
            .into()
    } else {
        Space::new().height(Length::Fixed(0.0)).into()
    };

    // Filters and Toolbar
    let filter_ext = TextInput::new("ext:pdf", &app.filter_extension)
        .on_input(Message::FilterExtensionChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(Padding::from(8.0))
        .size(14)
        .width(Length::Fixed(140.0));

    let filter_size = TextInput::new("size:>1MB", &app.filter_size)
        .on_input(Message::FilterSizeChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(Padding::from(8.0))
        .size(14)
        .width(Length::Fixed(140.0));

    let mode_btn = button(mode_text)
        .on_press(Message::ToggleSearchMode)
        .padding(Padding::from(8.0));
    let theme_btn = button(if app.is_dark { "Light" } else { "Dark" })
        .on_press(Message::ToggleTheme)
        .padding(Padding::from(8.0));
    let rebuild_display: Element<_> = if let Some(progress) = app.rebuild_progress {
        let status = app.rebuild_status.as_deref().unwrap_or("Rebuilding...");
        row![
            text(status).size(14),
            iced::widget::ProgressBar::new(0.0..=1.0, progress),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    } else {
        button("Rebuild Index")
            .on_press(Message::RebuildIndex)
            .padding(Padding::from(8.0))
            .into()
    };
    let settings_btn = button("Settings")
        .on_press(Message::TabChanged(Tab::Settings))
        .padding(Padding::from(8.0));

    let filter_row = row![
        mode_btn,
        filter_ext,
        filter_size,
        Space::new().width(Length::Fill),
        text(format!(
            "Files: {} | Index: {}",
            app.files_indexed, app.index_size
        ))
        .size(13),
        Space::new().width(Length::Fixed(16.0)),
        theme_btn,
        rebuild_display,
        settings_btn
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    // Top Section Container
    let top_section = container(
        column![search_row, filter_row]
            .spacing(16)
            .align_x(Alignment::Center),
    )
    .style(theme::top_bar_container)
    .padding(iced::Padding {
        top: 20.0,
        right: 30.0,
        bottom: 20.0,
        left: 30.0,
    })
    .width(Length::Fill);

    // Results Panel
    let results_panel: Element<Message> = if app.is_searching {
        container(text("Searching...").size(16))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if app.results.is_empty() {
        container(text("No results").size(16))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else {
        let items: Vec<_> = app
            .results
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let ext_str = item.extension.as_deref().unwrap_or("");

                // Extract directory from path
                let dir = std::path::Path::new(&item.path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                let ext_owned = ext_str.to_string();
                let header_row = row![
                    text(&item.title).size(15),
                    Space::new().width(Length::Fill),
                    text(ext_owned).size(13),
                ]
                .align_y(Alignment::Center);

                let body_text = text(dir.clone()).size(12);

                let card = Card::new(header_row, body_text)
                    .padding(Padding::from(10.0))
                    .width(Length::Fill);

                button(card)
                    .on_press(Message::ResultSelected(i))
                    .width(Length::Fill)
                    .padding(Padding::from(0))
                    .into()
            })
            .collect();
        let list = Scrollable::new(column(items).spacing(8)).height(Length::Fill);
        container(list)
            .padding(Padding::from(10.0))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    };

    // Preview Panel
    let preview_panel: Element<Message> = if app.is_loading_preview {
        container(text("Loading preview...").size(14))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if let Some(ref preview) = app.preview_content {
        let preview_text = text(preview).size(14);
        let scroll = Scrollable::new(
            container(preview_text)
                .padding(Padding::new(20.0))
                .width(Length::Fill),
        )
        .height(Length::Fill);

        container(scroll)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        container(text("Select a file to preview").size(14))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    };

    let preview_panel_container = container(preview_panel)
        .style(theme::sidebar_container)
        .width(Length::FillPortion(3))
        .height(Length::Fill);

    // Split pane: Left takes 2/5, Right takes 3/5
    let split_pane = row![
        container(results_panel)
            .width(Length::FillPortion(2))
            .height(Length::Fill),
        preview_panel_container
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    // Main layout
    container(
        column![top_section, split_pane]
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .style(theme::main_content_container)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
