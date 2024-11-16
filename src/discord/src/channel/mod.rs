use std::sync::Arc;

use scope_chat::channel::Channel;
use tokio::sync::{broadcast, RwLock};

use crate::{client::DiscordClient, message::{author::{DiscordMessageAuthor, DisplayName}, content::DiscordMessageContent, DiscordMessage}, snowflake::Snowflake};

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

  fn send_message(&self, content: String, nonce: String) -> DiscordMessage {
    let client = self.client.clone();
    let channel_id = self.channel_id;
    let sent_content = content.clone();
    let sent_nonce = nonce.clone();

    tokio::spawn(async move {
      client.write().await.send_message(channel_id, sent_content, sent_nonce).await;
    });

    DiscordMessage {
      content: DiscordMessageContent { content, is_pending: true },
      author: DiscordMessageAuthor { display_name: DisplayName("Pending".to_owned()), icon: "".to_owned() },
      id: Snowflake { content: 0 },
      nonce: Some(nonce),
    }
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
