use author::DiscordMessageAuthor;
use content::DiscordMessageContent;
use scope_chat::message::Message;

use crate::snowflake::Snowflake;

pub mod content;
pub mod author;

struct DiscordMessage {
  content: DiscordMessageContent,
  author: DiscordMessageAuthor,
  id: Snowflake,
}

impl Message for DiscordMessage {
  fn get_author(&self) -> &impl scope_chat::message::MessageAuthor {
    &self.author
  }

  fn get_content(&self) -> &impl gpui::Render {
    &self.content
  }

  fn get_identifier(&self) -> String {
    self.id.to_string()
  }
}

