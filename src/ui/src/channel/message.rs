use chrono::Local;
use gpui::prelude::FluentBuilder;
use gpui::{div, img, rgb, Element, IntoElement, ParentElement, Styled, StyledImage};
use scope_chat::message::{Message, MessageAuthor};

#[derive(Clone)]
pub struct MessageGroup<M: Message> {
  contents: Vec<M>,
}

impl<M: Message> MessageGroup<M> {
  pub fn new(message: M) -> MessageGroup<M> {
    MessageGroup { contents: vec![message] }
  }

  pub fn get_author(&self) -> &(impl MessageAuthor + '_) {
    self.contents.first().unwrap().get_author()
  }

  pub fn add(&mut self, message: M) {
    // FIXME: This is scuffed, should be using PartialEq trait.
    if self.get_author().get_id() != message.get_author().get_id() {
      panic!("Authors must match in a message group")
    }

    self.contents.push(message);
  }

  pub fn contents(&self) -> impl IntoIterator<Item = impl Element + '_> {
    self.contents.iter().map(|v| v.get_content())
  }

  pub fn find_matching(&self, nonce: &String) -> Option<usize> {
    for haystack in self.contents.iter().zip(0usize..) {
      if haystack.0.get_nonce().is_some() && haystack.0.get_nonce().unwrap() == nonce {
        return Some(haystack.1);
      }
    }

    None
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

  pub fn last(&self) -> &M {
    self.contents.last().unwrap()
  }
}

pub fn message<M: Message>(message: MessageGroup<M>) -> impl IntoElement {
  div()
    .flex()
    .flex_row()
    .text_color(rgb(0xFFFFFF))
    .gap_4()
    .pb_6()
    .child(img(message.get_author().get_small_icon()).flex_shrink_0().object_fit(gpui::ObjectFit::Fill).rounded_full().w_12().h_12())
    .child(
      div()
        .flex()
        .min_w_0()
        .flex_shrink()
        .flex_col()
        // enabling this, and thus enabling ellipsis causes a consistent panic!?
        // .child(div().text_ellipsis().min_w_0().child(message.get_author().get_display_name()))
        .child(
          div().min_w_0().flex().gap_2().child(message.get_author().get_display_name()).when_some(message.last().get_timestamp(), |d, ts| {
            d.child(div().min_w_0().text_color(rgb(0xAFBAC7)).text_sm().child(ts.with_timezone(&Local).format("%I:%M %p").to_string()))
          }),
        )
        .children(message.contents()),
    )
}
