use std::{
  collections::HashMap, rc::Rc, sync::{Arc, OnceLock}
};

use serenity::{
  all::{Context, EventHandler, GatewayIntents, Message},
  async_trait,
  futures::SinkExt,
};
use tokio::sync::{broadcast, Mutex, RwLock};

use crate::{
  message::{
    author::{DiscordMessageAuthor, DisplayName},
    content::DiscordMessageContent,
    DiscordMessage,
  },
  snowflake::{self, Snowflake},
};

#[derive(Default)]
pub struct DiscordClient {
  channel_message_event_handlers: RwLock<HashMap<Snowflake, Vec<broadcast::Sender<DiscordMessage>>>>,
  client: OnceLock<serenity::Client>,
}

impl DiscordClient {
  pub async fn new(token: String) -> Arc<DiscordClient> {
    let client = Arc::new(DiscordClient::default());

    let mut discord = serenity::Client::builder(token, GatewayIntents::all()).event_handler_arc(client.clone()).await.expect("Error creating client");

    if let Err(why) = discord.start().await {
      panic!("Client error: {why:?}");
    }

    let _ = client.client.set(discord);

    client
  }

  fn discord(&self) -> &serenity::Client {
    self.client.get().unwrap()
  }

  pub async fn add_channel_message_sender(&self, channel: Snowflake, sender: broadcast::Sender<DiscordMessage>) {
    self.channel_message_event_handlers.write().await.entry(channel).or_default().push(sender);
  }

  pub async fn send_message(&self, channel_id: Snowflake, content: String, nonce: String) {
    println!("All the way to discord~! {:?} {:?}", channel_id, content);
    ChannelId::new(channel_id.content)
      .send_message(
        self.discord().http.clone(),
        CreateMessage::new().content(content).enforce_nonce(true).nonce(serenity::all::Nonce::String(nonce)),
      )
      .await
      .unwrap();
  }
}

#[async_trait]
impl EventHandler for DiscordClient {
  async fn ready(&self, _: Context, data_about_bot: serenity::model::prelude::Ready) {
    println!("Ready! {:?}", data_about_bot);
  }

  async fn message(&self, _: Context, msg: Message) {
    println!("Got message: {:?} {:?}", msg.channel_id, msg.content);

    let snowflake = Snowflake {
      content: msg.channel_id.get(),
    };

    if let Some(vec) = self.channel_message_event_handlers.read().await.get(&snowflake) {
      for sender in vec {
        println!("Sending to sender!");

        let _ = sender.send(DiscordMessage {
          id: snowflake,
          author: DiscordMessageAuthor {
            display_name: DisplayName(msg.author.name.clone()),
            icon: msg.author.avatar_url().unwrap_or(msg.author.default_avatar_url()),
          },
          content: DiscordMessageContent {
            content: msg.content.clone(),
            is_pending: false,
          },
          nonce: msg.nonce.clone().map(|n| match n {
            Nonce::Number(n) => n.to_string(),
            Nonce::String(s) => s,
          }),
        });
      }
    }
  }
}
