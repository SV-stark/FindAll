use super::{App, Message, Tab, theme};
use crate::iced_ui::icons::load_icon_size;
use iced::widget::{Scrollable, Space, TextInput, button, checkbox, column, container, row, text};
use iced::{Alignment, Element, Font, Length, Padding, font};

pub fn settings_view(app: &App) -> Element<'_, Message> {
    let content = column![
        settings_tabs(app),
        Space::new().height(Length::Fixed(28.0)),
        row![
            container(load_icon_size("settings", 24.0))
                .padding(10)
                .style(theme::accent_badge_container),
            column![
                text("Application Settings").size(24).font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                }),
                text("Configure search preferences, indexed locations, and desktop options")
                    .size(13)
                    .style(theme::dim_text_style()),
            ]
            .spacing(2),
        ]
        .spacing(14)
        .align_y(Alignment::Center),
        Space::new().height(Length::Fixed(32.0)),
        container(settings_form(app))
            .width(Length::Fill)
            .max_width(820.0)
    ]
    .width(Length::Fill)
    .align_x(Alignment::Center);

    let scroll = Scrollable::new(content).direction(iced::widget::scrollable::Direction::Vertical(
        iced::widget::scrollable::Scrollbar::default(),
    ));

    container(scroll)
        .style(theme::main_content_container)
        .padding(Padding::new(32.0))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .into()
}

fn settings_tabs(app: &App) -> Element<'_, Message> {
    row![
        button(
            row![load_icon_size("search", 14.0), text("Search View").size(13)]
                .spacing(8)
                .align_y(Alignment::Center)
        )
        .on_press(Message::TabChanged(Tab::Search))
        .padding(Padding::from([8, 16]))
        .style(theme::tab_button(false)),
        button(
            row![load_icon_size("settings", 14.0), text("Settings").size(13)]
                .spacing(8)
                .align_y(Alignment::Center)
        )
        .on_press(Message::TabChanged(Tab::Settings))
        .padding(Padding::from([8, 16]))
        .style(theme::tab_button(true)),
        Space::new().width(Length::Fill),
        // Direct theme switcher in settings header
        button(
            row![
                load_icon_size(if app.is_dark { "sun" } else { "moon" }, 14.0),
                text(if app.is_dark {
                    "Light Mode"
                } else {
                    "Dark Mode"
                })
                .size(12)
            ]
            .spacing(6)
            .align_y(Alignment::Center)
        )
        .on_press(Message::ToggleTheme)
        .padding(Padding::from([6, 12]))
        .style(theme::secondary_button()),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn settings_form(app: &App) -> Element<'_, Message> {
    column![
        section_header("sliders", "Search Configuration"),
        container(search_settings_fields(app))
            .padding(20)
            .style(theme::padded_card_container)
            .width(Length::Fill),
        Space::new().height(Length::Fixed(32.0)),
        section_header("folder", "Index Directories"),
        container(index_directories_section(app))
            .padding(20)
            .style(theme::padded_card_container)
            .width(Length::Fill),
        Space::new().height(Length::Fixed(32.0)),
        section_header("gear", "System & Desktop Preferences"),
        container(system_integration_section(app))
            .padding(20)
            .style(theme::padded_card_container)
            .width(Length::Fill),
        Space::new().height(Length::Fixed(32.0)),
        section_header("sun", "Appearance & Theme"),
        container(appearance_section(app))
            .padding(20)
            .style(theme::padded_card_container)
            .width(Length::Fill),
        Space::new().height(Length::Fixed(32.0)),
        section_header("database", "Data Management"),
        container(data_management_section(app))
            .padding(20)
            .style(theme::padded_card_container)
            .width(Length::Fill),
        Space::new().height(Length::Fixed(32.0)),
        section_header("info", "Privacy & Local Security"),
        container(privacy_security_section())
            .padding(20)
            .style(theme::padded_card_container)
            .width(Length::Fill),
        Space::new().height(Length::Fixed(40.0)),
        container(
            button(
                row![
                    load_icon_size("check", 16.0),
                    text("Save All Changes").size(15).font(Font {
                        weight: font::Weight::Bold,
                        ..Font::default()
                    })
                ]
                .spacing(8)
                .align_y(Alignment::Center)
            )
            .on_press(Message::SaveSettings)
            .padding(Padding::from([12, 28]))
            .style(theme::primary_button())
        )
        .width(Length::Fill)
        .align_x(Alignment::Center),
        Space::new().height(Length::Fixed(32.0)),
    ]
    .width(Length::Fill)
    .into()
}

fn section_header<'a>(icon: &'a str, title: &'a str) -> Element<'a, Message> {
    column![
        row![
            load_icon_size(icon, 18.0),
            text(title).size(18).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        Space::new().height(Length::Fixed(8.0)),
    ]
    .into()
}

fn search_settings_fields(app: &App) -> Element<'_, Message> {
    column![
        row![
            column![
                text("Maximum Search Results").size(14).font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                }),
                text("Limits total search results returned for performance")
                    .size(12)
                    .style(theme::dim_text_style()),
            ]
            .spacing(2)
            .width(Length::Fill),
            TextInput::new("100", &app.settings.max_results.to_string())
                .padding(Padding::new(10.0))
                .size(14)
                .width(Length::Fixed(120.0))
                .on_input(Message::MaxResultsChanged)
                .style(theme::search_input())
        ]
        .spacing(12)
        .align_y(Alignment::Center),

        Space::new().height(Length::Fixed(16.0)),
        column![
            text("Exclude Patterns (comma separated)").size(14).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }),
            text("Folder and file patterns to skip during indexing (e.g. *.git, target, node_modules)")
                .size(12)
                .style(theme::dim_text_style()),
        ]
        .spacing(2),
        Space::new().height(Length::Fixed(6.0)),
        TextInput::new(
            "*.git, target, node_modules",
            &app.settings.exclude_patterns.join(", ")
        )
        .padding(Padding::new(12.0))
        .size(13)
        .on_input(Message::ExcludePatternsChanged)
        .style(theme::search_input()),

        Space::new().height(Length::Fixed(16.0)),
        column![
            text("Custom File Extensions").size(14).font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            }),
            text("Additional file formats to index as plain text (comma separated)")
                .size(12)
                .style(theme::dim_text_style()),
        ]
        .spacing(2),
        Space::new().height(Length::Fixed(6.0)),
        TextInput::new("e.g. log, env, conf, sdp", &app.settings.custom_extensions)
            .padding(Padding::new(12.0))
            .size(13)
            .on_input(Message::CustomExtensionsChanged)
            .style(theme::search_input()),

        Space::new().height(Length::Fixed(16.0)),
        row![
            column![
                text("Global Search Hotkey").size(14).font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                }),
                text("Keyboard shortcut to summon FindAll from anywhere")
                    .size(12)
                    .style(theme::dim_text_style()),
            ]
            .spacing(2)
            .width(Length::Fill),
            TextInput::new("Alt+Space", &app.settings.global_hotkey)
                .padding(Padding::new(10.0))
                .size(13)
                .width(Length::Fixed(200.0))
                .on_input(Message::GlobalHotkeyChanged)
                .style(theme::search_input())
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    ]
    .into()
}

fn index_directories_section(app: &App) -> Element<'_, Message> {
    let mut dirs_col = column![].spacing(10);

    if app.settings.index_dirs.is_empty() {
        dirs_col = dirs_col.push(
            container(
                text("No directories configured for indexing.")
                    .size(13)
                    .style(theme::dim_text_style()),
            )
            .padding(16.0)
            .style(theme::hit_highlight_container)
            .width(Length::Fill),
        );
    } else {
        for (i, dir) in app.settings.index_dirs.iter().enumerate() {
            dirs_col = dirs_col.push(
                container(
                    row![
                        load_icon_size("folder-open", 16.0),
                        text(dir).size(13).width(Length::Fill),
                        button(load_icon_size("trash", 15.0))
                            .on_press(Message::RemoveFolder(i))
                            .padding(Padding::new(6.0))
                            .style(theme::ghost_button())
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                )
                .style(theme::badge_container)
                .padding(Padding::new(10.0))
                .width(Length::Fill),
            );
        }
    }

    column![
        dirs_col,
        Space::new().height(Length::Fixed(8.0)),
        button(
            row![
                load_icon_size("plus", 14.0),
                text("Add Directory to Index").size(13)
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        )
        .on_press(Message::AddFolder)
        .padding(Padding::from([8, 16]))
        .style(theme::secondary_button())
    ]
    .spacing(8)
    .into()
}

fn system_integration_section(app: &App) -> Element<'_, Message> {
    column![
        checkbox(app.settings.minimize_to_tray)
            .label("Minimize to system tray on window close")
            .on_toggle(Message::ToggleMinimizeToTray)
            .size(18)
            .text_size(13),
        checkbox(app.settings.auto_start_on_boot)
            .label("Start FindAll automatically when system starts")
            .on_toggle(Message::ToggleAutoStart)
            .size(18)
            .text_size(13),
        checkbox(app.settings.context_menu_enabled)
            .label("Add 'Search with FindAll' to Windows right-click context menu")
            .on_toggle(Message::ToggleContextMenu)
            .size(18)
            .text_size(13),
        checkbox(app.settings.use_gitignore)
            .label("Respect .gitignore rules when scanning repository folders")
            .on_toggle(Message::ToggleGitignore)
            .size(18)
            .text_size(13),
    ]
    .spacing(14)
    .into()
}

fn appearance_section(app: &App) -> Element<'_, Message> {
    column![
        row![
            column![
                text("Color Theme").size(14).font(Font {
                    weight: font::Weight::Bold,
                    ..Font::default()
                }),
                text("Switch between Windows 11 Dark mode and Light mode")
                    .size(12)
                    .style(theme::dim_text_style()),
            ]
            .spacing(2)
            .width(Length::Fill),
            checkbox(app.is_dark)
                .label("Dark Theme")
                .on_toggle(|_| Message::ToggleTheme)
                .size(20)
                .text_size(13),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
    ]
    .into()
}

fn data_management_section(_app: &App) -> Element<'_, Message> {
    column![
        text("Force Complete Index Rebuild")
            .size(14)
            .font(Font { weight: font::Weight::Bold, ..Font::default() }),
        text("Clears all cached search index data and performs a fresh scan across all configured directories.")
            .size(12)
            .style(theme::dim_text_style()),
        Space::new().height(Length::Fixed(10.0)),
        button(
            row![load_icon_size("database", 14.0), text("Rebuild Search Index").size(13)]
                .spacing(8)
                .align_y(Alignment::Center)
        )
        .on_press(Message::RebuildIndex)
        .padding(Padding::from([8, 18]))
        .style(theme::secondary_button())
    ]
    .spacing(6)
    .into()
}

fn privacy_security_section() -> Element<'static, Message> {
    let app_dir_str = crate::get_app_data_dir().map_or_else(
        |_| "Unknown".to_string(),
        |p| p.to_string_lossy().to_string(),
    );

    column![
        row![
            load_icon_size("check", 16.0),
            text("FindAll operates 100% locally. Zero data or search queries ever leave your machine.")
                .size(13)
                .style(theme::muted_text_style()),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        Space::new().height(Length::Fixed(8.0)),
        container(
            column![
                row![
                    text("Local App Data Path: ").size(12).font(Font { weight: font::Weight::Bold, ..Font::default() }),
                    text(app_dir_str.clone()).size(12).font(Font::MONOSPACE),
                ].spacing(8),
                row![
                    text("Tantivy Search Index: ").size(12).font(Font { weight: font::Weight::Bold, ..Font::default() }),
                    text(format!("{app_dir_str}/index")).size(12).font(Font::MONOSPACE),
                ].spacing(8),
                row![
                    text("Redb KV Store: ").size(12).font(Font { weight: font::Weight::Bold, ..Font::default() }),
                    text(format!("{app_dir_str}/metadata.redb")).size(12).font(Font::MONOSPACE),
                ].spacing(8),
            ].spacing(6)
        )
        .padding(14)
        .style(theme::badge_container)
        .width(Length::Fill),
    ]
    .spacing(6)
    .into()
}
