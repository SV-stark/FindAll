use super::{theme, App, Message, Tab};
use iced::widget::{button, checkbox, column, container, row, text, Scrollable, Space, TextInput};
use iced::{Alignment, Element, Length, Padding};

pub fn settings_view(app: &App) -> Element<Message> {
    let title = text("Settings").size(28);
    let tabs = row![
        button(text("Search").size(16))
            .on_press(Message::TabChanged(Tab::Search))
            .padding(Padding::from([8.0, 16.0])),
        button(text("Settings").size(16))
            .on_press(Message::TabChanged(Tab::Settings))
            .padding(Padding::from([8.0, 16.0]))
    ]
    .spacing(12);

    let max_results_label = text("Max Results:").size(16);
    let max_results_val = app.settings.max_results.to_string();
    let max_results_input = TextInput::new("100", &max_results_val)
        .padding(Padding::from(10.0))
        .size(16)
        .width(Length::Fixed(150.0))
        .on_input(Message::MaxResultsChanged);

    let exclude_label = text("Exclude Patterns (comma separated):").size(16);
    let exclude_val = app.settings.exclude_patterns.join(", ");
    let exclude_input = TextInput::new("*.git, target, node_modules", &exclude_val)
        .padding(Padding::from(10.0))
        .size(16)
        .width(Length::Fill)
        .on_input(Message::ExcludePatternsChanged);

    let mut dirs_col = column![text("Index Directories").size(20)].spacing(10);
    for (i, dir) in app.settings.index_dirs.iter().enumerate() {
        let row_item = container(
            row![
                text(dir).size(15).width(Length::Fill),
                button("Remove")
                    .on_press(Message::RemoveFolder(i))
                    .padding(Padding::from(8.0))
            ]
            .spacing(16)
            .align_y(Alignment::Center),
        )
        .style(theme::padded_card_container)
        .padding(Padding::from(16.0))
        .width(Length::Fill);

        dirs_col = dirs_col.push(row_item);
    }

    let add_btn = button(text("Add Folder").size(16))
        .on_press(Message::AddFolder)
        .padding(Padding::from(12.0));
    let save_btn = button(text("Save Settings").size(16))
        .on_press(Message::SaveSettings)
        .padding(Padding::from(14.0));

    let sys_int_label = text("System Integration").size(20);
    let tray_toggle =
        checkbox(app.settings.minimize_to_tray).on_toggle(Message::ToggleMinimizeToTray);
    let autostart_toggle =
        checkbox(app.settings.auto_start_on_boot).on_toggle(Message::ToggleAutoStart);
    let context_toggle =
        checkbox(app.settings.context_menu_enabled).on_toggle(Message::ToggleContextMenu);

    let settings_form = column![
        max_results_label,
        max_results_input,
        Space::new().height(Length::Fixed(24.0)),
        exclude_label,
        exclude_input,
        Space::new().height(Length::Fixed(32.0)),
        dirs_col,
        Space::new().height(Length::Fixed(8.0)),
        add_btn,
        Space::new().height(Length::Fixed(24.0)),
        sys_int_label,
        tray_toggle,
        autostart_toggle,
        context_toggle,
        Space::new().height(Length::Fixed(40.0)),
        save_btn,
    ]
    .spacing(8)
    .width(Length::Fill);

    let center_content = container(settings_form)
        .width(Length::Fixed(800.0))
        .center_x(Length::Fill);

    let scroll = Scrollable::new(
        column![
            tabs,
            Space::new().height(Length::Fixed(24.0)),
            title,
            Space::new().height(Length::Fixed(32.0)),
            center_content
        ]
        .width(Length::Fill)
        .align_x(Alignment::Center),
    );

    container(scroll)
        .style(theme::main_content_container)
        .padding(Padding::from(40.0))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
