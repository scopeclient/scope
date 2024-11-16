use gpui::{ListState, Pixels};
use scope_backend_discord::{
  message::{
    author::{DiscordMessageAuthor, DisplayName},
    content::DiscordMessageContent,
    DiscordMessage,
  },
  snowflake::Snowflake,
};
use scope_chat::message::Message;

pub struct MessageList<M: Message> {
  pub(super) messages: Vec<M>,
}

impl<M: Message> MessageList<M> {
  pub fn new() -> MessageList<M> {
    Self { messages: Vec::default() }
  }
}
