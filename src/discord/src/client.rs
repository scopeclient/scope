use std::{
  collections::HashMap,
  sync::{Arc, OnceLock},
};

use crate::{
  channel::DiscordChannel,
  message::{
    author::{DiscordMessageAuthor, DisplayName},
    DiscordMessage,
  },
  snowflake::Snowflake,
};
use scope_chat::reaction::{MessageReactionType, ReactionOperation};
use serenity::all::Reaction;
use serenity::{
  all::{Cache, ChannelId, Context, CreateMessage, EventHandler, GatewayIntents, GetMessages, Http, Message, MessageId, Ready},
  async_trait,
};
use tokio::sync::{broadcast, RwLock};
use crate::message::reaction::discord_reaction_to_emoji;

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
  channel_reaction_event_handlers: RwLock<HashMap<Snowflake, Vec<broadcast::Sender<(String, ReactionOperation)>>>>,
  client: OnceLock<SerenityClient>,
  user: OnceLock<DiscordMessageAuthor>,
  channels: RwLock<HashMap<Snowflake, Arc<DiscordChannel>>>,
}

impl DiscordClient {
  pub async fn new(token: String) -> Arc<DiscordClient> {
    let client = Arc::new(DiscordClient::default());

    let mut discord = serenity::Client::builder(token, GatewayIntents::all()).event_handler_arc(client.clone()).await.expect("Error creating client");

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

  pub async fn add_channel_reaction_sender(&self, channel: Snowflake, sender: broadcast::Sender<(String, ReactionOperation)>) {
    self.channel_reaction_event_handlers.write().await.entry(channel).or_default().push(sender);
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
    println!("Discord: get_messages: {:?}", builder);
    // FIXME: proper error handling
    ChannelId::new(channel_id.content).messages(self.discord().http.clone(), builder).await.unwrap()
  }

  pub async fn get_specific_message(&self, channel_id: Snowflake, message_id: Snowflake) -> Option<Message> {
    println!("Discord: get_specific_messages");
    // FIXME: proper error handling
    Some(ChannelId::new(channel_id.content).message(self.discord().http.clone(), MessageId::new(message_id.content)).await.unwrap())
  }

  async fn send_reaction_operation(&self, channel_id: Snowflake, message_id: MessageId, operation: ReactionOperation) {
    if let Some(vec) = self.channel_reaction_event_handlers.read().await.get(&channel_id) {
      for sender in vec {
        let _ = sender.send((message_id.to_string(), operation.clone()));
      }
    }
  }
}

#[async_trait]
impl EventHandler for DiscordClient {
  async fn ready(&self, _: Context, ready: Ready) {
    self.user.get_or_init(|| DiscordMessageAuthor {
      display_name: DisplayName(ready.user.name.clone()),
      icon: ready.user.face(),
      id: ready.user.id.to_string(),
    });
  }
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

  async fn reaction_add(&self, _: Context, reaction: Reaction) {
    let channel_snowflake = reaction.channel_id.into();

    let ty = if reaction.burst {
      MessageReactionType::Burst
    } else {
      MessageReactionType::Normal
    };

    let emoji = discord_reaction_to_emoji(&reaction.emoji);

    let operation = if reaction.member.is_none_or(|member| member.user.id.get() == self.user().id.parse::<u64>().unwrap()) {
      ReactionOperation::AddSelf(emoji, ty)
    } else {
      ReactionOperation::Add(emoji, ty)
    };

    self.send_reaction_operation(channel_snowflake, reaction.message_id, operation).await;
  }

  async fn reaction_remove(&self, _: Context, reaction: Reaction) {
    let channel_snowflake = reaction.channel_id.into();

    let emoji = discord_reaction_to_emoji(&reaction.emoji);

    let operation = if reaction.member.is_none_or(|member| member.user.id.get() == self.user().id.parse::<u64>().unwrap()) {
      ReactionOperation::RemoveSelf(emoji)
    } else {
      ReactionOperation::Remove(emoji)
    };

    self.send_reaction_operation(channel_snowflake, reaction.message_id, operation).await;
  }

  async fn reaction_remove_all(&self, _: Context, channel_id: ChannelId, removed_from_message_id: MessageId) {
    let channel_snowflake = channel_id.into();
    self.send_reaction_operation(channel_snowflake, removed_from_message_id, ReactionOperation::RemoveAll).await;
  }

  async fn reaction_remove_emoji(&self, _: Context, removed_reactions: Reaction) {
    let channel_snowflake = removed_reactions.channel_id.into();
    let emoji = discord_reaction_to_emoji(&removed_reactions.emoji);
    self.send_reaction_operation(channel_snowflake, removed_reactions.message_id, ReactionOperation::RemoveEmoji(emoji)).await;
  }
}
