use std::{collections::HashMap, rc::Rc, sync::Arc};

use serenity::{all::{Context, EventHandler, GatewayIntents, Message}, async_trait, futures::SinkExt};
use tokio::sync::{broadcast, Mutex, RwLock};

use crate::{message::{author::{DiscordMessageAuthor, DisplayName}, content::DiscordMessageContent, DiscordMessage}, snowflake::{self, Snowflake}};

#[derive(Default)]
pub struct DiscordClient {
  channel_message_event_handlers: HashMap<Snowflake, Vec<broadcast::Sender<DiscordMessage>>>
}

impl DiscordClient {
  pub fn new(token: String) -> Arc<RwLock<DiscordClient>> {
    let client = Arc::new(RwLock::new(DiscordClient::default()));
    let remote = RemoteDiscordClient(client.clone());

    tokio::spawn(async {
      let mut client = serenity::Client::builder(token, GatewayIntents::all())
        .event_handler(remote)
        .await
        .expect("Error creating client");

      if let Err(why) = client.start().await {
        panic!("Client error: {why:?}");
      }
    });

    client
  }

  pub async fn add_channel_message_sender(this: &mut Arc<RwLock<Self>>, channel: Snowflake, sender: broadcast::Sender<DiscordMessage>) {
    let mut this = this.write().await;

    this.channel_message_event_handlers.entry(channel).or_default().push(sender);
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

    let snowflake = Snowflake { content: msg.channel_id.get() };

    if let Some(vec) = self.0.read().await.channel_message_event_handlers.get(&snowflake) {
      for sender in vec {
        println!("Sending to sender!");

        let _ = sender.send(DiscordMessage {
          id: snowflake,
          author: DiscordMessageAuthor {
            display_name: DisplayName(msg.author.name.clone()),
            icon: msg.author.avatar_url().unwrap_or(msg.author.default_avatar_url())
          },
          content: DiscordMessageContent {
            content: msg.content.clone()
          }
        });
      }
    }
  }
}