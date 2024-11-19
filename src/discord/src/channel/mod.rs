use std::sync::Arc;

use scope_backend_cache::async_list::{refcacheslice::Exists, AsyncListCache};
use scope_chat::{
  async_list::{AsyncList, AsyncListIndex, AsyncListItem, AsyncListResult},
  channel::Channel,
};
use serenity::all::{GetMessages, MessageId, Timestamp};
use tokio::sync::{broadcast, Mutex, Semaphore};

use crate::{
  client::DiscordClient,
  message::{content::DiscordMessageContent, DiscordMessage},
  snowflake::Snowflake,
};

pub struct DiscordChannel {
  channel_id: Snowflake,
  receiver: broadcast::Receiver<DiscordMessage>,
  client: Arc<DiscordClient>,
  cache: Arc<Mutex<AsyncListCache<DiscordMessage>>>,
  blocker: Semaphore,
}

impl DiscordChannel {
  pub async fn new(client: Arc<DiscordClient>, channel_id: Snowflake) -> Self {
    let (sender, receiver) = broadcast::channel(10);

    client.add_channel_message_sender(channel_id, sender).await;

    DiscordChannel {
      channel_id,
      receiver,
      client,
      cache: Arc::new(Mutex::new(AsyncListCache::new())),
      blocker: Semaphore::new(1),
    }
  }
}

impl Channel for DiscordChannel {
  type Message = DiscordMessage;

  fn get_receiver(&self) -> broadcast::Receiver<Self::Message> {
    self.receiver.resubscribe()
  }

  fn send_message(&self, content: String, nonce: String) -> DiscordMessage {
    let client = self.client.clone();
    let channel_id = self.channel_id;
    let sent_content = content.clone();
    let sent_nonce = nonce.clone();

    tokio::spawn(async move {
      client.send_message(channel_id, sent_content, sent_nonce).await;
    });

    DiscordMessage {
      content: DiscordMessageContent { content, is_pending: true },
      author: self.client.user().clone(),
      id: Snowflake { content: 0 },
      nonce: Some(nonce),
      creation_time: Timestamp::now(),
    }
  }
}

const DISCORD_MESSAGE_BATCH_SIZE: u8 = 5;

impl AsyncList for DiscordChannel {
  async fn bounded_at_bottom_by(&self) -> Option<Snowflake> {
    let lock = self.cache.lock().await;
    let cache_value = lock.bounded_at_top_by();

    if let Some(v) = cache_value {
      return Some(v);
    };

    self.client.get_messages(self.channel_id, GetMessages::new().limit(1)).await.first().map(|v| Snowflake { content: v.id.get() })
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
    let permit = self.blocker.acquire().await;

    let lock = self.cache.lock().await;
    let cache_value = lock.find(identifier);

    if let Some(v) = cache_value {
      return Some(v);
    }

    let result = self.client.get_specific_message(self.channel_id, *identifier).await.map(|v| DiscordMessage::from_serenity(&v));

    drop(permit);

    result
  }

  async fn get(&self, index: AsyncListIndex<Snowflake>) -> Option<AsyncListResult<Self::Content>> {
    let permit = self.blocker.acquire().await;

    let mut lock = self.cache.lock().await;
    let cache_value = lock.get(index.clone());

    if let Exists::Yes(v) = cache_value {
      return Some(v);
    } else if let Exists::No = cache_value {
      return None;
    }

    let result: Option<DiscordMessage>;
    let mut is_top = false;
    let mut is_bottom = false;

    match index {
      AsyncListIndex::RelativeToTop(_) => todo!("Unsupported"),
      AsyncListIndex::RelativeToBottom(index) => {
        if index != 0 {
          unimplemented!()
        }

        let v = self.client.get_messages(self.channel_id, GetMessages::new().limit(DISCORD_MESSAGE_BATCH_SIZE)).await;

        let is_end = v.len() == DISCORD_MESSAGE_BATCH_SIZE as usize;
        is_bottom = true;
        is_top = v.len() == 1;

        result = v.first().map(DiscordMessage::from_serenity);

        let mut iter = v.iter();

        let v = iter.next();

        if let Some(v) = v {
          let msg = DiscordMessage::from_serenity(v);
          let mut id = msg.get_list_identifier();
          lock.append_bottom(msg);

          for message in iter {
            let msg = DiscordMessage::from_serenity(&message);
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
            self.channel_id,
            GetMessages::new().after(MessageId::new(message.content)).limit(DISCORD_MESSAGE_BATCH_SIZE),
          )
          .await;
        let mut current_index: Snowflake = message;

        let is_end = v.len() == DISCORD_MESSAGE_BATCH_SIZE as usize;
        is_bottom = is_end;

        result = v.last().map(DiscordMessage::from_serenity);

        for message in v.iter().rev() {
          lock.insert(
            AsyncListIndex::After(current_index),
            DiscordMessage::from_serenity(message),
            false,
            is_end,
          );

          current_index = Snowflake { content: message.id.get() }
        }
      }
      AsyncListIndex::Before(message) => {
        let v = self
          .client
          .get_messages(
            self.channel_id,
            GetMessages::new().before(MessageId::new(message.content)).limit(DISCORD_MESSAGE_BATCH_SIZE),
          )
          .await;
        let mut current_index: Snowflake = message;

        let is_end = v.len() == DISCORD_MESSAGE_BATCH_SIZE as usize;
        is_top = is_end;

        result = v.first().map(DiscordMessage::from_serenity);

        for message in v {
          lock.insert(
            AsyncListIndex::Before(current_index),
            DiscordMessage::from_serenity(&message),
            false,
            is_end,
          );

          current_index = Snowflake { content: message.id.get() }
        }
      }
    };

    drop(permit);

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
      channel_id: self.channel_id,
      receiver: self.receiver.resubscribe(),
      client: self.client.clone(),
      cache: self.cache.clone(),
      blocker: Semaphore::new(1),
    }
  }
}
