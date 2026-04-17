use super::{App, Message, Tab, theme};
use crate::iced_ui::icons::{load_icon, load_icon_size};
use iced::widget::{Scrollable, Space, TextInput, button, checkbox, column, container, row, text};
use iced::{Alignment, Element, Font, Length, Padding, font};

pub fn settings_view(app: &App) -> Element<'_, Message> {
    let content = column![
        settings_tabs(app),
        Space::new().height(Length::Fixed(40.0)),
        row![
            load_icon_size("settings", 32.0),
            text("Application Settings").size(32).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }),
        ]
        .spacing(16)
        .align_y(Alignment::Center),
        Space::new().height(Length::Fixed(48.0)),
        container(settings_form(app))
            .width(Length::Fill)
            .max_width(800.0)
    ]
    .width(Length::Fill)
    .align_x(Alignment::Center);

    let scroll = Scrollable::new(content).direction(iced::widget::scrollable::Direction::Vertical(
        iced::widget::scrollable::Scrollbar::default(),
    ));

    container(scroll)
        .style(theme::main_content_container)
        .padding(Padding::new(40.0))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .into()
}

fn settings_tabs(_app: &App) -> Element<'_, Message> {
    row![
        button(text("Search View").size(14))
            .on_press(Message::TabChanged(Tab::Search))
            .padding(Padding::from([10, 20]))
            .style(theme::tab_button(false)),
        button(text("Settings").size(14))
            .on_press(Message::TabChanged(Tab::Settings))
            .padding(Padding::from([10, 20]))
            .style(theme::tab_button(true)),
    ]
    .spacing(8)
    .into()
}

fn settings_form(app: &App) -> Element<'_, Message> {
    column![
        section_header("search", "Search Configuration"),
        search_settings_fields(app),
        Space::new().height(Length::Fixed(48.0)),
        section_header("folder", "Index Directories"),
        index_directories_section(app),
        Space::new().height(Length::Fixed(48.0)),
        section_header("gear", "System & Preferences"),
        system_integration_section(app),
        Space::new().height(Length::Fixed(48.0)),
        section_header("star", "Appearance"),
        appearance_section(app),
        Space::new().height(Length::Fixed(48.0)),
        section_header("database", "Data Management"),
        data_management_section(app),
        Space::new().height(Length::Fixed(64.0)),
        container(
            button(text("Save All Changes").size(16).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }))
            .on_press(Message::SaveSettings)
            .padding(Padding::from([14, 32]))
            .style(theme::primary_button())
        )
        .width(Length::Fill)
        .align_x(Alignment::Center),
        Space::new().height(Length::Fixed(40.0)),
    ]
    .width(Length::Fill)
    .into()
}

fn section_header<'a>(icon: &'a str, title: &'a str) -> Element<'a, Message> {
    column![
        row![
            load_icon_size(icon, 18.0),
            text(title).size(20).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        Space::new().height(Length::Fixed(12.0)),
    ]
    .into()
}

fn search_settings_fields(app: &App) -> Element<'_, Message> {
    column![
        row![
            text("Maximum Search Results").size(14).width(Length::Fill),
            TextInput::new("100", &app.settings.max_results.to_string())
                .padding(Padding::new(12.0))
                .size(14)
                .width(Length::Fixed(140.0))
                .on_input(Message::MaxResultsChanged)
                .style(theme::search_input())
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        Space::new().height(Length::Fixed(20.0)),
        text("Exclude Patterns (comma separated)")
            .size(14)
            .style(theme::dim_text_style()),
        Space::new().height(Length::Fixed(8.0)),
        TextInput::new(
            "*.git, target, node_modules",
            &app.settings.exclude_patterns.join(", ")
        )
        .padding(Padding::new(14.0))
        .size(14)
        .on_input(Message::ExcludePatternsChanged)
        .style(theme::search_input()),
        Space::new().height(Length::Fixed(20.0)),
        text("Custom File Extensions")
            .size(14)
            .style(theme::dim_text_style()),
        Space::new().height(Length::Fixed(8.0)),
        TextInput::new("e.g. mp4, exe, custom", &app.settings.custom_extensions)
            .padding(Padding::new(14.0))
            .size(14)
            .on_input(Message::CustomExtensionsChanged)
            .style(theme::search_input()),
        Space::new().height(Length::Fixed(20.0)),
        row![
            text("Global Search Hotkey").size(14).width(Length::Fill),
            TextInput::new("Alt+Space", &app.settings.global_hotkey)
                .padding(Padding::new(12.0))
                .size(14)
                .width(Length::Fixed(220.0))
                .on_input(Message::GlobalHotkeyChanged)
                .style(theme::search_input())
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    ]
    .into()
}

fn index_directories_section(app: &App) -> Element<'_, Message> {
    let mut dirs_col = column![].spacing(12);

    if app.settings.index_dirs.is_empty() {
        dirs_col = dirs_col.push(
            container(
                text("No directories configured for indexing.")
                    .size(14)
                    .style(theme::dim_text_style()),
            )
            .padding(20.0)
            .style(theme::hit_highlight_container)
            .width(Length::Fill),
        );
    } else {
        for (i, dir) in app.settings.index_dirs.iter().enumerate() {
            dirs_col = dirs_col.push(
                container(
                    row![
                        load_icon_size("folder", 16.0),
                        text(dir).size(14).width(Length::Fill),
                        button(load_icon_size("trash", 16.0))
                            .on_press(Message::RemoveFolder(i))
                            .padding(Padding::new(8.0))
                            .style(theme::ghost_button())
                    ]
                    .spacing(16)
                    .align_y(Alignment::Center),
                )
                .style(theme::padded_card_container)
                .padding(Padding::new(12.0))
                .width(Length::Fill),
            );
        }
    }

    column![
        dirs_col,
        Space::new().height(Length::Fixed(12.0)),
        button(row![load_icon("plus"), text("Add Folder To Index")].spacing(8))
            .on_press(Message::AddFolder)
            .padding(Padding::from([10, 20]))
            .style(theme::secondary_button())
    ]
    .spacing(8)
    .into()
}

fn system_integration_section(app: &App) -> Element<'_, Message> {
    column![
        checkbox(app.settings.minimize_to_tray)
            .label("Minimize to system tray")
            .on_toggle(Message::ToggleMinimizeToTray)
            .size(20)
            .text_size(14),
        checkbox(app.settings.auto_start_on_boot)
            .label("Start automatically on boot")
            .on_toggle(Message::ToggleAutoStart)
            .size(20)
            .text_size(14),
        checkbox(app.settings.context_menu_enabled)
            .label("Add to context menu (right-click)")
            .on_toggle(Message::ToggleContextMenu)
            .size(20)
            .text_size(14),
    ]
    .spacing(16)
    .into()
}

fn appearance_section(app: &App) -> Element<'_, Message> {
    column![
        row![
            checkbox(app.is_dark)
                .label("Dark theme")
                .on_toggle(|_| Message::ToggleTheme)
                .size(22)
                .text_size(15),
        ]
        .spacing(12)
    ]
    .into()
}

fn data_management_section(_app: &App) -> Element<'_, Message> {
    column![
        text("Force Index Rebuild").size(15).font(Font { weight: font::Weight::Bold, ..Font::default() }),
        text("This will clear all indexed data and start a fresh scan of all folders. Use this if searches are yielding unexpected results.").size(13).style(theme::dim_text_style()),
        Space::new().height(Length::Fixed(12.0)),
        button(row![load_icon("database"), text("Rebuild Search Index")].spacing(8))
            .on_press(Message::RebuildIndex)
            .padding(Padding::from([10, 20]))
            .style(theme::secondary_button())
    ]
    .spacing(8)
    .into()
}
