use std::{
  collections::HashMap, sync::{Arc, OnceLock}
};

use serenity::{
  all::{Cache, ChannelId, Context, CreateMessage, Event, EventHandler, GatewayIntents, GetMessages, Message, MessageId, Nonce, RawEventHandler},
  async_trait,
};
use tokio::sync::{broadcast, RwLock};

use crate::{
  channel::DiscordChannel,
  message::{
    author::{DiscordMessageAuthor, DisplayName},
    content::DiscordMessageContent,
    DiscordMessage,
  },
  snowflake::Snowflake,
};

#[allow(dead_code)]
struct SerenityClient {
  // enable this when we enable the serenity[voice] feature
  // voice_manager: Option<Arc<dyn VoiceGatewayManager>>
  http: Arc<Http>,
  cache: Arc<Cache>,
}

#[derive(Default)]
pub struct DiscordClient {
  channel_message_event_handlers: RwLock<HashMap<Snowflake, Vec<broadcast::Sender<DiscordMessage>>>>,
  client: OnceLock<SerenityClient>,
  user: OnceLock<DiscordMessageAuthor>,
  channels: RwLock<HashMap<Snowflake, Arc<DiscordChannel>>>,
}

impl DiscordClient {
  pub async fn new(token: String) -> Arc<DiscordClient> {
    let client = Arc::new(DiscordClient::default());

    let mut discord = serenity::Client::builder(token, GatewayIntents::all())
      .event_handler_arc(client.clone())
      .raw_event_handler(RawClient(client.clone()))
      .await
      .expect("Error creating client");

    let _ = client.client.set(SerenityClient {
      // voice_manager: discord.voice_manager.clone(),
      cache: discord.cache.clone(),
      http: discord.http.clone(),
    });

    tokio::spawn(async move {
      if let Err(why) = discord.start().await {
        panic!("Client error: {why:?}");
      }
    });

    client
  }

  fn discord(&self) -> &SerenityClient {
    self.client.get().unwrap()
  }

  pub fn user(&self) -> &DiscordMessageAuthor {
    self.user.get().unwrap()
  }

  pub async fn add_channel_message_sender(&self, channel: Snowflake, sender: broadcast::Sender<DiscordMessage>) {
    self.channel_message_event_handlers.write().await.entry(channel).or_default().push(sender);
  }

  pub async fn channel(self: Arc<Self>, channel_id: Snowflake) -> Arc<DiscordChannel> {
    let self_clone = self.clone();
    let mut channels = self_clone.channels.write().await;
    let existing = channels.get(&channel_id);

    if let Some(existing) = existing {
      return existing.clone();
    }

    let new = Arc::new(DiscordChannel::new(self, channel_id).await);

    channels.insert(channel_id, new.clone());

    new
  }

  pub async fn send_message(&self, channel_id: Snowflake, content: String, nonce: String) {
    ChannelId::new(channel_id.content)
      .send_message(
        self.discord().http.clone(),
        CreateMessage::new().content(content).enforce_nonce(true).nonce(serenity::all::Nonce::String(nonce)),
      )
      .await
      .unwrap();
  }

  pub async fn get_messages(&self, channel_id: Snowflake, builder: GetMessages) -> Vec<Message> {
    // FIXME: proper error handling
    ChannelId::new(channel_id.content).messages(self.discord().http.clone(), builder).await.unwrap()
  }

  pub async fn get_specific_message(&self, channel_id: Snowflake, message_id: Snowflake) -> Option<Message> {
    // FIXME: proper error handling
    Some(ChannelId::new(channel_id.content).message(self.discord().http.clone(), MessageId::new(message_id.content)).await.unwrap())
  }
}

struct RawClient(Arc<DiscordClient>);

#[async_trait]
impl RawEventHandler for RawClient {
  async fn raw_event(&self, _: Context, ev: serenity::model::prelude::Event) {
    if let Event::Unknown(unk) = ev {
      if unk.kind == "READY" {
        if let Some(user) = unk.value.as_object().and_then(|obj| obj.get("user")).and_then(|u| u.as_object()) {
          let username = user.get("username").and_then(|u| u.as_str()).unwrap_or("Unknown User").to_owned();

          let user_id = user.get("id").and_then(|id| id.as_str()).unwrap_or_default();

          let icon = user
            .get("avatar")
            .and_then(|avatar| avatar.as_str())
            .map(|avatar| format!("https://cdn.discordapp.com/avatars/{}/{}", user_id, avatar))
            .unwrap_or_else(|| {
              format!(
                "https://cdn.discordapp.com/embed/avatars/{}.png",
                (user_id.parse::<u64>().unwrap_or(0) % 5)
              )
            });

          self.0.user.get_or_init(|| DiscordMessageAuthor {
            display_name: DisplayName(username),
            icon,
            id: user_id.to_owned(),
          });
        }
      }
    }
  }
}

#[async_trait]
impl EventHandler for DiscordClient {
  async fn message(&self, _: Context, msg: Message) {
    let snowflake = Snowflake {
      content: msg.channel_id.get(),
    };

    if let Some(vec) = self.channel_message_event_handlers.read().await.get(&snowflake) {
      for sender in vec {
        let _ = sender.send(DiscordMessage::from_serenity(&msg));
      }
    }
  }
}
