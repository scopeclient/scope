use std::{collections::HashMap, rc::Rc, sync::Arc};

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
  channel_message_event_handlers: HashMap<Snowflake, Vec<broadcast::Sender<DiscordMessage>>>,
  client: Option<serenity::Client>
}

impl DiscordClient {
  pub fn new(token: String) -> Arc<RwLock<DiscordClient>> {
    let client = Arc::new(RwLock::new(DiscordClient::default()));
    let remote = RemoteDiscordClient(client.clone());
    let async_client = client.clone();

    tokio::spawn(async move {
      let mut client = serenity::Client::builder(token, GatewayIntents::all())
        .event_handler(remote)
        .await
        .expect("Error creating client");

      if let Err(why) = client.start().await {
        panic!("Client error: {why:?}");
      }

      async_client.write().await.client = Some(client);
    });

    client
  }

  pub async fn add_channel_message_sender(&mut self, channel: Snowflake, sender: broadcast::Sender<DiscordMessage>) {
    self.channel_message_event_handlers.entry(channel).or_default().push(sender);
  }

  pub async fn send_message(&mut self, channel_id: Snowflake, content: String, nonce: String) {
    println!("All the way to discord~! {:?} {:?}", channel_id, content);
    ChannelId::new(channel_id.content).send_message(self.client.as_ref().unwrap().http.clone(), CreateMessage::new().content(content).enforce_nonce(true).nonce(serenity::all::Nonce::String(nonce))).await.unwrap();
  }
}

struct RemoteDiscordClient(Arc<RwLock<DiscordClient>>);

#[async_trait]
impl EventHandler for RemoteDiscordClient {
  async fn ready(&self, ctx: Context, data_about_bot: serenity::model::prelude::Ready) {
    println!("Ready! {:?}", data_about_bot);
  }

  async fn message(&self, _: Context, msg: Message) {
    println!("Got message: {:?} {:?}", msg.channel_id, msg.content);

    let snowflake = Snowflake {
      content: msg.channel_id.get(),
    };

    if let Some(vec) = self.0.read().await.channel_message_event_handlers.get(&snowflake) {
      for sender in vec {
        println!("Sending to sender!");

        let _ = sender.send(DiscordMessage {
          id: snowflake,
          author: DiscordMessageAuthor {
            display_name: DisplayName(msg.author.name.clone()),
            icon: msg.author.avatar_url().unwrap_or(msg.author.default_avatar_url()),
          },
          content: DiscordMessageContent {
            content: msg.content.clone()
          },
          nonce: msg.nonce.clone().map(|n| match n {
            Nonce::Number(n) => n.to_string(),
            Nonce::String(s) => s,
          })
        });
      }
    }
  }
}
