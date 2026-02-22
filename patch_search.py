import sys
import re

with open('src/iced_ui/search.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# Replace the rebuild_btn definition
pattern1 = r'let rebuild_btn = button\("Rebuild Index"\)\.on_press\(Message::RebuildIndex\)\.padding\(8\.0\)\.style\(theme::Button::Secondary\);'
replacement1 = '''let rebuild_display: Element<_> = if let Some(progress) = app.rebuild_progress {
        let status = app.rebuild_status.as_deref().unwrap_or("Rebuilding...");
        row![
            text(status).size(14),
            iced::widget::ProgressBar::new(0.0..=1.0, progress).height(Length::Fixed(8.0)).width(Length::Fixed(100.0)),
        ].spacing(8).align_items(Alignment::Center).into()
    } else {
        button("Rebuild Index").on_press(Message::RebuildIndex).padding(8.0).style(theme::Button::Secondary).into()
    };'''

# Replace rebuild_btn in the row array
pattern2 = r'rebuild_btn, theme_btn, settings_btn'
replacement2 = r'rebuild_display, theme_btn, settings_btn'

new_content = re.sub(pattern1, replacement1, content)
new_content = re.sub(pattern2, replacement2, new_content)

if new_content != content:
    with open('src/iced_ui/search.rs', 'w', encoding='utf-8') as f:
        f.write(new_content)
    print("search.rs successfully patched")
else:
    print("Failed to patch search.rs")
