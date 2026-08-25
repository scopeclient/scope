use std::{
  future::Future,
  sync::{Arc, RwLock},
  time::{Duration, Instant},
};

use chrono::Utc;
use scope_chat::{
  async_list::{AsyncList, AsyncListIndex, AsyncListResult},
  channel::{Channel, ChannelEvent},
  message::Message as _,
  nav::Id,
};
use scope_rich::{Emoji, ReplyRef};
use tokio::sync::broadcast;

use crate::message::DemoMessage;

/// How long the pretend server takes to acknowledge each kind of request.
pub const SEND_LATENCY: Duration = Duration::from_millis(250);
pub const EDIT_LATENCY: Duration = Duration::from_millis(150);
pub const DELETE_LATENCY: Duration = Duration::from_millis(150);
pub const REACTION_LATENCY: Duration = Duration::from_millis(120);

#[derive(Clone)]
pub struct DemoChannel {
  pub id: Id,
  pub guild: Option<Id>,
  pub name: String,
  /// Oldest first.
  history: Arc<RwLock<Vec<DemoMessage>>>,
  /// What the live feed posted and when, so it can later take something back.
  posted: Arc<RwLock<Vec<(Id, Instant)>>>,
  live: broadcast::Sender<ChannelEvent<DemoMessage>>,
}

impl DemoChannel {
  pub fn new(id: Id, guild: Option<Id>, name: impl Into<String>, history: Vec<DemoMessage>) -> Self {
    DemoChannel {
      id,
      guild,
      name: name.into(),
      history: Arc::new(RwLock::new(history)),
      posted: Arc::new(RwLock::new(Vec::new())),
      live: broadcast::channel(64).0,
    }
  }

  /// Append a message "from the server" and notify live listeners.
  pub fn post(&self, message: DemoMessage) {
    self.history.write().unwrap().push(message.clone());
    self.posted.write().unwrap().push((message.id, Instant::now()));
    let _ = self.live.send(ChannelEvent::New(message));
  }

  pub fn last_author(&self) -> Option<Id> {
    self.history.read().unwrap().last().map(|m| m.author.id)
  }

  pub fn newest(&self) -> Option<DemoMessage> {
    self.history.read().unwrap().last().cloned()
  }

  /// Read-only look at the history, oldest first.
  pub fn with_history<R>(&self, f: impl FnOnce(&[DemoMessage]) -> R) -> R {
    f(&self.history.read().unwrap())
  }

  /// Live-feed posts younger than `max_age`, oldest first.
  pub fn recent_posts(&self, max_age: Duration) -> Vec<Id> {
    let now = Instant::now();
    let mut posted = self.posted.write().unwrap();
    posted.retain(|(_, at)| now.duration_since(*at) <= max_age);
    posted.iter().map(|(id, _)| *id).collect()
  }

  // ---- server-side primitives --------------------------------------------------
  // The `Channel` impl (what the user does) and the live feed (what everyone else
  // does) both go through these, so the two are indistinguishable to the UI.

  /// Replace a message's text; `false` when there is no such message.
  pub fn edit(&self, id: Id, content: impl Into<String>) -> bool {
    let (content, now) = (content.into(), Utc::now());
    self.update_after(id, EDIT_LATENCY, |m| {
      m.edit(content, now);
      true
    })
  }

  /// Remove a message, orphaning any replies that quoted it.
  pub fn delete(&self, id: Id) -> bool {
    let Some(orphans) = delete_from(&mut self.history.write().unwrap(), id) else {
      return false;
    };
    self.posted.write().unwrap().retain(|(posted, _)| *posted != id);
    self.broadcast_after(DELETE_LATENCY, ChannelEvent::Deleted(id));
    for orphan in orphans {
      self.broadcast_after(DELETE_LATENCY, ChannelEvent::Updated(orphan));
    }
    true
  }

  /// `me` when the signed-in user is the one reacting.
  pub fn react(&self, id: Id, emoji: Emoji, me: bool) -> bool {
    self.update_after(id, REACTION_LATENCY, |m| m.react(emoji, me))
  }

  pub fn unreact(&self, id: Id, emoji: &Emoji, me: bool) -> bool {
    self.update_after(id, REACTION_LATENCY, |m| m.unreact(emoji, me))
  }

  /// Apply `change` to one message and, after `latency`, tell listeners.
  /// `false` when the message is unknown or `change` reports nothing changed.
  fn update_after(&self, id: Id, latency: Duration, change: impl FnOnce(&mut DemoMessage) -> bool) -> bool {
    let updated = {
      let mut history = self.history.write().unwrap();
      let Some(message) = history.iter_mut().find(|m| m.id == id) else {
        return false;
      };
      if !change(message) {
        return false;
      }
      message.clone()
    };
    self.broadcast_after(latency, ChannelEvent::Updated(updated));
    true
  }

  fn broadcast_after(&self, latency: Duration, event: ChannelEvent<DemoMessage>) {
    let live = self.live.clone();
    tokio::spawn(async move {
      tokio::time::sleep(latency).await;
      let _ = live.send(event);
    });
  }

  /// Queue an optimistic send: history gets the confirmed copy right away (so
  /// paging sees it), listeners hear about it after the round trip.
  fn send(&self, pending: DemoMessage) -> DemoMessage {
    let confirmed = pending.clone().confirmed(Id(rand::random()));
    self.history.write().unwrap().push(confirmed.clone());
    self.broadcast_after(SEND_LATENCY, ChannelEvent::New(confirmed));
    pending
  }
}

/// Drop `id` from `history` and flip the quote on every reply that pointed at it.
/// Returns the replies that changed, or `None` when `id` was not there.
pub fn delete_from(history: &mut Vec<DemoMessage>, id: Id) -> Option<Vec<DemoMessage>> {
  let index = history.iter().position(|m| m.id == id)?;
  history.remove(index);

  let orphans = history
    .iter_mut()
    .filter(|m| m.reply_target() == Some(id))
    .filter_map(|m| m.orphan_reply().then(|| m.clone()))
    .collect();
  Some(orphans)
}

/// What a reply shows when the message it quotes is already gone.
fn deleted_reply(id: Id) -> ReplyRef {
  ReplyRef {
    message_id: Some(id.0),
    author_name: "Unknown".into(),
    author_avatar: None,
    snippet: String::new(),
    deleted: true,
  }
}

impl Channel for DemoChannel {
  type Message = DemoMessage;
  type Identifier = Id;

  fn get_receiver(&self) -> broadcast::Receiver<ChannelEvent<Self::Message>> {
    self.live.subscribe()
  }

  fn send_message(&self, content: String, nonce: String) -> Self::Message {
    self.send(DemoMessage::pending(content, nonce))
  }

  fn send_reply(&self, content: String, nonce: String, reply_to: Self::Identifier) -> Self::Message {
    let reply = self
      .with_history(|history| history.iter().find(|m| m.id == reply_to).map(DemoMessage::reply_ref))
      .unwrap_or_else(|| deleted_reply(reply_to));
    self.send(DemoMessage::pending_reply(content, nonce, reply))
  }

  fn edit_message(&self, message: Self::Identifier, content: String) {
    let own = self.with_history(|history| history.iter().find(|m| m.id == message).map(DemoMessage::is_own));
    match own {
      Some(true) => {
        self.edit(message, content);
      }
      Some(false) => log::warn!("demo edit_message({message:?}): refusing to edit someone else's message"),
      None => log::warn!("demo edit_message({message:?}): no such message"),
    }
  }

  fn delete_message(&self, message: Self::Identifier) {
    if !self.delete(message) {
      log::warn!("demo delete_message({message:?}): no such message");
    }
  }

  fn add_reaction(&self, message: Self::Identifier, emoji: Emoji) {
    if !self.react(message, emoji.clone(), true) {
      log::debug!("demo add_reaction({message:?}, {}): nothing to do", emoji.label());
    }
  }

  fn remove_reaction(&self, message: Self::Identifier, emoji: Emoji) {
    if !self.unreact(message, &emoji, true) {
      log::debug!("demo remove_reaction({message:?}, {}): nothing to do", emoji.label());
    }
  }

  fn typing(&self) {
    log::debug!("demo typing in #{}", self.name);
  }

  fn get_identifier(&self) -> Self::Identifier {
    self.id
  }
}

impl DemoChannel {
  fn result(history: &[DemoMessage], index: usize) -> AsyncListResult<DemoMessage> {
    AsyncListResult {
      content: history[index].clone(),
      is_top: index == 0,
      is_bottom: index + 1 == history.len(),
    }
  }
}

impl AsyncList for DemoChannel {
  type Content = DemoMessage;

  fn bounded_at_top_by(&self) -> impl Future<Output = Option<Id>> {
    let first = self.history.read().unwrap().first().map(|m| m.id);
    async move { first }
  }

  fn bounded_at_bottom_by(&self) -> impl Future<Output = Option<Id>> {
    let last = self.history.read().unwrap().last().map(|m| m.id);
    async move { last }
  }

  fn find(&self, identifier: &Id) -> impl Future<Output = Option<DemoMessage>> {
    let found = self.history.read().unwrap().iter().find(|m| m.id == *identifier).cloned();
    async move { found }
  }

  fn get(&self, index: AsyncListIndex<Id>) -> impl Future<Output = Option<AsyncListResult<DemoMessage>>> + Send {
    let result = {
      let history = self.history.read().unwrap();
      let position = |id: Id| history.iter().position(|m| m.id == id);

      match index {
        AsyncListIndex::RelativeToTop(n) => (n < history.len()).then(|| Self::result(&history, n)),
        AsyncListIndex::RelativeToBottom(n) => history.len().checked_sub(n + 1).map(|i| Self::result(&history, i)),
        AsyncListIndex::Before(id) => position(id).and_then(|i| i.checked_sub(1)).map(|i| Self::result(&history, i)),
        AsyncListIndex::After(id) => position(id).map(|i| i + 1).filter(|&i| i < history.len()).map(|i| Self::result(&history, i)),
      }
    };

    async move {
      // A little latency so loading states are visible.
      tokio::time::sleep(Duration::from_millis(120)).await;
      result
    }
  }
}

#[cfg(test)]
mod tests {
  use chrono::Utc;

  use super::*;
  use crate::data;

  fn history() -> Vec<DemoMessage> {
    let now = Utc::now();
    let first = DemoMessage::new(Id(1), Id(2), "ship it", now);
    let reply = DemoMessage::pending_reply("yes".into(), "n".into(), first.reply_ref()).confirmed(Id(2));
    vec![first, reply, DemoMessage::new(Id(3), data::SELF_ID, "done", now)]
  }

  fn ids(history: &[DemoMessage]) -> Vec<Id> {
    history.iter().map(|m| m.id).collect()
  }

  #[test]
  fn delete_from_removes_and_orphans_replies() {
    let mut history = history();
    let orphans = delete_from(&mut history, Id(1)).expect("message exists");
    assert_eq!(ids(&history), vec![Id(2), Id(3)]);
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].id, Id(2));
    assert!(orphans[0].body().reply.as_ref().is_some_and(|r| r.deleted));
    assert!(history[0].body().reply.as_ref().is_some_and(|r| r.deleted), "history holds the orphaned copy");
  }

  #[test]
  fn delete_from_unknown_id_is_none() {
    let mut history = history();
    assert!(delete_from(&mut history, Id(99)).is_none());
    assert_eq!(ids(&history), vec![Id(1), Id(2), Id(3)]);
  }

  async fn next(receiver: &mut broadcast::Receiver<ChannelEvent<DemoMessage>>) -> ChannelEvent<DemoMessage> {
    tokio::time::timeout(Duration::from_secs(2), receiver.recv()).await.expect("event in time").expect("channel open")
  }

  #[tokio::test]
  async fn edit_updates_history_and_broadcasts() {
    let channel = DemoChannel::new(Id(100), None, "general", history());
    let mut receiver = channel.get_receiver();

    channel.edit_message(Id(3), "done!".into());
    assert_eq!(channel.with_history(|h| h[2].content.clone()), "done!", "history changes immediately");

    match next(&mut receiver).await {
      ChannelEvent::Updated(m) => {
        assert_eq!(m.id, Id(3));
        assert_eq!(m.content, "done!");
        assert!(m.is_edited());
      }
      _ => panic!("expected Updated"),
    }
  }

  #[tokio::test]
  async fn edit_refuses_other_peoples_messages() {
    let channel = DemoChannel::new(Id(100), None, "general", history());
    channel.edit_message(Id(1), "hijacked".into());
    assert_eq!(channel.with_history(|h| h[0].content.clone()), "ship it");
  }

  #[tokio::test]
  async fn delete_broadcasts_deleted_then_orphans() {
    let channel = DemoChannel::new(Id(100), None, "general", history());
    let mut receiver = channel.get_receiver();

    channel.delete_message(Id(1));
    assert_eq!(channel.with_history(ids), vec![Id(2), Id(3)]);

    assert!(matches!(next(&mut receiver).await, ChannelEvent::Deleted(Id(1))));
    match next(&mut receiver).await {
      ChannelEvent::Updated(m) => assert_eq!(m.id, Id(2)),
      _ => panic!("expected Updated for the orphaned reply"),
    }
  }

  #[tokio::test]
  async fn reactions_toggle_through_the_channel() {
    let channel = DemoChannel::new(Id(100), None, "general", history());
    let mut receiver = channel.get_receiver();
    let emoji = Emoji::Unicode("👍".into());

    channel.add_reaction(Id(1), emoji.clone());
    match next(&mut receiver).await {
      ChannelEvent::Updated(m) => assert_eq!(m.body().reactions[0].count, 1),
      _ => panic!("expected Updated"),
    }

    channel.remove_reaction(Id(1), emoji.clone());
    match next(&mut receiver).await {
      ChannelEvent::Updated(m) => assert!(m.body().reactions.is_empty()),
      _ => panic!("expected Updated"),
    }

    // Removing again has nothing to undo, so nothing is broadcast.
    channel.remove_reaction(Id(1), emoji);
    assert!(tokio::time::timeout(Duration::from_millis(300), receiver.recv()).await.is_err());
  }

  #[tokio::test]
  async fn send_reply_quotes_the_target() {
    let channel = DemoChannel::new(Id(100), None, "general", history());
    let mut receiver = channel.get_receiver();

    let pending = channel.send_reply("agreed".into(), "nonce-1".into(), Id(1));
    assert!(pending.pending);
    assert!(pending.is_own());
    let quote = pending.body().reply.clone().expect("pending carries the quote");
    assert_eq!(quote.snippet, "ship it");
    assert_eq!(quote.author_name, "zach");

    match next(&mut receiver).await {
      ChannelEvent::New(m) => {
        assert!(!m.pending);
        assert_eq!(m.nonce.as_deref(), Some("nonce-1"));
        assert_eq!(m.body().reply, Some(quote));
        assert_eq!(m.body().kind, scope_rich::MessageKind::Reply);
      }
      _ => panic!("expected New"),
    }
  }

  #[tokio::test]
  async fn send_reply_to_a_missing_message_is_marked_deleted() {
    let channel = DemoChannel::new(Id(100), None, "general", history());
    let pending = channel.send_reply("?".into(), "nonce-2".into(), Id(404));
    assert!(pending.body().reply.is_some_and(|r| r.deleted));
  }

  #[tokio::test]
  async fn recent_posts_tracks_live_feed_only() {
    let channel = DemoChannel::new(Id(100), None, "general", history());
    channel.post(DemoMessage::new(Id(50), Id(2), "hey", Utc::now()));
    channel.send_message("mine".into(), "n".into());
    assert_eq!(channel.recent_posts(Duration::from_secs(60)), vec![Id(50)]);

    channel.delete(Id(50));
    assert!(channel.recent_posts(Duration::from_secs(60)).is_empty());
  }
}
