use std::{
  collections::HashMap,
  sync::{Arc, OnceLock, Weak},
};

use crate::message::reaction::discord_reaction_to_emoji;
use atomic_refcell::AtomicRefCell;
use dashmap::DashMap;
use scope_chat::reaction::{MessageReactionType, ReactionEvent, ReactionOperation};
use serenity::all::Reaction;
use serenity::{
  all::{
    Cache, CacheHttp, ChannelId, Context, CreateMessage, EventHandler, GatewayIntents, GetMessages, GuildId, Http, Member, Message, MessageId,
    ModelError, Ready, User,
  },
  async_trait,
};
use tokio::sync::{broadcast, RwLock};

use crate::{channel::DiscordChannel, message::DiscordMessage, snowflake::Snowflake};

#[allow(dead_code)]
pub struct SerenityClient {
  // enable this when we enable the serenity[voice] feature
  // voice_manager: Option<Arc<dyn VoiceGatewayManager>>
  http: Arc<Http>,
  cache: Arc<Cache>,
}

impl CacheHttp for SerenityClient {
  fn http(&self) -> &Http {
    &self.http
  }

  fn cache(&self) -> Option<&Arc<Cache>> {
    Some(&self.cache)
  }
}

#[derive(Default)]
pub struct DiscordClient {
  channel_message_event_handlers: RwLock<HashMap<ChannelId, Vec<broadcast::Sender<DiscordMessage>>>>,
  channel_reaction_event_handlers: RwLock<HashMap<ChannelId, Vec<broadcast::Sender<ReactionEvent<Snowflake>>>>>,
  client: OnceLock<SerenityClient>,
  user: OnceLock<Arc<User>>,
  channels: RwLock<HashMap<ChannelId, Arc<DiscordChannel>>>,
  member: DashMap<GuildId, Arc<Member>>,
  ready_notifier: AtomicRefCell<Option<catty::Sender<()>>>,
  weak: Weak<DiscordClient>,
}

impl DiscordClient {
  pub async fn new(token: String) -> Arc<DiscordClient> {
    let (sender, receiver) = catty::oneshot::<()>();

    let client = Arc::new_cyclic(|weak| DiscordClient {
      ready_notifier: AtomicRefCell::new(Some(sender)),
      weak: weak.clone(),

      ..Default::default()
    });

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

    receiver.await.expect("The ready notifier was dropped");

    client
  }

  pub fn discord(&self) -> &SerenityClient {
    self.client.get().unwrap()
  }

  pub fn own_user(&self) -> Arc<User> {
    self.user.get().unwrap().clone()
  }

  pub fn own_member(&self, guild: GuildId) -> Option<Arc<Member>> {
    self.member.get(&guild).map(|v| v.clone())
  }

  pub async fn add_channel_message_sender(&self, channel: ChannelId, sender: broadcast::Sender<DiscordMessage>) {
    self.channel_message_event_handlers.write().await.entry(channel).or_default().push(sender);
  }

  pub async fn add_channel_reaction_sender(&self, channel: ChannelId, sender: broadcast::Sender<ReactionEvent<Snowflake>>) {
    self.channel_reaction_event_handlers.write().await.entry(channel).or_default().push(sender);
  }

  pub async fn channel(self: Arc<Self>, channel_id: Snowflake) -> Arc<DiscordChannel> {
    let channel_id = ChannelId::new(channel_id.0);

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

  pub async fn send_message(&self, channel_id: ChannelId, content: String, nonce: String) {
    channel_id
      .send_message(
        self.discord().http.clone(),
        CreateMessage::new().content(content).enforce_nonce(true).nonce(serenity::all::Nonce::String(nonce)),
      )
      .await
      .unwrap();
  }

  pub async fn get_messages(&self, channel_id: ChannelId, builder: GetMessages) -> Vec<Message> {
    println!("Discord: get_messages: {:?}", builder);
    // FIXME: proper error handling
    channel_id.messages(self.discord().http.clone(), builder).await.unwrap()
  }

  pub async fn get_specific_message(&self, channel_id: ChannelId, message_id: MessageId) -> Option<Message> {
    println!("Discord: get_specific_messages");
    // FIXME: proper error handling
    Some(channel_id.message(self.discord().http.clone(), message_id).await.unwrap())
  }

  async fn send_reaction_operation(&self, channel_id: ChannelId, message_id: MessageId, operation: ReactionOperation) {
    if let Some(vec) = self.channel_reaction_event_handlers.read().await.get(&channel_id) {
      for sender in vec {
        let _ = sender.send((message_id.into(), operation.clone()));
      }
    }
  }
}

#[async_trait]
impl EventHandler for DiscordClient {
  async fn ready(&self, _: Context, ready: Ready) {
    self.user.get_or_init(|| Arc::new((*ready.user).clone()));

    if let Some(ready_notifier) = self.ready_notifier.borrow_mut().take() {
      ready_notifier.send(()).unwrap();
    }
  }

  async fn message(&self, _: Context, msg: Message) {
    if let Some(vec) = self.channel_message_event_handlers.read().await.get(&msg.channel_id) {
      let msg = Arc::new(msg);
      let channel = Arc::new(msg.channel(self.discord()).await.unwrap());
      let member = match msg.member(self.discord()).await {
        Ok(v) => Ok(Some(Arc::new(v))),
        Err(serenity::Error::Model(ModelError::ItemMissing)) => Ok(None),
        Err(e) => Err(e),
      }
      .unwrap();

      for sender in vec {
        let _ = sender.send(DiscordMessage::from_serenity(
          self.weak.upgrade().unwrap(),
          msg.clone(),
          channel.clone(),
          member.clone(),
        ));
      }
    }
  }

  async fn reaction_add(&self, _: Context, reaction: Reaction) {
    let ty = if reaction.burst {
      MessageReactionType::Burst
    } else {
      MessageReactionType::Normal
    };

    let emoji = discord_reaction_to_emoji(&reaction.emoji);
    let me_id = self.user.get().expect("User not ready").id;

    let operation = if reaction.member.is_none_or(|member| member.user.id == me_id) {
      ReactionOperation::AddSelf(emoji, ty)
    } else {
      ReactionOperation::Add(emoji, ty)
    };

    self.send_reaction_operation(reaction.channel_id, reaction.message_id, operation).await;
  }

  async fn reaction_remove(&self, _: Context, reaction: Reaction) {
    let emoji = discord_reaction_to_emoji(&reaction.emoji);
    let me_id = self.user.get().expect("User not ready").id;

    let operation = if reaction.member.is_none_or(|member| member.user.id == me_id) {
      ReactionOperation::RemoveSelf(emoji)
    } else {
      ReactionOperation::Remove(emoji)
    };

    self.send_reaction_operation(reaction.channel_id, reaction.message_id, operation).await;
  }

  async fn reaction_remove_all(&self, _: Context, channel_id: ChannelId, removed_from_message_id: MessageId) {
    self.send_reaction_operation(channel_id, removed_from_message_id, ReactionOperation::RemoveAll).await;
  }

  async fn reaction_remove_emoji(&self, _: Context, removed_reactions: Reaction) {
    let emoji = discord_reaction_to_emoji(&removed_reactions.emoji);
    self.send_reaction_operation(removed_reactions.channel_id, removed_reactions.message_id, ReactionOperation::RemoveEmoji(emoji)).await;
  }
}
