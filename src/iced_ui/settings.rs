use iced::widget::{button, column, container, row, text, TextInput, Space};
use iced::{Element, Length, Padding, Alignment};
use iced::theme;
use super::{App, Message, Tab};

pub fn settings_view(app: &App) -> Element<Message> {
    let title = text("Settings").size(28);
    let tabs = row![
        button("Search").on_press(Message::TabChanged(Tab::Search)).padding(8.0).style(iced::theme::Button::Secondary),
        button("Settings").on_press(Message::TabChanged(Tab::Settings)).padding(8.0).style(iced::theme::Button::Primary)
    ].spacing(12);
    
    let max_results_label = text("Max Results:").size(16);
    let max_results_val = app.settings.max_results.to_string();
    let max_results_input = TextInput::new("100", &max_results_val)
        .padding(10.0).size(16).width(Length::Fixed(150.0))
        .on_input(Message::MaxResultsChanged);
    
    let exclude_label = text("Exclude Patterns (comma separated):").size(16);
    let exclude_val = app.settings.exclude_patterns.join(", ");
    let exclude_input = TextInput::new("*.git, target, node_modules", &exclude_val)
        .padding(10.0).size(16).width(Length::Fill)
        .on_input(Message::ExcludePatternsChanged);
    
    let mut dirs_col = column![text("Index Directories").size(20)].spacing(10);
    for (i, dir) in app.settings.index_dirs.iter().enumerate() {
        let row_item = container(
            row![
                text(dir).size(15).width(Length::Fill),
                button("Remove").on_press(Message::RemoveFolder(i)).padding(6.0).style(iced::theme::Button::Destructive)
            ].spacing(16).align_x(Alignment::Center)
        )
        .padding(12.0)
        .style(iced::theme::Container::Box)
        .width(Length::Fill);
        
        dirs_col = dirs_col.push(row_item);
    }
    
    let add_btn = button(text("Add Folder").size(16)).on_press(Message::AddFolder).padding(12.0).style(iced::theme::Button::Secondary);
    let save_btn = button(text("Save Settings").size(16)).on_press(Message::SaveSettings).padding(14.0).style(iced::theme::Button::Primary);
    
    let settings_form = column![
        max_results_label,
        max_results_input,
        Space::with_height(Length::Fixed(24.0)),
        exclude_label,
        exclude_input,
        Space::with_height(Length::Fixed(32.0)),
        dirs_col,
        Space::with_height(Length::Fixed(8.0)),
        add_btn,
        Space::with_height(Length::Fixed(40.0)),
        save_btn,
    ].spacing(8).width(Length::Fill);
    
    let center_content = container(settings_form)
        .width(Length::Fixed(800.0))
        .center_x(Length::Fill);

    container(
        column![
            tabs, 
            Space::with_height(Length::Fixed(16.0)),
            title, 
            Space::with_height(Length::Fixed(32.0)), 
            center_content
        ]
        .width(Length::Fill)
        .align_x(Alignment::Center)
    )
    .padding(40)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
