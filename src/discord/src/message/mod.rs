use std::sync::Arc;

use author::DiscordMessageAuthor;
use chrono::{DateTime, Utc};
use gpui::{App, Entity, Window};
use scope_chat::{async_list::AsyncListItem, message::Message};
use scope_rich::{ContentCell, MessageKind, ReplyRef, RichContentView, RichMessage};
use serenity::all::Nonce;

use crate::{client::DiscordClient, snowflake::Snowflake};

pub mod author;
pub mod rich;

#[derive(Clone)]
pub enum DiscordMessageData {
  /// Sent by us and not echoed back by the gateway yet.
  Pending {
    nonce: String,
    content: String,
    sent_time: DateTime<Utc>,
    list_item_id: Snowflake,
    /// The message this one replies to, so the reply header shows straight away.
    reply_to: Option<ReplyRef>,
  },
  Received(Arc<serenity::model::channel::Message>, Option<Arc<serenity::model::guild::Member>>),
}

#[derive(Clone)]
pub struct DiscordMessage {
  pub client: Arc<DiscordClient>,
  pub channel: Arc<serenity::model::channel::Channel>,
  pub data: DiscordMessageData,
  pub content: ContentCell,
}

impl DiscordMessage {
  /// A message fetched over REST for `channel`.
  pub async fn load_serenity(
    client: Arc<DiscordClient>,
    channel: Arc<serenity::model::channel::Channel>,
    msg: Arc<serenity::model::channel::Message>,
  ) -> Self {
    let member = client.message_member(&msg).await;

    Self {
      client,
      channel,
      data: DiscordMessageData::Received(msg, member),

      content: ContentCell::new(),
    }
  }

  pub fn from_serenity(
    client: Arc<DiscordClient>,
    msg: Arc<serenity::model::channel::Message>,
    channel: Arc<serenity::model::channel::Channel>,
    member: Option<Arc<serenity::model::guild::Member>>,
  ) -> Self {
    Self {
      client,
      channel,
      data: DiscordMessageData::Received(msg, member),

      content: ContentCell::new(),
    }
  }

  /// The optimistic copy of a message we just sent to `channel`.
  pub fn pending(
    client: Arc<DiscordClient>,
    channel: Arc<serenity::model::channel::Channel>,
    content: String,
    nonce: String,
    reply_to: Option<ReplyRef>,
  ) -> Self {
    Self {
      client,
      channel,
      data: DiscordMessageData::Pending {
        nonce,
        content,
        sent_time: Utc::now(),
        list_item_id: Snowflake::random(),
        reply_to,
      },
      content: ContentCell::new(),
    }
  }

  /// The serenity message behind a received message; `None` while pending.
  pub(crate) fn serenity(&self) -> Option<&Arc<serenity::model::channel::Message>> {
    match &self.data {
      DiscordMessageData::Received(message, _) => Some(message),
      DiscordMessageData::Pending { .. } => None,
    }
  }

  /// This message with `msg` as its new data (an edit, reactions, resolved embeds, …).
  ///
  /// The author stays as loaded; the content is rendered afresh.
  pub(crate) fn with_serenity(&self, msg: Arc<serenity::model::channel::Message>) -> Self {
    let member = match &self.data {
      DiscordMessageData::Received(_, member) => member.clone(),
      DiscordMessageData::Pending { .. } => None,
    };

    Self {
      client: self.client.clone(),
      channel: self.channel.clone(),
      data: DiscordMessageData::Received(msg, member),
      content: ContentCell::new(),
    }
  }
}

enum NonceState<'r> {
  Fixed(&'r String),
  Discord(&'r Option<Nonce>),
}

impl<'r> PartialEq for NonceState<'r> {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      // comparing anything with `None` means they are not equal
      (NonceState::Discord(None), _) => false,
      (_, NonceState::Discord(None)) => false,

      // two Fixed strings are equal if their contents are
      (NonceState::Fixed(left), NonceState::Fixed(right)) => left == right,

      // Fixed strings are only equal to Discord String Nonces
      (NonceState::Fixed(left), NonceState::Discord(Some(Nonce::String(right)))) => *left == right,
      (NonceState::Discord(Some(Nonce::String(right))), NonceState::Fixed(left)) => *left == right,

      // Discord Nonces are only equal if their types are.
      (NonceState::Discord(Some(Nonce::Number(left))), NonceState::Discord(Some(Nonce::Number(right)))) => left == right,
      (NonceState::Discord(Some(Nonce::String(left))), NonceState::Discord(Some(Nonce::String(right)))) => left == right,

      _ => false,
    }
  }
}

impl Message for DiscordMessage {
  type Identifier = Snowflake;
  type Author = DiscordMessageAuthor;

  fn is_own(&self) -> bool {
    match &self.data {
      DiscordMessageData::Pending { .. } => true,
      DiscordMessageData::Received(message, _) => message.author.id == self.client.own_user().id,
    }
  }

  fn get_author(&self) -> DiscordMessageAuthor {
    match &self.data {
      DiscordMessageData::Pending { .. } => DiscordMessageAuthor {
        client: self.client.clone(),
        data: match &*self.channel {
          serenity::model::channel::Channel::Private(_) => author::DiscordMessageAuthorData::User(self.client.own_user().clone()),
          serenity::model::channel::Channel::Guild(guild_channel) => match self.client.own_member(guild_channel.guild_id) {
            Some(member) => author::DiscordMessageAuthorData::Member(member),
            None => author::DiscordMessageAuthorData::User(self.client.own_user().clone()),
          },
          _ => unimplemented!(),
        },
      },

      DiscordMessageData::Received(message, member) => DiscordMessageAuthor {
        client: self.client.clone(),
        data: match member {
          None => author::DiscordMessageAuthorData::NonMemberAuthor(message.clone()),
          Some(member) => author::DiscordMessageAuthorData::Member(member.clone()),
        },
      },
    }
  }

  // TODO: want reviewer discussion. I'm really stretching the abilities of gpui here and im not sure if this is the right way to do this.
  fn get_content(&self, _window: &mut Window, cx: &mut App) -> Entity<RichContentView> {
    self.content.get_or_create(cx, || {
      Arc::new(match &self.data {
        DiscordMessageData::Pending { content, reply_to, .. } => {
          let mut rich = RichMessage::pending(content.clone());

          if let Some(reply) = reply_to {
            rich.kind = MessageKind::Reply;
            rich.reply = Some(reply.clone());
          }

          rich
        }
        DiscordMessageData::Received(message, member) => rich::from_serenity(message, member.as_deref(), &self.channel, &self.client),
      })
    })
  }

  fn get_identifier(&self) -> Option<Snowflake> {
    match &self.data {
      DiscordMessageData::Received(message, _) => Some(message.id.into()),
      DiscordMessageData::Pending { .. } => None,
    }
  }

  fn get_nonce(&self) -> impl PartialEq {
    match &self.data {
      DiscordMessageData::Pending { nonce, .. } => NonceState::Fixed(nonce),
      DiscordMessageData::Received(message, _) => NonceState::Discord(&message.nonce),
    }
  }

  fn should_group(&self, previous: &Self) -> bool {
    const MAX_DISCORD_MESSAGE_GAP_SECS_FOR_GROUP: i64 = 5 * 60;

    let left = self.get_timestamp().unwrap();
    let right = previous.get_timestamp().unwrap();

    left.signed_duration_since(right).num_seconds() <= MAX_DISCORD_MESSAGE_GAP_SECS_FOR_GROUP
  }

  fn get_timestamp(&self) -> Option<DateTime<Utc>> {
    match &self.data {
      DiscordMessageData::Pending { sent_time, .. } => Some(*sent_time),
      DiscordMessageData::Received(message, _) => DateTime::from_timestamp_millis(message.timestamp.timestamp_millis()),
    }
  }
}

impl AsyncListItem for DiscordMessage {
  type Identifier = Snowflake;

  fn get_list_identifier(&self) -> Self::Identifier {
    match &self.data {
      DiscordMessageData::Pending { list_item_id, .. } => *list_item_id,
      DiscordMessageData::Received(message, _) => message.id.into(),
    }
  }
}
