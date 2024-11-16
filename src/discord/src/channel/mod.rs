use std::sync::Arc;

use scope_chat::channel::Channel;
use tokio::sync::{broadcast, RwLock};

use crate::{client::DiscordClient, message::DiscordMessage, snowflake::Snowflake};

pub struct DiscordChannel {
  channel_id: Snowflake,
  receiver: broadcast::Receiver<DiscordMessage>,
  client: Arc<RwLock<DiscordClient>>
}

impl DiscordChannel {
  pub async fn new(client: Arc<RwLock<DiscordClient>>, channel_id: Snowflake) -> Self {
    let (sender, receiver) = broadcast::channel(10);

    client.write().await.add_channel_message_sender(channel_id, sender).await;

    DiscordChannel { channel_id, receiver, client }
  }
}

impl Channel for DiscordChannel {
  type Message = DiscordMessage;

  fn get_receiver(&self) -> broadcast::Receiver<Self::Message> {
    self.receiver.resubscribe()
  }

  async fn send_message(&self, content: String, nonce: String) {
    println!("Sending: {:?}", content);

    self.client.write().await.send_message(self.channel_id, content, nonce).await;
  }
}

impl Clone for DiscordChannel {
  fn clone(&self) -> Self {
    Self {
      channel_id: self.channel_id.clone(),
      receiver: self.receiver.resubscribe(),
      client: self.client.clone()
    }
  }
}
