use gpui::{div, img, rgb, Element, IntoElement, ParentElement, Styled};
use scope_chat::message::{Message, MessageAuthor};

#[derive(Clone)]
pub struct MessageGroup<M: Message> {
  contents: Vec<M>,
}

impl<M: Message> MessageGroup<M> {
  pub fn new(message: M) -> MessageGroup<M> {
    MessageGroup { contents: vec![message] }
  }

  pub fn get_author<'s>(&'s self) -> &'s (impl MessageAuthor + 's) {
    self.contents.get(0).unwrap().get_author()
  }

  pub fn add(&mut self, message: M) {
    // FIXME: This is scuffed, should be using PartialEq trait.
    if self.get_author().get_id() != message.get_author().get_id() {
      panic!("Authors must match in a message group")
    }

    self.contents.push(message);
  }

  pub fn contents<'s>(&'s self) -> impl IntoIterator<Item = impl Element + 's> {
    self.contents.iter().map(|v| v.get_content())
  }

  pub fn find_matching(&self, nonce: &String) -> Option<usize> {
    for haystack in self.contents.iter().zip(0usize..) {
      if haystack.0.get_nonce().is_some() && haystack.0.get_nonce().unwrap() == nonce {
        return Some(haystack.1);
      }
    }

    return None;
  }

  pub fn size(&self) -> usize {
    self.contents.len()
  }

  pub fn remove(&mut self, index: usize) {
    if self.size() == 1 {
      panic!("Cannot remove such that it would leave the group empty.");
    }

    self.contents.remove(index);
  }
}

pub fn message<M: Message>(message: MessageGroup<M>) -> impl IntoElement {
  div()
    .flex()
    .flex_row()
    .text_color(rgb(0xFFFFFF))
    .gap_4()
    .pb_6()
    .child(img(message.get_author().get_icon()).flex_shrink_0().object_fit(gpui::ObjectFit::Fill).bg(rgb(0xFFFFFF)).rounded_full().w_12().h_12())
    .child(
      div()
        .flex()
        .min_w_0()
        .flex_shrink()
        .flex_col()
        // enabling this, and thus enabling ellipsis causes a consistent panic!?
        // .child(div().text_ellipsis().min_w_0().child(message.get_author().get_display_name()))
        .child(div().min_w_0().child(message.get_author().get_display_name()))
        .children(message.contents()),
    )
}
