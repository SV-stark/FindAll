use iced::widget::{button, column, container, row, text, Space, Scrollable, TextInput};
use iced::{Element, Length, Alignment, Padding};
use iced::theme;
use super::{App, Message, SearchMode, Tab};

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
        .padding(Padding::from([12.0, 24.0]))
        .style(theme::Button::Primary);
        
    let search_row = row![input, search_btn]
        .spacing(12)
        .width(Length::Fill)
        .align_items(Alignment::Center);

    // Filters and Toolbar
    let filter_ext = TextInput::new("ext:pdf", &app.filter_extension)
        .on_input(Message::FilterExtensionChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(8.0)
        .size(14)
        .width(Length::Fixed(140.0));
    
    let filter_size = TextInput::new("size:>1MB", &app.filter_size)
        .on_input(Message::FilterSizeChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(8.0)
        .size(14)
        .width(Length::Fixed(140.0));
    
    let mode_btn = button(mode_text).on_press(Message::ToggleSearchMode).padding(8.0).style(theme::Button::Secondary);
    let theme_btn = button(if app.is_dark { "Light" } else { "Dark" }).on_press(Message::ToggleTheme).padding(8.0).style(theme::Button::Secondary);
    let rebuild_btn = button("Rebuild Index").on_press(Message::RebuildIndex).padding(8.0).style(theme::Button::Secondary);
    let settings_btn = button("Settings").on_press(Message::TabChanged(Tab::Settings)).padding(8.0).style(theme::Button::Secondary);

    let filter_row = row![
        mode_btn, filter_ext, filter_size, 
        Space::with_width(Length::Fill),
        text(format!("Files: {} | Index: {}", app.files_indexed, app.index_size)).size(13),
        Space::with_width(Length::Fixed(16.0)),
        theme_btn, rebuild_btn, settings_btn
    ].spacing(12).align_items(Alignment::Center).width(Length::Fill);
    
    // Top Section Container
    let top_section = container(column![search_row, filter_row].spacing(16))
        .padding(Padding::from([10.0, 0.0, 20.0, 0.0]))
        .width(Length::Fill);

    // Results Panel
    let results_panel: Element<Message> = if app.is_searching {
        container(text("Searching...").size(16))
            .width(Length::Fill).height(Length::Fill)
            .center_x(Length::Fill).center_y(Length::Fill).into()
    } else if app.results.is_empty() {
        container(text("No results").size(16).style(theme::Text::Color(iced::Color::from_rgb(0.5, 0.5, 0.5))))
            .width(Length::Fill).height(Length::Fill)
            .center_x(Length::Fill).center_y(Length::Fill).into()
    } else {
        let items: Vec<_> = app.results.iter().enumerate().map(|(i, item)| {
            let ext_str = item.extension.as_deref().unwrap_or("");
            
            let item_content = row![
                text(&item.title).size(15).width(Length::Fill),
                text(ext_str).size(13).style(theme::Text::Color(iced::Color::from_rgb(0.5, 0.5, 0.5))),
            ].spacing(10).align_items(Alignment::Center);
            
            let mut btn = button(item_content)
                .on_press(Message::ResultSelected(i))
                .width(Length::Fill)
                .padding(Padding::from([10.0, 16.0]));
            
            if Some(i) == app.selected_index {
                btn = btn.style(theme::Button::Primary);
            } else {
                btn = btn.style(theme::Button::Text);
            }
                
            btn.into()
        }).collect();
        let list = Scrollable::new(column(items).spacing(4)).height(Length::Fill);
        container(list)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::Container::Box)
            .into()
    };
    
    // Preview Panel
    let preview_panel: Element<Message> = if app.is_loading_preview {
        container(text("Loading preview...").size(14))
            .width(Length::Fill).height(Length::Fill)
            .center_x(Length::Fill).center_y(Length::Fill).into()
    } else if let Some(ref preview) = app.preview_content {
        let preview_text = text(preview).size(14);
        let scroll = Scrollable::new(
            container(preview_text).padding(Padding::new(20.0)).width(Length::Fill)
        ).height(Length::Fill);
        
        container(scroll)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::Container::Box)
            .into()
    } else {
        container(text("Select a file to preview").size(14).style(theme::Text::Color(iced::Color::from_rgb(0.5, 0.5, 0.5))))
            .width(Length::Fill).height(Length::Fill)
            .center_x(Length::Fill).center_y(Length::Fill)
            .style(theme::Container::Box)
            .into()
    };
    
    // Split pane: Left takes 2/5, Right takes 3/5
    let split_pane = row![
        container(results_panel).width(Length::FillPortion(2)).height(Length::Fill),
        container(preview_panel).width(Length::FillPortion(3)).height(Length::Fill)
    ].spacing(20).width(Length::Fill).height(Length::Fill);
    
    // Main layout
    container(
        column![
            top_section,
            split_pane
        ]
        .width(Length::Fill)
        .height(Length::Fill)
    )
    .padding(30)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
