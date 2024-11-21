use chrono::Local;
use gpui::{div, img, prelude::FluentBuilder, rgb, Element, IntoElement, ParentElement, Render, Styled, StyledImage, View, WindowContext};
use scope_chat::message::{IconRenderConfig, Message, MessageAuthor};

#[derive(Clone)]
pub struct MessageGroup<M: Message> {
  contents: Vec<M>,
}

impl<M: Message> MessageGroup<M> {
  pub fn new(message: M) -> MessageGroup<M> {
    MessageGroup { contents: vec![message] }
  }

  pub fn get_author(&self) -> M::Author {
    self.contents.first().unwrap().get_author()
  }

  pub fn add(&mut self, message: M) {
    // FIXME: This is scuffed, should be using PartialEq trait.
    if self.get_author().get_identifier() != message.get_author().get_identifier() {
      panic!("Authors must match in a message group")
    }

    self.contents.push(message);
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

pub fn message_group<M: Message>(group: MessageGroup<M>, cx: &mut WindowContext) -> impl IntoElement {
  div()
    .flex()
    .flex_row()
    .text_color(rgb(0xFFFFFF))
    .gap_4()
    .pb_6()
    .child(div().child(group.get_author().get_icon(IconRenderConfig::small())).flex_shrink_0().rounded_full().w_12().h_12())
    .child(
      div()
        .flex()
        .min_w_0()
        .flex_shrink()
        .flex_col()
        // enabling this, and thus enabling ellipsis causes a consistent panic!?
        // .child(div().text_ellipsis().min_w_0().child(message.get_author().get_display_name()))
        .child(
          div().min_w_0().flex().gap_2().child(group.get_author().get_display_name()).when_some(group.last().get_timestamp(), |d, ts| {
            d.child(div().min_w_0().text_color(rgb(0xAFBAC7)).text_sm().child(ts.with_timezone(&Local).format("%I:%M %p").to_string()))
          }),
        )
        .children(group.contents.iter().map(|v| v.get_content(cx).clone())),
    )
}
