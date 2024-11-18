use std::sync::Arc;

use scope_backend_cache::async_list::{refcacheslice::Exists, AsyncListCache};
use scope_chat::{
  async_list::{AsyncList, AsyncListIndex, AsyncListResult},
  channel::Channel,
};
use serenity::all::{GetMessages, MessageId, Timestamp};
use tokio::sync::{broadcast, Mutex};

use crate::{
  client::DiscordClient,
  message::{content::DiscordMessageContent, DiscordMessage},
  snowflake::Snowflake,
};

pub struct DiscordChannel {
  channel_id: Snowflake,
  receiver: broadcast::Receiver<DiscordMessage>,
  client: Arc<DiscordClient>,
  cache: Arc<Mutex<AsyncListCache<DiscordChannel>>>,
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
    let lock = self.cache.lock().await;
    let cache_value = lock.find(identifier);

    if let Some(v) = cache_value {
      return Some(v);
    }

    self.client.get_specific_message(self.channel_id, *identifier).await.map(|v| DiscordMessage::from_serenity(&v))
  }

  async fn get(&self, index: AsyncListIndex<Snowflake>) -> Option<AsyncListResult<Self::Content>> {
    let mut lock = self.cache.lock().await;
    let cache_value = lock.get(index.clone());

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
      AsyncListIndex::RelativeToBottom(index) => todo!("TODO :3 have fun!!"),
      AsyncListIndex::After(message) => {
        // NEWEST first
        let v = self.client.get_messages(self.channel_id, GetMessages::new().after(MessageId::new(message.content)).limit(50)).await;
        let mut lock = self.cache.lock().await;
        let mut current_index: Snowflake = message;

        let is_end = v.len() == 50;
        is_bottom = is_end;

        result = Some(DiscordMessage::from_serenity(v.get(0).unwrap()));

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
        let v = self.client.get_messages(self.channel_id, GetMessages::new().after(MessageId::new(message.content)).limit(50)).await;
        let mut lock = self.cache.lock().await;
        let mut current_index: Snowflake = message;

        let is_end = v.len() == 50;
        is_top = is_end;

        result = Some(DiscordMessage::from_serenity(v.get(0).unwrap()));

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
    }
  }
}
