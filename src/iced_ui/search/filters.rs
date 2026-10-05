//! Sidebar: filter sections and the chips summarising the active filters.
//!
//! Every widget here reads the same handful of `App` fields
//! (`filter_extensions`, `min_size`, `date_filter`, `search_mode`, `sort_by`) and
//! renders one control per setting. The small `*_button` helpers are the
//! per-option widgets those sections are built from.
//!
//! Only the three panel entry points are visible to the parent module;
//! everything else is an implementation detail of this sidebar.

use super::super::{App, DateFilter, Message, SearchMode, SortBy, theme};
use crate::iced_ui::icons::load_icon_size;
use iced::widget::{
    Space, TextInput, button, checkbox, column, container, mouse_area, row, scrollable, text,
};
use iced::{Alignment, Element, Font, Length, Padding, font};

fn sidebar_section<'a>(
    title: &'a str,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    column![
        text(title)
            .size(theme::fs(12.0))
            .font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            })
            .style(theme::muted_text_style()),
        container(content.into())
            .padding(Padding::new(12.0))
            .style(theme::sidebar_panel_container)
            .width(Length::Fill)
    ]
    .spacing(8)
    .into()
}

pub(super) fn collapsed_sidebar(_app: &App) -> Element<'_, Message> {
    container(
        column![
            button(load_icon_size("filter", 18.0))
                .on_press(Message::ToggleSidebar)
                .style(theme::ghost_button())
                .padding(Padding::new(12.0)),
        ]
        .spacing(16)
        .padding(Padding::new(4.0))
        .align_x(Alignment::Center),
    )
    .style(theme::sidebar_container)
    .height(Length::Fill)
    .width(Length::Fixed(56.0))
    .into()
}

pub(super) fn left_sidebar(app: &App) -> Element<'_, Message> {
    let filter_header = row![
        load_icon_size("filter", 16.0),
        text("Filter Options").size(theme::fs(15.0)).font(Font {
            weight: font::Weight::Bold,
            ..Font::default()
        }),
        Space::new().width(Length::Fill),
        button(load_icon_size("chevron-left", 16.0))
            .on_press(Message::ToggleSidebar)
            .style(theme::ghost_button())
            .padding(Padding::new(6.0))
    ]
    .align_y(Alignment::Center)
    .spacing(8);

    let filter_content = scrollable(
        column![
            category_filter_section(app),
            sort_order_section(app),
            extension_filter_section(app),
            size_filter_section(app),
            date_filter_section(app),
            match_options_section(app),
            Space::new().height(Length::Fill),
            button(
                row![
                    load_icon_size("x", 14.0),
                    text("Reset All Filters").size(theme::fs(12.0))
                ]
                .spacing(6)
                .align_y(Alignment::Center)
            )
            .on_press(Message::ClearFilters)
            .style(theme::secondary_button())
            .width(Length::Fill)
            .padding(Padding::new(8.0)),
        ]
        .spacing(20),
    )
    .height(Length::Fill);

    let filter_panel = column![
        filter_header,
        Space::new().height(Length::Fixed(4.0)),
        filter_content,
    ]
    .spacing(14)
    .padding(Padding::new(18.0));

    container(filter_panel)
        .style(theme::sidebar_container)
        .width(Length::Fixed(290.0))
        .height(Length::Fill)
        .into()
}

fn extension_filter_section(app: &App) -> Element<'_, Message> {
    sidebar_section(
        "File Extension",
        column![
            row![
                extension_checkbox("pdf", app),
                extension_checkbox("docx", app),
            ]
            .spacing(12),
            row![
                extension_checkbox("md", app),
                extension_checkbox("txt", app),
            ]
            .spacing(12),
            row![extension_checkbox("rs", app), extension_checkbox("py", app),].spacing(12),
            row![
                extension_checkbox("json", app),
                extension_checkbox("csv", app),
            ]
            .spacing(12),
            row![
                extension_checkbox("log", app),
                extension_checkbox("cpp", app),
            ]
            .spacing(12),
        ]
        .spacing(8),
    )
}

fn size_filter_section(app: &App) -> Element<'_, Message> {
    sidebar_section(
        "Size Range",
        column![
            row![
                TextInput::new("Min", &app.min_size)
                    .on_input(Message::MinSizeChanged)
                    .padding(Padding::new(7.0))
                    .size(theme::fs(12.0))
                    .style(theme::search_input())
                    .width(Length::Fill),
                text("-")
                    .size(theme::fs(14.0))
                    .style(theme::dim_text_style()),
                TextInput::new("Max", &app.max_size)
                    .on_input(Message::MaxSizeChanged)
                    .padding(Padding::new(7.0))
                    .size(theme::fs(12.0))
                    .style(theme::search_input())
                    .width(Length::Fill),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            row![
                size_unit_button("KB", app),
                size_unit_button("MB", app),
                size_unit_button("GB", app),
            ]
            .spacing(4)
        ]
        .spacing(10),
    )
}

fn date_filter_section(app: &App) -> Element<'_, Message> {
    sidebar_section(
        "Last Modified",
        column![
            date_filter_button("Anytime", DateFilter::Anytime, app),
            date_filter_button("Today", DateFilter::Today, app),
            date_filter_button("Past Week", DateFilter::Last7Days, app),
            date_filter_button("Past Month", DateFilter::Last30Days, app),
        ]
        .spacing(4),
    )
}

fn match_options_section(app: &App) -> iced::widget::Column<'_, Message> {
    column![
        text("Search Scope")
            .size(theme::fs(12.0))
            .font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            })
            .style(theme::muted_text_style()),
        container(
            row![
                search_mode_button("Full Text", SearchMode::FullText, app),
                search_mode_button("Filename", SearchMode::Filename, app),
            ]
            .spacing(4)
        )
        .padding(Padding::new(4.0))
        .style(theme::sidebar_panel_container)
        .width(Length::Fill),
        Space::new().height(Length::Fixed(6.0)),
        text("Match Flags")
            .size(theme::fs(12.0))
            .font(Font {
                weight: font::Weight::Bold,
                ..Font::default()
            })
            .style(theme::muted_text_style()),
        container(
            column![
                checkbox(app.settings.case_sensitive)
                    .label("Match Case")
                    .on_toggle(Message::ToggleCaseSensitive)
                    .size(theme::fs(16.0))
                    .text_size(12),
                checkbox(app.settings.whole_word)
                    .label("Whole Word")
                    .on_toggle(Message::ToggleWholeWord)
                    .size(theme::fs(16.0))
                    .text_size(12),
            ]
            .spacing(8)
        )
        .padding(Padding::new(10.0))
        .style(theme::sidebar_panel_container)
        .width(Length::Fill),
    ]
    .spacing(6)
}

fn search_mode_button<'a>(label: &'a str, mode: SearchMode, app: &App) -> Element<'a, Message> {
    let is_active = app.search_mode == mode;
    button(text(label).size(theme::fs(11.0)).font(Font {
        weight: font::Weight::Bold,
        ..Font::default()
    }))
    .on_press(Message::SearchModeChanged(mode))
    .style(move |t: &iced::Theme, s| {
        if is_active {
            theme::primary_button()(t, s)
        } else {
            theme::secondary_button()(t, s)
        }
    })
    .width(Length::Fill)
    .padding(Padding::from([5, 10]))
    .into()
}

fn sort_order_section(app: &App) -> Element<'_, Message> {
    sidebar_section(
        "Sort Results By",
        column![
            sort_button("Relevance Score", SortBy::Relevance, app),
            sort_button("Date Modified", SortBy::DateModified, app),
            sort_button("File Size", SortBy::Size, app),
            sort_button("File Name", SortBy::Name, app),
        ]
        .spacing(4),
    )
}

fn sort_button<'a>(label: &'a str, sort: SortBy, app: &App) -> Element<'a, Message> {
    let is_active = app.sort_by == sort;
    button(text(label).size(theme::fs(12.0)))
        .on_press(Message::SortByChanged(sort))
        .style(move |t: &iced::Theme, s| {
            if is_active {
                theme::nav_button(true)(t, s)
            } else {
                theme::ghost_button()(t, s)
            }
        })
        .width(Length::Fill)
        .padding(Padding::new(7.0))
        .into()
}

fn category_filter_section(app: &App) -> Element<'_, Message> {
    sidebar_section(
        "Quick Categories",
        column![
            category_preset_button("📄 Documents", &["pdf", "docx", "md", "txt"], app),
            category_preset_button("💻 Source Code", &["rs", "py", "js", "ts", "cpp"], app),
            category_preset_button("📊 Data & Logs", &["json", "csv", "log", "xml"], app),
            category_preset_button("🖼️ Images", &["png", "jpg", "jpeg", "svg"], app),
        ]
        .spacing(4),
    )
}

fn category_preset_button<'a>(
    label: &'a str,
    exts: &'static [&'static str],
    app: &App,
) -> Element<'a, Message> {
    let is_active = exts.iter().all(|e| app.filter_extensions.contains(*e));
    let exts_vec: Vec<String> = exts.iter().map(|s| (*s).to_string()).collect();

    button(text(label).size(theme::fs(12.0)))
        .on_press(Message::ToggleCategory(exts_vec))
        .style(move |t: &iced::Theme, s| {
            if is_active {
                theme::nav_button(true)(t, s)
            } else {
                theme::ghost_button()(t, s)
            }
        })
        .width(Length::Fill)
        .padding(Padding::new(7.0))
        .into()
}

fn extension_checkbox<'a>(ext: &'a str, app: &App) -> Element<'a, Message> {
    checkbox(app.filter_extensions.contains(ext))
        .label(ext)
        .on_toggle(move |_| Message::ToggleFilterExtension(ext.to_string()))
        .size(theme::fs(16.0))
        .text_size(12)
        .into()
}

fn size_unit_button<'a>(unit: &'a str, app: &App) -> Element<'a, Message> {
    let is_active = app.size_unit == unit;
    button(text(unit).size(theme::fs(11.0)).font(Font {
        weight: font::Weight::Bold,
        ..Font::default()
    }))
    .on_press(Message::SizeUnitChanged(unit.to_string()))
    .style(move |t: &iced::Theme, s| {
        if is_active {
            theme::primary_button()(t, s)
        } else {
            theme::secondary_button()(t, s)
        }
    })
    .padding(Padding::from([4, 10]))
    .into()
}

fn date_filter_button<'a>(label: &'a str, filter: DateFilter, app: &App) -> Element<'a, Message> {
    let is_active = app.date_filter == filter;
    button(text(label).size(theme::fs(12.0)))
        .on_press(Message::DateFilterChanged(filter))
        .style(move |t: &iced::Theme, s| {
            if is_active {
                theme::nav_button(true)(t, s)
            } else {
                theme::ghost_button()(t, s)
            }
        })
        .width(Length::Fill)
        .padding(Padding::new(7.0))
        .into()
}

pub(super) fn filter_chips(app: &App) -> Element<'_, Message> {
    if app.filter_extensions.is_empty() {
        return Space::new().height(0).into();
    }

    let mut chips_row = row![
        load_icon_size("filter", 14.0),
        text("Active Filters:")
            .size(theme::fs(12.0))
            .style(theme::dim_text_style())
    ]
    .spacing(8)
    .padding(Padding {
        top: 6.0,
        bottom: 6.0,
        left: 16.0,
        right: 16.0,
    })
    .align_y(Alignment::Center);

    for ext in &app.filter_extensions {
        let ext_clone = ext.clone();
        chips_row = chips_row.push(
            container(
                row![
                    text(ext).size(theme::fs(11.0)).font(Font {
                        weight: font::Weight::Bold,
                        ..Font::default()
                    }),
                    mouse_area(load_icon_size("x", 12.0))
                        .on_press(Message::ToggleFilterExtension(ext_clone))
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([3, 8]))
            .style(|t| theme::file_badge_container(t, Some(ext))),
        );
    }

    container(chips_row)
        .width(Length::Fill)
        .style(theme::header_container)
        .into()
}
