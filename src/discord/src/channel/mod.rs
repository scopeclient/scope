use std::sync::Arc;

use scope_backend_cache::async_list::{AsyncListCache, refcacheslice::Exists};
use scope_chat::{
  async_list::{AsyncList, AsyncListIndex, AsyncListItem, AsyncListResult},
  channel::{Channel, ChannelEvent},
};
use scope_rich::{Emoji, ReplyRef};
use serenity::all::{ChannelId, GetMessages, Message, MessageId};
use tokio::sync::{Mutex, Semaphore, broadcast};

use crate::{
  client::DiscordClient,
  dm,
  message::{DiscordMessage, rich},
  snowflake::Snowflake,
};

pub struct DiscordChannel {
  channel: Arc<serenity::model::channel::Channel>,

  receiver: broadcast::Receiver<ChannelEvent<DiscordMessage>>,
  client: Arc<DiscordClient>,
  cache: Arc<Mutex<AsyncListCache<DiscordMessage>>>,
  blocker: Semaphore,
}

impl DiscordChannel {
  pub(crate) async fn new(client: Arc<DiscordClient>, channel_id: ChannelId) -> Self {
    let (sender, receiver) = broadcast::channel(10);

    client.add_channel_message_sender(channel_id, sender).await;

    let channel = Arc::new(client.resolve_channel(channel_id).await.unwrap_or_else(|| dm::placeholder_channel(channel_id, &client.own_user())));

    DiscordChannel {
      channel,
      receiver,
      client,
      cache: Arc::new(Mutex::new(AsyncListCache::new())),
      blocker: Semaphore::new(1),
    }
  }

  /// Send `content` (as a reply to `reply_to`, when given) in the background and return the
  /// optimistic message; the gateway echo replaces it by nonce.
  fn send(&self, content: String, nonce: String, reply_to: Option<Snowflake>) -> DiscordMessage {
    let client = self.client.clone();
    let channel_id = self.channel.id();
    let sent_content = content.clone();
    let sent_nonce = nonce.clone();
    let reference = reply_to.map(|id| MessageId::new(id.0));

    let reply = reply_to.map(|id| self.reply_ref(id));
    let pending = DiscordMessage::pending(self.client.clone(), self.channel.clone(), content, nonce, reply);
    let pending_id = pending.get_list_identifier();

    tokio::spawn(async move {
      // A rejected send (usually missing permissions) retracts the optimistic
      // bubble instead of leaving it half-transparent forever.
      if !client.send_message(channel_id, sent_content, sent_nonce, reference).await {
        client.publish(channel_id, ChannelEvent::Deleted(pending_id)).await;
      }
    });

    pending
  }

  /// Reply header for `message_id`: built from the cached message when it is paged in, else a
  /// placeholder until Discord echoes the sent message back with the full reference.
  fn reply_ref(&self, message_id: Snowflake) -> ReplyRef {
    // Sending is synchronous; if a page load holds the cache the placeholder will do.
    let cached = self.cache.try_lock().ok().and_then(|cache| cache.find(&message_id));

    match cached.as_ref().and_then(DiscordMessage::serenity) {
      Some(referenced) => {
        let cache = &self.client.discord().cache;
        rich::reply_ref_to(referenced, cache, rich::message_guild_id(referenced, &self.channel))
      }
      None => rich::unresolved_reply_ref(message_id.0),
    }
  }

  /// The serenity message cached for `id`, when it is paged in.
  pub(crate) async fn cached_message(&self, id: Snowflake) -> Option<Arc<Message>> {
    self.cache.lock().await.find(&id).as_ref().and_then(DiscordMessage::serenity).cloned()
  }

  /// Take `msg` as the new state of a message; the cached copy (if paged in) is replaced.
  /// Returns the message to publish as updated.
  pub(crate) async fn message_updated(&self, msg: Arc<Message>) -> DiscordMessage {
    let cached = self.cache.lock().await.find(&msg.id.into());

    match cached {
      Some(cached) => {
        let updated = cached.with_serenity(msg);
        self.cache.lock().await.replace(updated.clone());
        updated
      }
      // Not paged in (it arrived live, or is outside the loaded window): nothing to keep in step.
      None => DiscordMessage::load_serenity(self.client.clone(), self.channel.clone(), msg).await,
    }
  }

  /// Forget a deleted message; its neighbours become adjacent.
  pub(crate) async fn message_deleted(&self, id: Snowflake) {
    self.cache.lock().await.remove(&id);
  }
}

impl Channel for DiscordChannel {
  type Message = DiscordMessage;
  type Identifier = Snowflake;

  fn get_receiver(&self) -> broadcast::Receiver<ChannelEvent<Self::Message>> {
    self.receiver.resubscribe()
  }

  fn send_message(&self, content: String, nonce: String) -> DiscordMessage {
    self.send(content, nonce, None)
  }

  fn send_reply(&self, content: String, nonce: String, reply_to: Self::Identifier) -> DiscordMessage {
    self.send(content, nonce, Some(reply_to))
  }

  fn edit_message(&self, message: Self::Identifier, content: String) {
    let client = self.client.clone();
    let channel_id = self.channel.id();

    tokio::spawn(async move { client.edit_message(channel_id, MessageId::new(message.0), content).await });
  }

  fn delete_message(&self, message: Self::Identifier) {
    let client = self.client.clone();
    let channel_id = self.channel.id();

    tokio::spawn(async move { client.delete_message(channel_id, MessageId::new(message.0)).await });
  }

  fn add_reaction(&self, message: Self::Identifier, emoji: Emoji) {
    let client = self.client.clone();
    let channel_id = self.channel.id();
    let reaction = rich::reaction_type(&emoji);

    tokio::spawn(async move { client.add_reaction(channel_id, MessageId::new(message.0), reaction).await });
  }

  fn remove_reaction(&self, message: Self::Identifier, emoji: Emoji) {
    let client = self.client.clone();
    let channel_id = self.channel.id();
    let reaction = rich::reaction_type(&emoji);

    tokio::spawn(async move { client.remove_reaction(channel_id, MessageId::new(message.0), reaction).await });
  }

  fn typing(&self) {
    let client = self.client.clone();
    let channel_id = self.channel.id();

    tokio::spawn(async move { client.broadcast_typing(channel_id).await });
  }

  fn get_identifier(&self) -> Self::Identifier {
    self.channel.id().into()
  }
}

/// Discord allows up to 100 per request; bigger pages = fewer requests.
const DISCORD_MESSAGE_BATCH_SIZE: u8 = 100;

impl AsyncList for DiscordChannel {
  type Content = DiscordMessage;

  async fn bounded_at_top_by(&self) -> Option<Snowflake> {
    self.cache.lock().await.bounded_at_top_by()
  }

  async fn bounded_at_bottom_by(&self) -> Option<Snowflake> {
    let cached = self.cache.lock().await.bounded_at_bottom_by();

    if let Some(v) = cached {
      return Some(v);
    }

    self.client.get_messages(self.channel.id(), GetMessages::new().limit(1)).await.first().map(|v| Snowflake(v.id.get()))
  }

  async fn find(&self, identifier: &Snowflake) -> Option<Self::Content> {
    let cached = self.cache.lock().await.find(identifier);

    if let Some(v) = cached {
      return Some(v);
    }

    let result = self.client.get_specific_message(self.channel.id(), MessageId::new(identifier.0)).await?;

    Some(DiscordMessage::load_serenity(self.client.clone(), self.channel.clone(), Arc::new(result)).await)
  }

  /// Serve `index` from the cache, fetching a full page from Discord on a miss.
  ///
  /// Discord returns pages newest → oldest. A SHORT page means that edge of
  /// history is final; a full page means there is (probably) more.
  async fn get(&self, index: AsyncListIndex<Snowflake>) -> Option<AsyncListResult<Self::Content>> {
    let _permit = self.blocker.acquire().await;
    let mut lock = self.cache.lock().await;

    match lock.get(index) {
      Exists::Yes(v) => return Some(v),
      Exists::No => return None,
      Exists::Unknown => {}
    }

    let page_of = |builder: GetMessages| self.client.get_messages(self.channel.id(), builder.limit(DISCORD_MESSAGE_BATCH_SIZE));

    match index {
      AsyncListIndex::RelativeToTop(_) => {
        log::warn!("paging relative to the top of history is not supported");
        None
      }

      AsyncListIndex::RelativeToBottom(offset) => {
        if offset != 0 {
          log::warn!("paging relative to the bottom only supports offset 0");
          return None;
        }

        // Newest page. The newest message is the bottom bound by definition;
        // a short page also makes the oldest one the top bound.
        let page = page_of(GetMessages::new()).await;
        let exhausted = page.len() < DISCORD_MESSAGE_BATCH_SIZE as usize;
        let count = page.len();

        let mut newest = None;
        let mut previous: Option<Snowflake> = None;

        for (position, message) in page.into_iter().enumerate() {
          let value = DiscordMessage::load_serenity(self.client.clone(), self.channel.clone(), Arc::new(message)).await;
          let id = value.get_list_identifier();
          let is_oldest_of_history = exhausted && position == count - 1;

          match previous {
            None => {
              // The cache only inserts relative to known items; the newest
              // message of the first page is appended as the bottom bound.
              lock.append_bottom(value.clone());
              if is_oldest_of_history {
                lock.mark_top_bound(&id);
              }
              newest = Some(value);
            }
            Some(prev) => lock.insert(AsyncListIndex::Before(prev), value, is_oldest_of_history, false),
          }

          previous = Some(id);
        }

        let newest = newest?;
        Some(AsyncListResult {
          content: newest,
          is_top: exhausted && count == 1,
          is_bottom: true,
        })
      }

      AsyncListIndex::Before(anchor) => {
        // Older history: page is newest → oldest, all older than `anchor`.
        let page = page_of(GetMessages::new().before(MessageId::new(anchor.0))).await;
        let exhausted = page.len() < DISCORD_MESSAGE_BATCH_SIZE as usize;
        let count = page.len();
        log::debug!("Discord: {count} older messages before {anchor:?} (exhausted: {exhausted})");

        let mut first = None;
        let mut previous = anchor;

        for (position, message) in page.into_iter().enumerate() {
          let value = DiscordMessage::load_serenity(self.client.clone(), self.channel.clone(), Arc::new(message)).await;
          let id = value.get_list_identifier();
          let is_oldest_of_history = exhausted && position == count - 1;

          lock.insert(AsyncListIndex::Before(previous), value.clone(), is_oldest_of_history, false);

          if position == 0 {
            first = Some(value);
          }

          previous = id;
        }

        if count == 0 {
          // Nothing older: `anchor` itself is the top of history.
          lock.mark_top_bound(&anchor);
          return None;
        }

        Some(AsyncListResult {
          content: first?,
          is_top: exhausted && count == 1,
          is_bottom: false,
        })
      }

      AsyncListIndex::After(anchor) => {
        // Newer messages: request is oldest-bounded; page still newest → oldest,
        // so walk it in reverse to chain upwards from `anchor`.
        let page = page_of(GetMessages::new().after(MessageId::new(anchor.0))).await;
        let exhausted = page.len() < DISCORD_MESSAGE_BATCH_SIZE as usize;
        let count = page.len();

        let mut first = None;
        let mut previous = anchor;

        for (position, message) in page.into_iter().rev().enumerate() {
          let value = DiscordMessage::load_serenity(self.client.clone(), self.channel.clone(), Arc::new(message)).await;
          let id = value.get_list_identifier();
          let is_newest_of_history = exhausted && position == count - 1;

          lock.insert(AsyncListIndex::After(previous), value.clone(), false, is_newest_of_history);

          if position == 0 {
            first = Some(value);
          }

          previous = id;
        }

        if count == 0 {
          lock.mark_bottom_bound(&anchor);
          return None;
        }

        Some(AsyncListResult {
          content: first?,
          is_top: false,
          is_bottom: exhausted && count == 1,
        })
      }
    }
  }
}

impl Clone for DiscordChannel {
  fn clone(&self) -> Self {
    Self {
      channel: self.channel.clone(),
      receiver: self.receiver.resubscribe(),
      client: self.client.clone(),
      cache: self.cache.clone(),
      blocker: Semaphore::new(1),
    }
  }
}
