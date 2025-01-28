use crate::message::reaction_list::DiscordReactionList;
use gpui::prelude::FluentBuilder;
use gpui::{div, Context, IntoElement, ParentElement, Render, Styled, Window};
use serenity::all::Message;

#[derive(Clone, Debug)]
pub struct DiscordMessageContent {
  pub content: String,
  pub is_pending: bool,
  pub reactions: Option<DiscordReactionList>,
}

impl DiscordMessageContent {
  pub fn pending(content: String) -> DiscordMessageContent {
    DiscordMessageContent {
      content,
      is_pending: true,
      reactions: None,
    }
  }

  pub fn received(message: &Message, reactions: &DiscordReactionList) -> DiscordMessageContent {
    DiscordMessageContent {
      content: message.content.clone(),
      is_pending: false,
      reactions: Some(reactions.clone()),
    }
  }
}

impl Render for DiscordMessageContent {
  fn render(&mut self, _: &mut Window, _: &mut Context<DiscordMessageContent>) -> impl IntoElement {
    div()
      .opacity(if self.is_pending { 0.25 } else { 1.0 })
      .child(self.content.clone())
      .when_some(self.reactions.clone(), |d, reactions| d.child(reactions))
  }
}
