use std::{
  collections::{HashMap, HashSet},
  sync::{Arc, Mutex, OnceLock, Weak},
  time::Duration,
};

use atomic_refcell::AtomicRefCell;
use dashmap::{DashMap, DashSet};
use serenity::{
  all::{
    Activity, Cache, CacheHttp, Channel, ChannelId, Context, CreateMessage, EditMessage, Event, EventHandler, GatewayIntents, GetMessages, Guild,
    GuildChannel, GuildId, GuildMemberUpdateEvent, GuildMembersChunkEvent, Http, Member, Message, MessageId, MessageUpdateEvent, ModelError, Nonce,
    OnlineStatus, PartialGuild, PartialGuildChannel, Presence, RawEventHandler, Reaction, ReactionType, Ready, Role, RoleId, UnavailableGuild, User,
  },
  async_trait,
  json::{Value, from_value},
};
use tokio::sync::{RwLock, broadcast};

use scope_chat::channel::ChannelEvent;

use crate::{
  channel::DiscordChannel,
  dm::{self, DmChannel},
  message::DiscordMessage,
  nav::{ClientEvent, custom_status, id},
  snowflake::Snowflake,
  unread::UnreadTracker,
};

/// How long `READY` may wait for the DM list before login proceeds without it.
const DM_FETCH_BUDGET: Duration = Duration::from_secs(10);
/// Hard cap on how long login waits for the gateway READY event.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);

/// Reaction events for one message arriving within this window share a single re-fetch.
const REACTION_REFRESH_COALESCE: Duration = Duration::from_millis(300);

/// Which kind of Discord token we were given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TokenKind {
  /// A bot token (`Bot xxx`); the default for testing.
  #[default]
  Bot,
  /// A user account token (requires the user-account patches in the serenity fork).
  User,
}

/// Gateway intents to request. Bots need the privileged ones enabled in the
/// Developer Portal, otherwise Discord refuses the connection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Intents {
  #[default]
  All,
  NonPrivileged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectError {
  /// The bot asked for privileged intents that are not enabled for it.
  DisallowedIntents,
  InvalidToken,
  Build(String),
  Gateway(String),
}

impl std::fmt::Display for ConnectError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ConnectError::DisallowedIntents => {
        write!(
          f,
          "This bot's privileged intents are not enabled. Turn on Presence, Server Members and Message Content in the Discord Developer Portal."
        )
      }
      ConnectError::InvalidToken => write!(f, "Discord rejected the token."),
      ConnectError::Build(e) => write!(f, "could not create the Discord client: {e}"),
      ConnectError::Gateway(e) => write!(f, "Discord connection failed: {e}"),
    }
  }
}

/// Broadcast bus for [`ClientEvent`]s; defaulted so `DiscordClient` can keep deriving `Default`.
pub struct EventBus(broadcast::Sender<ClientEvent>);

impl Default for EventBus {
  fn default() -> Self {
    EventBus(broadcast::channel(256).0)
  }
}

/// The signed-in user's own status, as reported by the user-account `SESSIONS_REPLACE` event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OwnPresence {
  pub status: OnlineStatus,
  pub status_text: Option<String>,
}

#[allow(dead_code)]
pub struct SerenityClient {
  // enable this when we enable the serenity[voice] feature
  // voice_manager: Option<Arc<dyn VoiceGatewayManager>>
  pub(crate) http: Arc<Http>,
  pub(crate) cache: Arc<Cache>,
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
  channel_message_event_handlers: RwLock<HashMap<ChannelId, Vec<broadcast::Sender<ChannelEvent<DiscordMessage>>>>>,
  client: OnceLock<SerenityClient>,
  user: OnceLock<Arc<User>>,
  channels: RwLock<HashMap<ChannelId, Arc<DiscordChannel>>>,
  member: DashMap<GuildId, Arc<Member>>,
  ready_notifier: AtomicRefCell<Option<catty::Sender<()>>>,
  connect_error: AtomicRefCell<Option<ConnectError>>,
  weak: Weak<DiscordClient>,
  events: EventBus,
  /// Private channels (DMs and group DMs), which the serenity cache does not hold.
  pub(crate) dms: DashMap<ChannelId, DmChannel>,
  pub(crate) unread: UnreadTracker,
  own_presence: Mutex<Option<OwnPresence>>,
  /// Messages with a re-fetch already scheduled (see [`REACTION_REFRESH_COALESCE`]).
  pending_refreshes: DashSet<(ChannelId, MessageId)>,
}

impl DiscordClient {
  /// Connect to Discord with a user token and wait until the gateway is ready.
  ///
  /// Fails if the client cannot be built or the gateway stops before `READY`
  /// (typically an invalid token).
  pub async fn new(token: String, kind: TokenKind, intents: Intents) -> Result<Arc<DiscordClient>, ConnectError> {
    // The fork passes tokens through verbatim, so the `Bot ` prefix is on us.
    let raw = token.trim().trim_start_matches("Bot ").to_string();
    let token = match kind {
      TokenKind::Bot => format!("Bot {raw}"),
      TokenKind::User => raw,
    };

    let intents = match intents {
      Intents::All => GatewayIntents::all(),
      Intents::NonPrivileged => GatewayIntents::non_privileged(),
    };

    let (sender, receiver) = catty::oneshot::<()>();

    let client = Arc::new_cyclic(|weak| DiscordClient {
      ready_notifier: AtomicRefCell::new(Some(sender)),
      weak: weak.clone(),

      ..Default::default()
    });

    let mut discord = serenity::Client::builder(token, intents)
      .event_handler_arc(client.clone())
      .raw_event_handler(RawEvents(client.weak.clone()))
      .await
      .map_err(|e| ConnectError::Build(e.to_string()))?;

    let _ = client.client.set(SerenityClient {
      // voice_manager: discord.voice_manager.clone(),
      cache: discord.cache.clone(),
      http: discord.http.clone(),
    });

    let weak = Arc::downgrade(&client);

    tokio::spawn(async move {
      if let Err(why) = discord.start().await {
        log::error!("Discord gateway stopped: {why:?}");

        let error = match &why {
          serenity::Error::Gateway(serenity::gateway::GatewayError::DisallowedGatewayIntents) => ConnectError::DisallowedIntents,
          serenity::Error::Gateway(serenity::gateway::GatewayError::InvalidAuthentication) => ConnectError::InvalidToken,
          other => ConnectError::Gateway(other.to_string()),
        };

        // Record why, then drop the notifier so `new` wakes with an error instead of hanging.
        if let Some(client) = weak.upgrade() {
          *client.connect_error.borrow_mut() = Some(error);
          client.ready_notifier.borrow_mut().take();
        }
      }
    });

    // Defence in depth: never hang forever. If READY has not arrived in time the
    // connection is wedged (e.g. a gateway payload that will not deserialise);
    // surface that instead of spinning on "Connecting…".
    match tokio::time::timeout(CONNECT_TIMEOUT, receiver).await {
      Ok(Ok(())) => Ok(client),
      Ok(Err(_)) => {
        let error = client.connect_error.borrow_mut().take();
        Err(error.unwrap_or_else(|| ConnectError::Gateway("the connection closed before it became ready".into())))
      }
      Err(_) => Err(ConnectError::Gateway(format!("Discord did not become ready within {CONNECT_TIMEOUT:?}. Check the token and the bot's intents."))),
    }
  }

  /// Subscribe to navigation-relevant client events.
  pub fn events(&self) -> broadcast::Receiver<ClientEvent> {
    self.events.0.subscribe()
  }

  pub(crate) fn emit(&self, event: ClientEvent) {
    let _ = self.events.0.send(event);
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

  /// Own status from `SESSIONS_REPLACE`, if Discord has sent one yet.
  pub(crate) fn own_presence(&self) -> Option<OwnPresence> {
    self.own_presence.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
  }

  pub async fn add_channel_message_sender(&self, channel: ChannelId, sender: broadcast::Sender<ChannelEvent<DiscordMessage>>) {
    self.channel_message_event_handlers.write().await.entry(channel).or_default().push(sender);
  }

  pub async fn channel(self: Arc<Self>, channel_id: Snowflake) -> Arc<DiscordChannel> {
    let channel_id = ChannelId::new(channel_id.0);

    // Opening a channel reads it.
    self.mark_read(channel_id);

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

  /// The channel behind `channel_id`.
  ///
  /// Private channels come from the DM list (the fork cannot deserialize group DMs from
  /// REST); everything else from the cache or REST. `None` when neither knows it.
  pub(crate) async fn resolve_channel(&self, channel_id: ChannelId) -> Option<Channel> {
    if let Some(dm) = self.dms.get(&channel_id) {
      return Some(dm.to_channel(&self.own_user()));
    }

    match channel_id.to_channel(self.discord()).await {
      Ok(channel) => Some(channel),
      Err(why) => {
        log::warn!("Discord: could not resolve channel {channel_id}: {why:?}");
        None
      }
    }
  }

  /// The guild member who sent `msg`, when it is a guild message and the member can be found.
  pub(crate) async fn message_member(&self, msg: &Message) -> Option<Arc<Member>> {
    match msg.member(self.discord()).await {
      Ok(member) => Some(Arc::new(member)),
      Err(serenity::Error::Model(ModelError::ItemMissing)) => None,
      Err(why) => {
        log::debug!("Discord: could not load the author of message {}: {why:?}", msg.id);
        None
      }
    }
  }

  /// Re-fetch the private channel list from REST and publish it.
  pub(crate) async fn refresh_dms(&self) {
    match dm::fetch_dm_channels(&self.discord().http).await {
      Ok(channels) => {
        log::debug!("Discord: loaded {} private channels", channels.len());

        let ids: HashSet<ChannelId> = channels.iter().map(|channel| channel.id).collect();
        self.dms.retain(|id, _| ids.contains(id));

        for channel in channels {
          self.dms.insert(channel.id, channel);
        }

        self.emit(ClientEvent::GuildsUpdated);
      }
      Err(why) => log::warn!("Discord: could not fetch private channels: {why:?}"),
    }
  }

  /// Clear unread state for a channel and tell the UI, if anything changed.
  pub(crate) fn mark_read(&self, channel_id: ChannelId) {
    let Some(cleared) = self.unread.mark_read(channel_id) else { return };

    match cleared.guild_id {
      Some(guild) => {
        self.emit(ClientEvent::ChannelsUpdated(id(guild)));

        // The guild badge sums mentions.
        if cleared.mentions > 0 {
          self.emit(ClientEvent::GuildsUpdated);
        }
      }
      None => self.emit(ClientEvent::GuildsUpdated),
    }
  }

  fn mentions_own_role(&self, guild_id: GuildId, roles: &[RoleId]) -> bool {
    if roles.is_empty() {
      return false;
    }

    let Some(own) = self.user.get() else { return false };
    let cache = &self.discord().cache;

    cache
      .guild(guild_id)
      .and_then(|guild| guild.members.get(&own.id).map(|member| member.roles.iter().any(|role| roles.contains(role))))
      .unwrap_or(false)
  }

  /// Unread and DM bookkeeping for an incoming message.
  async fn record_message(&self, msg: &Message) {
    let own_id = self.user.get().map(|user| user.id);
    let is_own = own_id == Some(msg.author.id);
    let is_dm = msg.guild_id.is_none();

    if is_dm {
      match self.dms.get_mut(&msg.channel_id) {
        Some(mut dm) => dm.last_message_id = Some(msg.id),
        // A DM created since the list was fetched: CHANNEL_CREATE for DMs does not survive the fork's deserializer.
        None => {
          if let Some(client) = self.weak.upgrade() {
            tokio::spawn(async move { client.refresh_dms().await });
          }
        }
      }
    }

    // Posting in a channel reads it. A channel opened in this session is treated as read
    // too: we cannot tell which one is on screen, and a badge on the visible channel is worse
    // than a missing one.
    if is_own || self.channels.read().await.contains_key(&msg.channel_id) {
      self.mark_read(msg.channel_id);

      if is_dm {
        // DM order follows the last message.
        self.emit(ClientEvent::GuildsUpdated);
      }

      return;
    }

    let mentioned = is_dm
      || msg.mention_everyone
      || own_id.is_some_and(|own| msg.mentions_user_id(own))
      || msg.guild_id.is_some_and(|guild| self.mentions_own_role(guild, &msg.mention_roles));

    self.unread.record(msg.channel_id, msg.guild_id, mentioned);

    match msg.guild_id {
      Some(guild) => {
        self.emit(ClientEvent::ChannelsUpdated(id(guild)));

        if mentioned {
          self.emit(ClientEvent::GuildsUpdated);
        }
      }
      None => self.emit(ClientEvent::GuildsUpdated),
    }
  }

  /// User-account gateway events the fork has no model for.
  fn raw_event(&self, event: Event) {
    let Event::Unknown(unknown) = event else { return };

    match unknown.kind.as_str() {
      "SESSIONS_REPLACE" => self.sessions_replaced(&unknown.value),
      // `{ channel_id, message_id, version, mention_count?, manual? }`: another client read the channel.
      "MESSAGE_ACK" => {
        if let Some(channel) = unknown.value.get("channel_id").and_then(|channel| from_value::<ChannelId>(channel.clone()).ok()) {
          self.mark_read(channel);
        }
      }
      _ => {}
    }
  }

  /// `SESSIONS_REPLACE` lists every gateway session of the account with its `status` and
  /// `activities`; the `"all"` pseudo-session (else the active one, else the first) is the aggregate.
  fn sessions_replaced(&self, value: &Value) {
    let Some(sessions) = value.as_array() else { return };

    let session = sessions
      .iter()
      .find(|session| session.get("session_id").and_then(Value::as_str) == Some("all"))
      .or_else(|| sessions.iter().find(|session| session.get("active").and_then(Value::as_bool) == Some(true)))
      .or_else(|| sessions.first());

    let Some(session) = session else { return };
    let Some(status) = session.get("status").and_then(|status| from_value::<OnlineStatus>(status.clone()).ok()) else {
      return;
    };
    let activities: Vec<Activity> = session.get("activities").and_then(|activities| from_value(activities.clone()).ok()).unwrap_or_default();

    let presence = OwnPresence {
      status,
      status_text: custom_status(&activities),
    };

    let changed = {
      let mut own = self.own_presence.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
      let changed = own.as_ref() != Some(&presence);
      *own = Some(presence);
      changed
    };

    if changed {
      self.emit(ClientEvent::GuildsUpdated);
    }
  }

  fn channels_updated(&self, guild: GuildId) {
    self.emit(ClientEvent::ChannelsUpdated(id(guild)));
  }

  fn members_updated(&self, guild: GuildId) {
    self.emit(ClientEvent::MembersUpdated(id(guild)));
  }

  /// Send `content` to `channel_id`, as a reply to `reply_to` when given.
  pub async fn send_message(&self, channel_id: ChannelId, content: String, nonce: String, reply_to: Option<MessageId>) {
    let mut builder = CreateMessage::new().content(content).enforce_nonce(true).nonce(Nonce::String(nonce));

    if let Some(reply_to) = reply_to {
      builder = builder.reference_message((channel_id, reply_to));
    }

    // The http client alone: a permission check against the cache would refuse DMs it does not hold.
    if let Err(why) = channel_id.send_message(self.discord().http.clone(), builder).await {
      log::warn!("Discord: could not send a message to {channel_id}: {why:?}");
    }
  }

  pub async fn edit_message(&self, channel_id: ChannelId, message_id: MessageId, content: String) {
    let result = channel_id.edit_message(self.discord().http.clone(), message_id, EditMessage::new().content(content)).await;
    report(channel_id, "edit a message", result);
  }

  pub async fn delete_message(&self, channel_id: ChannelId, message_id: MessageId) {
    let result = channel_id.delete_message(&self.discord().http, message_id).await;
    report(channel_id, "delete a message", result);
  }

  pub async fn add_reaction(&self, channel_id: ChannelId, message_id: MessageId, reaction: ReactionType) {
    let result = channel_id.create_reaction(&self.discord().http, message_id, reaction).await;
    report(channel_id, "add a reaction", result);
  }

  /// Remove our own `reaction` from a message.
  pub async fn remove_reaction(&self, channel_id: ChannelId, message_id: MessageId, reaction: ReactionType) {
    let result = channel_id.delete_reaction(&self.discord().http, message_id, None, reaction).await;
    report(channel_id, "remove a reaction", result);
  }

  /// Show the typing indicator in `channel_id` for a few seconds.
  pub async fn broadcast_typing(&self, channel_id: ChannelId) {
    let result = channel_id.broadcast_typing(&self.discord().http).await;
    report(channel_id, "start typing", result);
  }

  /// A channel opened in this session, if `channel_id` is one.
  async fn open_channel(&self, channel_id: ChannelId) -> Option<Arc<DiscordChannel>> {
    self.channels.read().await.get(&channel_id).cloned()
  }

  /// Deliver `event` to every subscriber of `channel_id`.
  async fn broadcast(&self, channel_id: ChannelId, event: ChannelEvent<DiscordMessage>) {
    let senders = self.channel_message_event_handlers.read().await.get(&channel_id).cloned();

    for sender in senders.into_iter().flatten() {
      let _ = sender.send(event.clone());
    }
  }

  /// Take `msg` as a message's new state: update the open channel's cache and tell subscribers.
  async fn publish_update(&self, channel: &DiscordChannel, msg: Arc<Message>) {
    let channel_id = msg.channel_id;
    let updated = channel.message_updated(msg).await;

    self.broadcast(channel_id, ChannelEvent::Updated(updated)).await;
  }

  /// Drop a deleted message from the open channel's cache and tell subscribers.
  async fn publish_delete(&self, channel_id: ChannelId, message_id: MessageId) {
    let Some(channel) = self.open_channel(channel_id).await else { return };
    let id = Snowflake::from(message_id);

    channel.message_deleted(id).await;
    self.broadcast(channel_id, ChannelEvent::Deleted(id)).await;
  }

  /// Re-fetch `message_id` shortly and publish it as updated.
  ///
  /// Reaction events carry no counts, so the message has to be fetched again; they also arrive
  /// in bursts (one per user per emoji), so a message already waiting is not fetched twice.
  async fn refresh_message_soon(&self, channel_id: ChannelId, message_id: MessageId) {
    if self.open_channel(channel_id).await.is_none() {
      return;
    }

    if !self.pending_refreshes.insert((channel_id, message_id)) {
      return;
    }

    let Some(client) = self.weak.upgrade() else { return };

    tokio::spawn(async move {
      tokio::time::sleep(REACTION_REFRESH_COALESCE).await;
      client.pending_refreshes.remove(&(channel_id, message_id));

      let Some(channel) = client.open_channel(channel_id).await else { return };
      let Some(message) = client.get_specific_message(channel_id, message_id).await else {
        return;
      };

      client.publish_update(&channel, Arc::new(message)).await;
    });
  }

  /// Messages matching `builder`; empty when the request fails.
  pub async fn get_messages(&self, channel_id: ChannelId, builder: GetMessages) -> Vec<Message> {
    log::debug!("Discord: get_messages in {channel_id}: {builder:?}");

    match channel_id.messages(self.discord().http.clone(), builder).await {
      Ok(messages) => messages,
      Err(why) => {
        log::warn!("Discord: could not fetch messages in {channel_id}: {why:?}");
        Vec::new()
      }
    }
  }

  pub async fn get_specific_message(&self, channel_id: ChannelId, message_id: MessageId) -> Option<Message> {
    log::debug!("Discord: get_specific_message {message_id} in {channel_id}");

    match channel_id.message(self.discord().http.clone(), message_id).await {
      Ok(message) => Some(message),
      Err(why) => {
        log::warn!("Discord: could not fetch message {message_id} in {channel_id}: {why:?}");
        None
      }
    }
  }
}

/// Log a failed fire-and-forget request; the UI has nowhere to show it.
fn report<T>(channel_id: ChannelId, what: &str, result: serenity::Result<T>) {
  if let Err(why) = result {
    log::warn!("Discord: could not {what} in {channel_id}: {why:?}");
  }
}

/// Forwards raw gateway events to the client without another strong reference cycle.
struct RawEvents(Weak<DiscordClient>);

#[async_trait]
impl RawEventHandler for RawEvents {
  async fn raw_event(&self, _: Context, event: Event) {
    if let Some(client) = self.0.upgrade() {
      client.raw_event(event);
    }
  }
}

#[async_trait]
impl EventHandler for DiscordClient {
  async fn ready(&self, _: Context, ready: Ready) {
    self.user.get_or_init(|| Arc::new((*ready.user).clone()));
    log::debug!("Discord: ready as {} with {} guilds", ready.user.name, ready.guilds.len());

    // READY's `private_channels` is skipped by the fork, so load DMs before the UI first paints,
    // but never let a stalled request hold up login: finish it in the background instead.
    if tokio::time::timeout(DM_FETCH_BUDGET, self.refresh_dms()).await.is_err() {
      log::warn!("Discord: private channels took longer than {DM_FETCH_BUDGET:?}; loading them in the background");

      if let Some(client) = self.weak.upgrade() {
        tokio::spawn(async move { client.refresh_dms().await });
      }
    }

    if let Some(ready_notifier) = self.ready_notifier.borrow_mut().take() {
      let _ = ready_notifier.send(());
    }

    self.emit(ClientEvent::Ready);
  }

  async fn guild_create(&self, _: Context, _guild: Guild, _is_new: Option<bool>) {
    self.emit(ClientEvent::GuildsUpdated);
  }

  async fn guild_update(&self, _: Context, _old: Option<Guild>, _new: PartialGuild) {
    self.emit(ClientEvent::GuildsUpdated);
  }

  /// Also fired when a guild becomes unavailable (`incomplete.unavailable`).
  async fn guild_delete(&self, _: Context, _incomplete: UnavailableGuild, _full: Option<Guild>) {
    self.emit(ClientEvent::GuildsUpdated);
  }

  async fn channel_create(&self, _: Context, channel: GuildChannel) {
    self.channels_updated(channel.guild_id);
  }

  async fn channel_update(&self, _: Context, _old: Option<GuildChannel>, new: GuildChannel) {
    self.channels_updated(new.guild_id);
  }

  async fn channel_delete(&self, _: Context, channel: GuildChannel, _messages: Option<Vec<Message>>) {
    self.channels_updated(channel.guild_id);
  }

  async fn category_create(&self, _: Context, category: GuildChannel) {
    self.channels_updated(category.guild_id);
  }

  async fn category_delete(&self, _: Context, category: GuildChannel) {
    self.channels_updated(category.guild_id);
  }

  async fn thread_create(&self, _: Context, thread: GuildChannel) {
    self.channels_updated(thread.guild_id);
  }

  async fn thread_update(&self, _: Context, _old: Option<GuildChannel>, new: GuildChannel) {
    self.channels_updated(new.guild_id);
  }

  async fn thread_delete(&self, _: Context, thread: PartialGuildChannel, _full: Option<GuildChannel>) {
    self.channels_updated(thread.guild_id);
  }

  async fn guild_member_addition(&self, _: Context, new_member: Member) {
    self.members_updated(new_member.guild_id);
  }

  async fn guild_member_removal(&self, _: Context, guild_id: GuildId, _user: User, _member: Option<Member>) {
    self.members_updated(guild_id);
  }

  async fn guild_member_update(&self, _: Context, _old: Option<Member>, _new: Option<Member>, event: GuildMemberUpdateEvent) {
    self.members_updated(event.guild_id);
  }

  async fn guild_members_chunk(&self, _: Context, chunk: GuildMembersChunkEvent) {
    self.members_updated(chunk.guild_id);
  }

  // Role groups in the member list follow hoisted roles.
  async fn guild_role_create(&self, _: Context, new: Role) {
    self.members_updated(new.guild_id);
  }

  async fn guild_role_update(&self, _: Context, _old: Option<Role>, new: Role) {
    self.members_updated(new.guild_id);
  }

  async fn guild_role_delete(&self, _: Context, guild_id: GuildId, _role_id: RoleId, _role: Option<Role>) {
    self.members_updated(guild_id);
  }

  async fn typing_start(&self, ctx: Context, event: serenity::all::TypingStartEvent) {
    if event.user_id == self.own_user().id {
      return;
    }

    let user = match &event.member {
      Some(member) => member.display_name().to_owned(),
      None => ctx.cache.user(event.user_id).map(|u| u.display_name().to_owned()).unwrap_or_else(|| event.user_id.to_string()),
    };

    self.emit(ClientEvent::Typing {
      channel: id(event.channel_id),
      user,
    });
  }

  async fn presence_update(&self, _: Context, new_data: Presence) {
    if let Some(guild_id) = new_data.guild_id {
      self.emit(ClientEvent::PresenceUpdated(id(guild_id)));
    }
  }

  async fn message(&self, _: Context, msg: Message) {
    self.record_message(&msg).await;

    let senders = self.channel_message_event_handlers.read().await.get(&msg.channel_id).cloned();
    let Some(senders) = senders else { return };

    let Some(channel) = self.resolve_channel(msg.channel_id).await else {
      log::warn!("Discord: dropping message {} in unknown channel {}", msg.id, msg.channel_id);
      return;
    };

    let Some(client) = self.weak.upgrade() else { return };
    let msg = Arc::new(msg);
    let channel = Arc::new(channel);
    let member = self.message_member(&msg).await;

    for sender in senders {
      let _ = sender.send(ChannelEvent::New(DiscordMessage::from_serenity(
        client.clone(),
        msg.clone(),
        channel.clone(),
        member.clone(),
      )));
    }
  }

  /// Edits, embed resolution, pins, … `new` is only set when the serenity message cache holds
  /// the message; otherwise the partial event is applied to our cached copy, or the message is
  /// fetched again.
  async fn message_update(&self, _: Context, _old: Option<Message>, new: Option<Message>, event: MessageUpdateEvent) {
    let Some(channel) = self.open_channel(event.channel_id).await else {
      return;
    };

    let message = match new {
      Some(message) => message,
      None => match channel.cached_message(event.id.into()).await {
        Some(cached) => {
          let mut message = (*cached).clone();
          event.apply_to_message(&mut message);
          message
        }
        None => match self.get_specific_message(event.channel_id, event.id).await {
          Some(message) => message,
          None => return,
        },
      },
    };

    self.publish_update(&channel, Arc::new(message)).await;
  }

  async fn message_delete(&self, _: Context, channel_id: ChannelId, deleted_message_id: MessageId, _guild_id: Option<GuildId>) {
    self.publish_delete(channel_id, deleted_message_id).await;
  }

  async fn message_delete_bulk(&self, _: Context, channel_id: ChannelId, ids: Vec<MessageId>, _guild_id: Option<GuildId>) {
    for id in ids {
      self.publish_delete(channel_id, id).await;
    }
  }

  async fn reaction_add(&self, _: Context, reaction: Reaction) {
    self.refresh_message_soon(reaction.channel_id, reaction.message_id).await;
  }

  async fn reaction_remove(&self, _: Context, reaction: Reaction) {
    self.refresh_message_soon(reaction.channel_id, reaction.message_id).await;
  }

  async fn reaction_remove_all(&self, _: Context, channel_id: ChannelId, message_id: MessageId) {
    self.refresh_message_soon(channel_id, message_id).await;
  }

  async fn reaction_remove_emoji(&self, _: Context, reaction: Reaction) {
    self.refresh_message_soon(reaction.channel_id, reaction.message_id).await;
  }
}
