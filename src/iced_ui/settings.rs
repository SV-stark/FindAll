use super::{theme, App, Message, Tab};
use iced::widget::{button, checkbox, column, container, row, text, Scrollable, Space, TextInput};
use iced::{Alignment, Element, Length, Padding};

pub fn settings_view(app: &App) -> Element<Message> {
    let title = text("Settings").size(28);

    let search_tab_style = theme::tab_button(matches!(app.active_tab, Tab::Search));
    let settings_tab_style = theme::tab_button(matches!(app.active_tab, Tab::Settings));

    let search_tab = button(text("Search").size(16))
        .on_press(Message::TabChanged(Tab::Search))
        .padding(Padding::new(12.0))
        .style(search_tab_style);

    let settings_tab = button(text("Settings").size(16))
        .on_press(Message::TabChanged(Tab::Settings))
        .padding(Padding::new(12.0))
        .style(settings_tab_style);

    let tabs = row![search_tab, settings_tab].spacing(8);

    let max_results_label = text("Max Results").size(14);

    let max_results_val = app.settings.max_results.to_string();
    let max_results_input = TextInput::new("100", &max_results_val)
        .padding(Padding::new(10.0))
        .size(14)
        .width(Length::Fixed(120.0))
        .on_input(Message::MaxResultsChanged)
        .style(theme::small_input());

    let exclude_label = text("Exclude Patterns").size(14);
    let exclude_val = app.settings.exclude_patterns.join(", ");
    let exclude_placeholder = "*.git, target, node_modules";
    let exclude_input = TextInput::new(exclude_placeholder, &exclude_val)
        .padding(Padding::new(10.0))
        .size(14)
        .width(Length::Fill)
        .on_input(Message::ExcludePatternsChanged)
        .style(theme::search_input());

    let dirs_section = column![text("Index Directories").size(18)].spacing(12);
    let mut dirs_col = column!().spacing(8);
    for (i, dir) in app.settings.index_dirs.iter().enumerate() {
        let row_item = container(
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
        .width(Length::Fill);

        dirs_col = dirs_col.push(row_item);
    }

    let add_btn = button(text("+ Add Folder").size(14))
        .on_press(Message::AddFolder)
        .padding(Padding::new(12.0))
        .style(theme::secondary_button());

    let save_btn = button(text("Save Settings").size(15))
        .on_press(Message::SaveSettings)
        .padding(Padding::new(12.0))
        .style(theme::primary_button());

    let sys_int_section = column![text("System Integration").size(18)].spacing(12);

    let tray_toggle = row![
        checkbox(app.settings.minimize_to_tray).on_toggle(Message::ToggleMinimizeToTray),
        text("Minimize to system tray").size(14),
    ]
    .spacing(8);

    let autostart_toggle = row![
        checkbox(app.settings.auto_start_on_boot).on_toggle(Message::ToggleAutoStart),
        text("Start automatically on boot").size(14),
    ]
    .spacing(8);

    let context_toggle = row![
        checkbox(app.settings.context_menu_enabled).on_toggle(Message::ToggleContextMenu),
        text("Add to context menu (right-click)").size(14),
    ]
    .spacing(8);

    let sys_toggles = column![tray_toggle, autostart_toggle, context_toggle].spacing(12);

    let settings_form = column![
        text("Search").size(18),
        Space::new().height(Length::Fixed(8.0)),
        row![max_results_label, max_results_input]
            .spacing(12)
            .align_y(Alignment::Center),
        Space::new().height(Length::Fixed(16.0)),
        exclude_label,
        Space::new().height(Length::Fixed(6.0)),
        exclude_input,
        Space::new().height(Length::Fixed(24.0)),
        dirs_section,
        dirs_col,
        Space::new().height(Length::Fixed(4.0)),
        add_btn,
        Space::new().height(Length::Fixed(32.0)),
        sys_int_section,
        Space::new().height(Length::Fixed(8.0)),
        sys_toggles,
        Space::new().height(Length::Fixed(40.0)),
        save_btn,
    ]
    .spacing(8)
    .width(Length::Fill);

    let center_content = container(settings_form)
        .width(Length::Fixed(700.0))
        .align_x(Alignment::Center);

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
        .padding(Padding::new(32.0))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
