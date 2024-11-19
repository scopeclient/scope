use crate::channel::DiscordChannelMetadata;
use crate::{
  message::{
    author::{DiscordMessageAuthor, DisplayName},
    content::DiscordMessageContent,
    DiscordMessage,
  },
  snowflake::Snowflake,
};
use scope_chat::channel::ChannelMetadata;
use serde_json::Map;
use serenity::all::Channel;
use serenity::json::Value;
use serenity::{
  all::{Cache, ChannelId, Context, CreateMessage, Event, EventHandler, GatewayIntents, Http, Message, Nonce, RawEventHandler},
  async_trait,
};
use std::{
  collections::HashMap,
  sync::{Arc, OnceLock},
};
use tokio::sync::{broadcast, RwLock};

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
  channel_updates_event_handler: RwLock<Vec<broadcast::Sender<DiscordChannelMetadata>>>,
  direct_message_channels: RwLock<Vec<DiscordChannelMetadata>>,
  client: OnceLock<SerenityClient>,
  user: OnceLock<DiscordMessageAuthor>,
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

  pub async fn set_channel_update_sender(&self, sender: broadcast::Sender<DiscordChannelMetadata>) {
    self.channel_updates_event_handler.write().await.push(sender);
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

  pub async fn get_channel(&self, channel_id: Snowflake) -> Option<Channel> {
    ChannelId::new(channel_id.content).to_channel(self.discord().http.clone()).await.ok()
  }

  pub async fn list_direct_message_channels(&self) -> Vec<DiscordChannelMetadata> {
    self.direct_message_channels.read().await.clone()
  }
}

fn avatar_by_user_value(user: &Map<String, Value>, user_id: String) -> String {
  user
    .get("avatar")
    .and_then(|avatar| avatar.as_str())
    .map(|avatar| format!("https://cdn.discordapp.com/avatars/{}/{}", user_id, avatar))
    .unwrap_or_else(|| {
      format!(
        "https://cdn.discordapp.com/embed/avatars/{}.png",
        (user_id.parse::<u64>().unwrap_or(0) % 5)
      )
    })
}

struct RawClient(Arc<DiscordClient>);

#[async_trait]
impl RawEventHandler for RawClient {
  async fn raw_event(&self, _: Context, ev: Event) {
    if let Event::Unknown(unk) = ev {
      if unk.kind == "READY" {
        if let Some(user) = unk.value.as_object().and_then(|obj| obj.get("user")).and_then(|u| u.as_object()) {
          let username = user.get("username").and_then(|u| u.as_str()).unwrap_or("Unknown User").to_owned();

          let user_id = user.get("id").and_then(|id| id.as_str()).unwrap_or_default();
          let icon = avatar_by_user_value(user, user_id.to_owned());

          self.0.user.get_or_init(|| DiscordMessageAuthor {
            display_name: DisplayName(username),
            icon,
            id: user_id.to_owned(),
          });
        }

        if let Some(private_channels) = unk.value.as_object().and_then(|obj| obj.get("private_channels")).and_then(|channels| channels.as_array()) {
          for channel in private_channels {
            if let Some(user) = channel
              .get("recipients")
              .and_then(|recipients| recipients.as_array())
              .and_then(|recipients| recipients.get(0))
              .and_then(|recipient| recipient.as_object())
            {
              let channel_id = channel.get("id").and_then(|id| id.as_str()).unwrap_or_default().to_owned();

              let user_id = user.get("id").and_then(|id| id.as_str()).unwrap_or_default().to_owned();
              let user_name = user.get("username").and_then(|name| name.as_str()).unwrap_or_default().to_owned();
              let chat_icon = avatar_by_user_value(user, user_id.to_owned());

              let channel = DiscordChannelMetadata::new(
                self.0.clone(),
                Snowflake {
                  content: channel_id.parse().unwrap(),
                },
                user_name,
                chat_icon,
              )
              .await;

              self.0.direct_message_channels.write().await.push(channel.clone());
              self.0.channel_updates_event_handler.read().await.iter().for_each(|sender| {
                let _ = sender.send(channel.clone());
              });
            }
          }
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
        let _ = sender.send(DiscordMessage {
          id: snowflake,
          author: DiscordMessageAuthor {
            display_name: DisplayName(msg.author.name.clone()),
            icon: msg.author.avatar_url().unwrap_or(msg.author.default_avatar_url()),
            id: msg.author.id.to_string(),
          },
          content: DiscordMessageContent {
            content: msg.content.clone(),
            is_pending: false,
          },
          nonce: msg.nonce.clone().map(|n| match n {
            Nonce::Number(n) => n.to_string(),
            Nonce::String(s) => s,
          }),
          creation_time: msg.timestamp,
        });
      }
    }
  }
}
