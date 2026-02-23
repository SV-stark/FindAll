use super::{theme, App, Message, SearchMode, Tab};
use iced::widget::{
    button, column, container, progress_bar, row, svg, text, Scrollable, Space, TextInput,
};
use iced::{Alignment, Element, Length, Padding};

const SEARCH_ICON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>"#;

const FILE_ICON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5-2z"/><polyline points="14 2 14 8 20 8"/></svg>"#;

const SETTINGS_ICON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>"#;

const REFRESH_ICON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/></svg>"#;

const SUN_ICON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>"#;

const MOON_ICON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>"#;

fn load_icon(icon_name: &str) -> iced::widget::Svg {
    let svg_data = match icon_name {
        "search" => SEARCH_ICON_SVG,
        "file" => FILE_ICON_SVG,
        "settings" => SETTINGS_ICON_SVG,
        "refresh" => REFRESH_ICON_SVG,
        "sun" => SUN_ICON_SVG,
        "moon" => MOON_ICON_SVG,
        _ => SEARCH_ICON_SVG,
    };
    svg::Svg::new(svg::Handle::from_memory(svg_data.as_bytes().to_vec()))
}

pub fn search_view(app: &App) -> Element<Message> {
    let mode_text = match app.search_mode {
        SearchMode::FullText => "Full Text",
        SearchMode::Filename => "Filename",
    };

    let search_icon = container(load_icon("search")).padding(Padding::new(12.0));

    let input = TextInput::new("Search files...", &app.search_query)
        .on_input(Message::SearchQueryChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(Padding::new(12.0))
        .size(16)
        .style(theme::search_input())
        .width(Length::Fill);

    let search_btn = button(container(load_icon("search")).padding(Padding::new(8.0)))
        .on_press(Message::SearchSubmitted)
        .style(theme::primary_button())
        .padding(Padding::new(12.0));

    let search_bar = row![search_icon, input, search_btn]
        .spacing(0)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let search_bar_container = container(search_bar)
        .style(theme::input_container)
        .width(Length::Fill);

    let error_display: Element<Message> = if let Some(ref err) = app.search_error {
        container(text(err).size(13))
            .padding(Padding::new(8.0))
            .into()
    } else {
        Space::new().height(Length::Fixed(0.0)).into()
    };

    let filter_ext = TextInput::new("ext:pdf", &app.filter_extension)
        .on_input(Message::FilterExtensionChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(Padding::new(8.0))
        .size(13)
        .width(Length::Fixed(100.0))
        .style(theme::small_input());

    let filter_size = TextInput::new("size:>1MB", &app.filter_size)
        .on_input(Message::FilterSizeChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(Padding::new(8.0))
        .size(13)
        .width(Length::Fixed(100.0))
        .style(theme::small_input());

    let mode_btn = button(text(mode_text).size(13))
        .on_press(Message::ToggleSearchMode)
        .padding(Padding::new(12.0))
        .style(theme::secondary_button());

    let theme_btn = button(if app.is_dark {
        container(load_icon("sun"))
    } else {
        container(load_icon("moon"))
    })
    .on_press(Message::ToggleTheme)
    .padding(Padding::new(10.0))
    .style(theme::icon_button());

    let sep1 = container(
        Space::new()
            .width(Length::Fixed(1.0))
            .height(Length::Fixed(20.0)),
    );

    let rebuild_display: Element<_> = if let Some(progress) = app.rebuild_progress {
        let status = app.rebuild_status.as_deref().unwrap_or("Rebuilding...");
        column![text(status).size(12), progress_bar(0.0..=1.0, progress),]
            .spacing(4)
            .align_x(Alignment::Center)
            .width(Length::Fixed(120.0))
            .into()
    } else {
        button(row![container(load_icon("refresh")), text("Rebuild").size(13)].spacing(6))
            .on_press(Message::RebuildIndex)
            .padding(Padding::new(10.0))
            .style(theme::secondary_button())
            .into()
    };

    let settings_btn = button(container(load_icon("settings")).padding(Padding::new(4.0)))
        .on_press(Message::TabChanged(Tab::Settings))
        .padding(Padding::new(10.0))
        .style(theme::icon_button());

    let stats = text(format!("{} files | {}", app.files_indexed, app.index_size)).size(13);

    let filter_group = row![mode_btn, sep1, filter_ext, filter_size,]
        .spacing(8)
        .align_y(Alignment::Center);

    let toolbar_spacer = Space::new().width(Length::Fixed(16.0));

    let filter_row = row![
        filter_group,
        Space::new().width(Length::Fill),
        stats,
        toolbar_spacer,
        theme_btn,
        rebuild_display,
        settings_btn
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let top_section = container(
        column![search_bar_container, error_display, filter_row]
            .spacing(12)
            .align_x(Alignment::Center),
    )
    .style(theme::top_bar_container)
    .padding(Padding::new(16.0))
    .width(Length::Fill);

    let results_panel: Element<Message> = if app.is_searching {
        container(column![text("Searching...").size(14),].align_x(Alignment::Center))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if app.results.is_empty() && !app.search_query.is_empty() {
        container(
            column![
                text("No results found").size(16),
                Space::new().height(Length::Fixed(8.0)),
                text("Try adjusting your search terms").size(13),
            ]
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    } else if app.results.is_empty() {
        container(
            column![
                container(load_icon("search")),
                Space::new().height(Length::Fixed(12.0)),
                text("Start searching").size(16),
                text("Enter keywords to find files").size(13),
            ]
            .align_x(Alignment::Center),
        )
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
                let is_selected = app.selected_index == Some(i);

                let dir = std::path::Path::new(&item.path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                let file_icon = container(load_icon("file")).padding(Padding::new(8.0));

                let ext_badge =
                    container(text(ext_str.to_uppercase()).size(10)).padding(Padding::new(4.0));

                let score_text = format!("{:.1}", item.score * 100.0);
                let score_badge =
                    container(text(format!("{}%", score_text)).size(10)).padding(Padding::new(4.0));

                let header = row![
                    file_icon,
                    text(&item.title).size(15),
                    Space::new().width(Length::Fill),
                    score_badge,
                    ext_badge,
                ]
                .align_y(Alignment::Center);

                let body = text(dir).size(12);

                let card_content = column![header, body].spacing(4).align_x(Alignment::Start);

                let card = container(card_content)
                    .padding(Padding::new(12.0))
                    .width(Length::Fill);

                let card_container = if is_selected {
                    container(card).style(theme::result_card_hover)
                } else {
                    container(card).style(theme::result_card_normal)
                };

                let button_style = theme::result_button(is_selected);

                button(card_container)
                    .on_press(Message::ResultSelected(i))
                    .width(Length::Fill)
                    .padding(Padding::new(0.0))
                    .style(button_style)
                    .into()
            })
            .collect();
        let list = Scrollable::new(column(items).spacing(8)).height(Length::Fill);
        container(list)
            .padding(Padding::new(12.0))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    };

    let preview_panel: Element<Message> = if app.is_loading_preview {
        container(column![text("Loading preview...").size(14),].align_x(Alignment::Center))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if let Some(ref preview) = app.preview_content {
        let scroll = Scrollable::new(
            container(text(preview).size(14))
                .padding(Padding::new(20.0))
                .width(Length::Fill),
        )
        .height(Length::Fill);

        container(scroll)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        container(
            column![
                container(load_icon("file")),
                Space::new().height(Length::Fixed(12.0)),
                text("Select a file to preview").size(14),
            ]
            .align_x(Alignment::Center),
        )
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

    let split_pane = row![
        container(results_panel)
            .width(Length::FillPortion(2))
            .height(Length::Fill),
        preview_panel_container
    ]
    .width(Length::Fill)
    .height(Length::Fill);

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
