use chrono::{DateTime, Utc};
use author::DiscordMessageAuthor;
use content::DiscordMessageContent;
use gpui::{Element, IntoElement};
use scope_chat::{async_list::AsyncListItem, message::Message};

use crate::snowflake::Snowflake;

pub mod author;
pub mod content;

#[derive(Clone)]
pub struct DiscordMessage {
  pub content: DiscordMessageContent,
  pub author: DiscordMessageAuthor,
  pub id: Snowflake,
  pub nonce: Option<String>,
  pub creation_time: serenity::model::Timestamp,
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

  fn get_nonce(&self) -> Option<&String> {
    self.nonce.as_ref()
  }

  fn should_group(&self, previous: &Self) -> bool {
    const MAX_DISCORD_MESSAGE_GAP_SECS_FOR_GROUP: i64 = 5 * 60;

    self.creation_time.signed_duration_since(*previous.creation_time).num_seconds() <= MAX_DISCORD_MESSAGE_GAP_SECS_FOR_GROUP
  }

  fn get_timestamp(&self) -> Option<DateTime<Utc>> {
    let ts = self.creation_time.timestamp_millis();

    DateTime::from_timestamp_millis(ts)
  }
}

impl AsyncListItem for DiscordMessage {
  type Identifier = String;
}
