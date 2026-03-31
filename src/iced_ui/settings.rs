use super::{theme, App, Message, Tab};
use iced::widget::{button, checkbox, column, container, row, text, Scrollable, Space, TextInput};
use iced::{Alignment, Element, Length, Padding};

pub fn settings_view(app: &App) -> Element<'_, Message> {
    let scroll = Scrollable::new(
        column![
            settings_tabs(app),
            Space::new().height(Length::Fixed(24.0)),
            text("Settings").size(28),
            Space::new().height(Length::Fixed(32.0)),
            container(settings_form(app))
                .width(Length::Fixed(700.0))
                .align_x(Alignment::Center)
        ]
        .width(Length::Fill)
        .align_x(Alignment::Center),
    );

    container(scroll)
        .style(theme::main_content_container)
        .padding(Padding::new(32.0))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn settings_tabs(app: &App) -> Element<'_, Message> {
    let search_tab_style = theme::tab_button(matches!(app.active_tab, Tab::Search));
    let settings_tab_style = theme::tab_button(matches!(app.active_tab, Tab::Settings));

    row![
        button(text("Search").size(16))
            .on_press(Message::TabChanged(Tab::Search))
            .padding(Padding::new(12.0))
            .style(search_tab_style),
        button(text("Settings").size(16))
            .on_press(Message::TabChanged(Tab::Settings))
            .padding(Padding::new(12.0))
            .style(settings_tab_style),
    ]
    .spacing(8)
    .into()
}

fn settings_form(app: &App) -> Element<'_, Message> {
    column![
        text("Search").size(18),
        Space::new().height(Length::Fixed(8.0)),
        search_settings_fields(app),
        Space::new().height(Length::Fixed(24.0)),
        index_directories_section(app),
        Space::new().height(Length::Fixed(32.0)),
        system_integration_section(app),
        Space::new().height(Length::Fixed(32.0)),
        appearance_section(app),
        Space::new().height(Length::Fixed(32.0)),
        data_management_section(app),
        Space::new().height(Length::Fixed(40.0)),
        save_button(),
    ]
    .spacing(8)
    .width(Length::Fill)
    .into()
}

fn search_settings_fields(app: &App) -> Element<'_, Message> {
    column![
        row![
            text("Max Results").size(14),
            TextInput::new("100", &app.settings.max_results.to_string())
                .padding(Padding::new(10.0))
                .size(14)
                .width(Length::Fixed(120.0))
                .on_input(Message::MaxResultsChanged)
                .style(theme::small_input())
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        Space::new().height(Length::Fixed(16.0)),
        text("Exclude Patterns").size(14),
        Space::new().height(Length::Fixed(6.0)),
        TextInput::new(
            "*.git, target, node_modules",
            &app.settings.exclude_patterns.join(", ")
        )
        .padding(Padding::new(10.0))
        .size(14)
        .width(Length::Fill)
        .on_input(Message::ExcludePatternsChanged)
        .style(theme::search_input()),
        Space::new().height(Length::Fixed(16.0)),
        text("Custom Extensions").size(14),
        Space::new().height(Length::Fixed(6.0)),
        TextInput::new("e.g. mp4, exe, custom", &app.settings.custom_extensions)
            .padding(Padding::new(10.0))
            .size(14)
            .width(Length::Fill)
            .on_input(Message::CustomExtensionsChanged)
            .style(theme::search_input()),
        Space::new().height(Length::Fixed(16.0)),
        row![
            text("Global Hotkey").size(14),
            TextInput::new("Alt+Space / Control+Shift+F", &app.settings.global_hotkey)
                .padding(Padding::new(10.0))
                .size(14)
                .width(Length::Fixed(200.0))
                .on_input(Message::GlobalHotkeyChanged)
                .style(theme::small_input())
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    ]
    .into()
}

fn index_directories_section(app: &App) -> Element<'_, Message> {
    let mut dirs_col = column!().spacing(8);
    for (i, dir) in app.settings.index_dirs.iter().enumerate() {
        dirs_col = dirs_col.push(
            container(
                row![
                    text(dir).size(14).width(Length::Fill),
                    button("Remove")
                        .on_press(Message::RemoveFolder(i))
                        .padding(Padding::new(8.0))
                        .style(theme::secondary_button())
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .style(theme::padded_card_container)
            .padding(Padding::new(12.0))
            .width(Length::Fill),
        );
    }

    column![
        text("Index Directories").size(18),
        Space::new().height(Length::Fixed(12.0)),
        dirs_col,
        Space::new().height(Length::Fixed(4.0)),
        button(text("+ Add Folder").size(14))
            .on_press(Message::AddFolder)
            .padding(Padding::new(12.0))
            .style(theme::secondary_button())
    ]
    .spacing(8)
    .into()
}

fn system_integration_section(app: &App) -> Element<'_, Message> {
    column![
        text("System Integration").size(18),
        Space::new().height(Length::Fixed(12.0)),
        column![
            row![
                checkbox(app.settings.minimize_to_tray).on_toggle(Message::ToggleMinimizeToTray),
                text("Minimize to system tray").size(14),
            ]
            .spacing(8),
            row![
                checkbox(app.settings.auto_start_on_boot).on_toggle(Message::ToggleAutoStart),
                text("Start automatically on boot").size(14),
            ]
            .spacing(8),
            row![
                checkbox(app.settings.context_menu_enabled).on_toggle(Message::ToggleContextMenu),
                text("Add to context menu (right-click)").size(14),
            ]
            .spacing(8),
        ]
        .spacing(12)
    ]
    .into()
}

fn appearance_section(app: &App) -> Element<'_, Message> {
    column![
        text("Appearance").size(18),
        Space::new().height(Length::Fixed(12.0)),
        row![
            checkbox(app.is_dark).on_toggle(|_| Message::ToggleTheme),
            text("Dark Mode").size(14),
        ]
        .spacing(8)
    ]
    .into()
}

fn data_management_section(_app: &App) -> Element<'_, Message> {
    column![
        text("Data Management").size(18),
        Space::new().height(Length::Fixed(8.0)),
        text("Clear the current index and rescan all configured directories.").size(14),
        Space::new().height(Length::Fixed(12.0)),
        button(text("Force Rebuild Index").size(14))
            .on_press(Message::RebuildIndex)
            .padding(Padding::new(12.0))
            .style(theme::secondary_button())
    ]
    .spacing(4)
    .into()
}

fn save_button() -> Element<'static, Message> {
    button(text("Save Settings").size(15))
        .on_press(Message::SaveSettings)
        .padding(Padding::new(12.0))
        .style(theme::primary_button())
        .into()
}
