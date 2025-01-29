use std::sync::{Arc, OnceLock};

use crate::message::reaction_list::DiscordReactionList;
use author::DiscordMessageAuthor;
use chrono::{DateTime, Utc};
use content::DiscordMessageContent;
use gpui::{App, AppContext, Entity};
use scope_chat::reaction::ReactionList;
use scope_chat::{async_list::AsyncListItem, message::Message};
use serenity::all::{ModelError, Nonce};

use crate::{client::DiscordClient, snowflake::Snowflake};

pub mod author;
pub mod content;
pub mod reaction;
pub mod reaction_list;

#[derive(Clone)]
pub enum DiscordMessageData {
  Pending {
    nonce: String,
    content: String,
    sent_time: DateTime<Utc>,
    list_item_id: Snowflake,
  },
  Received(
    Arc<serenity::model::channel::Message>,
    Option<Arc<serenity::model::guild::Member>>,
    DiscordReactionList,
  ),
}

#[derive(Clone)]
pub struct DiscordMessage {
  pub client: Arc<DiscordClient>,
  pub channel: Arc<serenity::model::channel::Channel>,
  pub data: DiscordMessageData,
  pub content: Arc<OnceLock<Entity<DiscordMessageContent>>>,
}

impl DiscordMessage {
  pub async fn load_serenity(client: Arc<DiscordClient>, msg: Arc<serenity::model::channel::Message>) -> Self {
    let channel = Arc::new(msg.channel(client.discord()).await.unwrap());
    let member = match msg.member(client.discord()).await {
      Ok(v) => Ok(Some(Arc::new(v))),
      Err(serenity::Error::Model(ModelError::ItemMissing)) => Ok(None),
      Err(e) => Err(e),
    }
    .unwrap();

    let reactions = DiscordReactionList::new(msg.reactions.clone(), channel.id(), msg.id, client.clone());

    Self {
      client,
      channel,
      data: DiscordMessageData::Received(msg, member, reactions),
      content: Arc::new(OnceLock::new()),
    }
  }

  pub fn from_serenity(
    client: Arc<DiscordClient>,
    msg: Arc<serenity::model::channel::Message>,
    channel: Arc<serenity::model::channel::Channel>,
    member: Option<Arc<serenity::model::guild::Member>>,
  ) -> Self {
    let reactions = DiscordReactionList::new(msg.reactions.clone(), channel.id(), msg.id, client.clone());
    Self {
      client,
      channel,
      data: DiscordMessageData::Received(msg, member, reactions),
      content: Arc::new(OnceLock::new()),
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
  type Content = DiscordMessageContent;

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

      DiscordMessageData::Received(message, member, ..) => DiscordMessageAuthor {
        client: self.client.clone(),
        data: match member {
          None => author::DiscordMessageAuthorData::NonMemberAuthor(message.clone()),
          Some(member) => author::DiscordMessageAuthorData::Member(member.clone()),
        },
      },
    }
  }

  // TODO: want reviewer discussion. I'm really stretching the abilities of gpui here and im not sure if this is the right way to do this.
  // Additional Context to this discussion: the OnceLock in context CANNOT be cloned because it messes with the internal gpui entityids
  // and recreates the entity on every interaction making it impossible to interact with the message content in any way. Right now, I'm
  // using an arc for this, but this feels like a band-aid solution. I do agree that this should be refactored out at some point.
  fn get_content(&self, cx: &mut App) -> Entity<Self::Content> {
    self
      .content
      .get_or_init(|| {
        cx.new(|_| match &self.data {
          DiscordMessageData::Pending { content, .. } => DiscordMessageContent::pending(content.clone()),
          DiscordMessageData::Received(message, _, reactions) => DiscordMessageContent::received(message, reactions),
        })
      }).clone()
  }

  fn get_identifier(&self) -> Option<Snowflake> {
    match &self.data {
      DiscordMessageData::Received(message, ..) => Some(message.id.into()),
      DiscordMessageData::Pending { .. } => None,
    }
  }

  fn get_nonce(&self) -> impl PartialEq {
    match &self.data {
      DiscordMessageData::Pending { nonce, .. } => NonceState::Fixed(nonce),
      DiscordMessageData::Received(message, ..) => NonceState::Discord(&message.nonce),
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
      DiscordMessageData::Received(message, ..) => DateTime::from_timestamp_millis(message.timestamp.timestamp_millis()),
    }
  }

  fn get_reactions(&mut self) -> Option<&mut impl ReactionList> {
    match &mut self.data {
      DiscordMessageData::Pending { .. } => None,
      DiscordMessageData::Received(_, _, reactions) => Some(reactions),
    }
  }
}

impl AsyncListItem for DiscordMessage {
  type Identifier = Snowflake;

  fn get_list_identifier(&self) -> Self::Identifier {
    match &self.data {
      DiscordMessageData::Pending { list_item_id, .. } => *list_item_id,
      DiscordMessageData::Received(message, ..) => message.id.into(),
    }
  }
}
