use std::sync::{Arc, OnceLock};

use chrono::Utc;
use scope_backend_cache::async_list::{refcacheslice::Exists, AsyncListCache};
use scope_chat::reaction::ReactionEvent;
use scope_chat::{
  async_list::{AsyncList, AsyncListIndex, AsyncListItem, AsyncListResult},
  channel::Channel,
};
use serenity::all::{ChannelId, GetMessages, MessageId};
use tokio::sync::{broadcast, Mutex, Semaphore};

use crate::{
  client::DiscordClient,
  message::{DiscordMessage, DiscordMessageData},
  snowflake::Snowflake,
};

pub struct DiscordChannel {
  channel: Arc<serenity::model::channel::Channel>,
  message_receiver: broadcast::Receiver<DiscordMessage>,
  reaction_receiver: broadcast::Receiver<ReactionEvent<Snowflake>>,
  client: Arc<DiscordClient>,
  cache: Arc<Mutex<AsyncListCache<DiscordMessage>>>,
  blocker: Semaphore,
}

impl DiscordChannel {
  pub(crate) async fn new(client: Arc<DiscordClient>, channel_id: ChannelId) -> Self {
    let channel = Arc::new(channel_id.to_channel(client.discord()).await.unwrap());
    let (message_sender, message_receiver) = broadcast::channel(10);
    client.add_channel_message_sender(channel_id, message_sender).await;

    let (reaction_sender, reaction_receiver) = broadcast::channel(10);
    client.add_channel_reaction_sender(channel_id, reaction_sender).await;

    DiscordChannel {
      channel,
      message_receiver,
      reaction_receiver,
      client,
      cache: Arc::new(Mutex::new(AsyncListCache::new())),
      blocker: Semaphore::new(1),
    }
  }
}

impl Channel for DiscordChannel {
  type Message = DiscordMessage;
  type Identifier = Snowflake;

  fn get_message_receiver(&self) -> broadcast::Receiver<Self::Message> {
    self.message_receiver.resubscribe()
  }

  fn get_reaction_receiver(&self) -> broadcast::Receiver<ReactionEvent<Snowflake>> {
    self.reaction_receiver.resubscribe()
  }

  fn send_message(&self, content: String, nonce: String) -> DiscordMessage {
    let client = self.client.clone();
    let channel_id = self.channel.id();
    let sent_content = content.clone();
    let sent_nonce = nonce.clone();

    tokio::spawn(async move {
      client.send_message(channel_id, sent_content, sent_nonce).await;
    });

    DiscordMessage {
      channel: self.channel.clone(),
      client: self.client.clone(),
      data: DiscordMessageData::Pending {
        nonce,
        content,
        sent_time: Utc::now(),
        list_item_id: Snowflake::random(),
        reactions: Default::default(),
      },
      content: OnceLock::new(),
    }
  }

  fn get_identifier(&self) -> Self::Identifier {
    self.channel.id().into()
  }
}

const DISCORD_MESSAGE_BATCH_SIZE: u8 = 50;

impl AsyncList for DiscordChannel {
  async fn bounded_at_bottom_by(&self) -> Option<Snowflake> {
    let lock = self.cache.lock().await;
    let cache_value = lock.bounded_at_top_by();

    if let Some(v) = cache_value {
      return Some(v);
    };

    match &*self.channel {
      serenity::model::channel::Channel::Guild(guild_channel) => guild_channel.messages(self.client.discord(), GetMessages::new().limit(1)).await,
      serenity::model::channel::Channel::Private(private_channel) => {
        private_channel.messages(self.client.discord(), GetMessages::new().limit(1)).await
      }
      _ => unimplemented!(),
    }
    .unwrap()
    .first()
    .map(|v| Snowflake(v.id.get()))
  }

  async fn bounded_at_top_by(&self) -> Option<Snowflake> {
    let lock = self.cache.lock().await;
    let cache_value = lock.bounded_at_bottom_by();

    if let Some(v) = cache_value {
      return Some(v);
    };

    panic!("Unsupported")
  }

  async fn find(&self, identifier: &Snowflake) -> Option<Self::Content> {
    let lock = self.cache.lock().await;
    let cache_value = lock.find(identifier);

    drop(lock);

    if let Some(v) = cache_value {
      return Some(v);
    }

    let result = self.client.get_specific_message(self.channel.id(), MessageId::new(identifier.0)).await?;

    Some(DiscordMessage::load_serenity(self.client.clone(), Arc::new(result)).await)
  }

  async fn get(&self, index: AsyncListIndex<Snowflake>) -> Option<AsyncListResult<Self::Content>> {
    let permit = self.blocker.acquire().await;
    let mut lock = self.cache.lock().await;
    let cache_value = lock.get(index);

    if let Exists::Yes(v) = cache_value {
      return Some(v);
    } else if let Exists::No = cache_value {
      return None;
    }

    let mut result: Option<DiscordMessage> = None;
    let mut is_top = false;
    let mut is_bottom = false;

    match index {
      AsyncListIndex::RelativeToTop(_) => todo!("Unsupported"),
      AsyncListIndex::RelativeToBottom(index) => {
        if index != 0 {
          unimplemented!()
        }

        let v = self.client.get_messages(self.channel.id(), GetMessages::new().limit(DISCORD_MESSAGE_BATCH_SIZE)).await;

        let is_end = v.len() == DISCORD_MESSAGE_BATCH_SIZE as usize;
        is_bottom = true;
        is_top = v.len() == 1;

        let mut iter = v.into_iter();

        let v = iter.next();

        if let Some(v) = v {
          let msg = DiscordMessage::load_serenity(self.client.clone(), Arc::new(v)).await;
          let mut id = msg.get_list_identifier();
          lock.append_bottom(msg.clone());
          result = Some(msg);

          for message in iter {
            let msg = DiscordMessage::load_serenity(self.client.clone(), Arc::new(message)).await;
            let nid = msg.get_list_identifier();

            lock.insert(AsyncListIndex::Before(id), msg, false, is_end);

            id = nid;
          }
        };
      }
      AsyncListIndex::After(message) => {
        // NEWEST first
        let v = self
          .client
          .get_messages(
            self.channel.id(),
            GetMessages::new().after(MessageId::new(message.0)).limit(DISCORD_MESSAGE_BATCH_SIZE),
          )
          .await;
        let mut current_index: Snowflake = message;

        let is_end = v.len() == DISCORD_MESSAGE_BATCH_SIZE as usize;
        let len = v.len();
        is_bottom = is_end && v.len() == 1;

        for (message, index) in v.into_iter().rev().zip(0..) {
          let id = Snowflake(message.id.get());

          let value = DiscordMessage::load_serenity(self.client.clone(), Arc::new(message)).await;

          if index == 0 {
            result = Some(value.clone());
          }

          lock.insert(AsyncListIndex::After(current_index), value, false, is_end && index == (len - 1));

          current_index = id;
        }
      }
      AsyncListIndex::Before(message) => {
        let v = self
          .client
          .get_messages(
            self.channel.id(),
            GetMessages::new().before(MessageId::new(message.0)).limit(DISCORD_MESSAGE_BATCH_SIZE),
          )
          .await;
        let mut current_index: Snowflake = message;

        println!("Discord gave us {:?} messages (out of {:?})", v.len(), DISCORD_MESSAGE_BATCH_SIZE);

        let is_end = v.len() == DISCORD_MESSAGE_BATCH_SIZE as usize;
        let len = v.len();
        is_top = is_end && v.len() == 1;

        result = None;

        for (message, index) in v.into_iter().zip(0..) {
          let id = Snowflake(message.id.get());

          let value = DiscordMessage::load_serenity(self.client.clone(), Arc::new(message)).await;

          if index == 0 {
            result = Some(value.clone());
          }

          lock.insert(AsyncListIndex::Before(current_index), value, false, is_end && index == len);

          current_index = id;
        }
      }
    };

    drop(permit);
    drop(lock);

    result.map(|v| AsyncListResult {
      content: v,
      is_top,
      is_bottom,
    })
  }

  type Content = DiscordMessage;
}

impl Clone for DiscordChannel {
  fn clone(&self) -> Self {
    Self {
      channel: self.channel.clone(),
      message_receiver: self.message_receiver.resubscribe(),
      reaction_receiver: self.reaction_receiver.resubscribe(),
      client: self.client.clone(),
      cache: self.cache.clone(),
      blocker: Semaphore::new(1),
    }
  }
}
