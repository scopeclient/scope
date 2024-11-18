use std::sync::Arc;

use scope_chat::{
  async_list::{AsyncList, AsyncListIndex, AsyncListItem, AsyncListResult},
  channel::Channel,
};
use serenity::all::{ChannelId, GetMessages, Timestamp};
use tokio::sync::broadcast;

use crate::{
  client::DiscordClient,
  message::{content::DiscordMessageContent, DiscordMessage},
  snowflake::Snowflake,
};

pub struct DiscordChannel {
  channel_id: Snowflake,
  receiver: broadcast::Receiver<DiscordMessage>,
  client: Arc<DiscordClient>,
}

impl DiscordChannel {
  pub async fn new(client: Arc<DiscordClient>, channel_id: Snowflake) -> Self {
    let (sender, receiver) = broadcast::channel(10);

    client.add_channel_message_sender(channel_id, sender).await;

    DiscordChannel {
      channel_id,
      receiver,
      client,
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
  async fn bounded_at_bottom_by(&self) -> Option<<Self::Content as AsyncListItem>::Identifier> {
    unimplemented!()
  }

  async fn bounded_at_top_by(&self) -> Option<<Self::Content as AsyncListItem>::Identifier> {
    unimplemented!()
  }

  async fn find(&self, identifier: &<Self::Content as AsyncListItem>::Identifier) -> Option<AsyncListResult<Self::Content>> {
    unimplemented!()
  }

  async fn get(&self, index: AsyncListIndex<<Self::Content as AsyncListItem>::Identifier>) -> Option<AsyncListResult<Self::Content>> {
    unimplemented!()
  }

  type Content = DiscordMessage;
}

impl Clone for DiscordChannel {
  fn clone(&self) -> Self {
    Self {
      channel_id: self.channel_id,
      receiver: self.receiver.resubscribe(),
      client: self.client.clone(),
    }
  }
}
