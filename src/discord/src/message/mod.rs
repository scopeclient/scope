use author::DiscordMessageAuthor;
use content::DiscordMessageContent;
use gpui::{Element, IntoElement};
use scope_chat::message::Message;

use crate::snowflake::Snowflake;

pub mod author;
pub mod content;

#[derive(Clone)]
pub struct DiscordMessage {
  pub content: DiscordMessageContent,
  pub author: DiscordMessageAuthor,
  pub id: Snowflake,
  pub nonce: Option<String>,
}

impl Message for DiscordMessage {
  fn get_author(&self) -> &impl scope_chat::message::MessageAuthor {
    &self.author
  }

  fn get_content(&self) -> impl Element {
    self.content.clone().into_element()
  }

  fn get_identifier(&self) -> String {
    self.id.to_string()
  }

  fn get_nonce(&self) -> Option<String> {
    self.nonce.clone()
  }
}
