use author::{DiscordMessageAuthor, DisplayName};
use chrono::{DateTime, Utc};
use content::DiscordMessageContent;
use gpui::{Element, IntoElement};
use scope_chat::{async_list::AsyncListItem, message::Message};
use serenity::all::Nonce;

use crate::snowflake::Snowflake;

pub mod author;
pub mod content;

#[derive(Clone, Debug)]
pub struct DiscordMessage {
  pub content: DiscordMessageContent,
  pub author: DiscordMessageAuthor,
  pub id: Snowflake,
  pub nonce: Option<String>,
  pub creation_time: serenity::model::Timestamp,
}

impl DiscordMessage {
  pub fn from_serenity(msg: &serenity::all::Message) -> Self {
    DiscordMessage {
      id: Snowflake { content: msg.id.get() },
      author: DiscordMessageAuthor {
        display_name: DisplayName(msg.author.name.clone()),
        icon: msg.author.avatar_url().unwrap_or(msg.author.default_avatar_url()),
        id: msg.author.id.to_string(),
      },
      content: DiscordMessageContent {
        content: msg.content.clone(),
        is_pending: false,
      },
      nonce: msg.nonce.clone().map(|n| match n {
        Nonce::Number(n) => n.to_string(),
        Nonce::String(s) => s,
      }),
      creation_time: msg.timestamp,
    }
  }
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
  type Identifier = Snowflake;

  fn get_list_identifier(&self) -> Self::Identifier {
    self.id
  }
}
