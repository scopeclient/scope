use gpui::{div, img, rgb, IntoElement, ParentElement, Styled};
use scope_chat::message::{Message, MessageAuthor};

pub fn message(message: impl Message) -> impl IntoElement {
  div()
    .flex()
    .flex_row()
    .text_color(rgb(0xFFFFFF))
    .gap_2()
    .p_2()
    .child(img(message.get_author().get_icon()).object_fit(gpui::ObjectFit::Fill).bg(rgb(0xFFFFFF)).rounded_full().w_12().h_12())
    .child(div().flex().flex_col().child(message.get_author().get_display_name()).child(message.get_content()).child(message.get_images()))
}
