use iced::widget::text_input;
fn main() {
    let id = text_input::Id::new("my_id");
    let t = text_input::focus(id);
}
