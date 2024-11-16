use std::sync::Arc;

use scope_chat::channel::Channel;
use tokio::sync::{broadcast, RwLock};

use crate::{client::DiscordClient, message::DiscordMessage, snowflake::Snowflake};

pub struct DiscordChannel {
  channel_id: Snowflake,
  receiver: broadcast::Receiver<DiscordMessage>,
}

impl DiscordChannel {
  pub async fn new(client: &mut Arc<RwLock<DiscordClient>>, channel_id: Snowflake) -> Self {
    let (sender, receiver) = broadcast::channel(10);

    DiscordClient::add_channel_message_sender(client, channel_id, sender).await;

    DiscordChannel { channel_id, receiver }
  }
}

impl Channel for DiscordChannel {
  type Message = DiscordMessage;

  fn get_receiver(&self) -> broadcast::Receiver<Self::Message> {
    self.receiver.resubscribe()
  }
}