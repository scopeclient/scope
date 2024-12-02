use crate::message::reaction_list::DiscordReactionList;
use gpui::{div, IntoElement, ParentElement, Render, Styled, ViewContext};
use serenity::all::Message;

#[derive(Clone, Debug)]
pub struct DiscordMessageContent {
  pub content: String,
  pub is_pending: bool,
  pub reactions: DiscordReactionList,
}

impl DiscordMessageContent {
  pub fn pending(content: String) -> DiscordMessageContent {
    DiscordMessageContent {
      content,
      is_pending: true,
      reactions: Default::default(),
    }
  }

  pub fn received(message: &Message, reactions: &DiscordReactionList) -> DiscordMessageContent {
    DiscordMessageContent {
      content: message.content.clone(),
      is_pending: false,
      reactions: reactions.clone(),
    }
  }
}

impl Render for DiscordMessageContent {
  fn render(&mut self, _: &mut ViewContext<DiscordMessageContent>) -> impl IntoElement {
    div()
        .opacity(if self.is_pending { 0.25 } else { 1.0 })
        .child(self.content.clone())
        .child(self.reactions.clone())
  }
}
