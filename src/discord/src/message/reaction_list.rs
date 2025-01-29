use crate::client::DiscordClient;
use crate::message::reaction::{DiscordMessageReaction, ReactionData};
use atomic_refcell::{AtomicRef, AtomicRefCell};
use gpui::{div, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window};
use scope_chat::reaction::MessageReactionType::Normal;
use scope_chat::reaction::{MessageReaction, MessageReactionType, ReactionEmoji, ReactionList, ReactionOperation};
use serenity::all::{ChannelId, MessageId};
use std::fmt::Debug;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone)]
pub struct DiscordReactionList {
  reactions: Arc<AtomicRefCell<Vec<DiscordMessageReaction>>>,
  message_id: MessageId,
  channel_id: ChannelId,
  client: Arc<DiscordClient>,
  entity: Arc<OnceLock<Entity<RenderableReactionList>>>,
}

impl DiscordReactionList {
  pub fn new(reactions: Vec<serenity::all::MessageReaction>, channel_id: ChannelId, message_id: MessageId, client: Arc<DiscordClient>) -> Self {
    DiscordReactionList {
      reactions: Arc::new(AtomicRefCell::new(
        reactions.iter().map(|reaction| DiscordMessageReaction::new(reaction, client.clone(), message_id.clone(), channel_id.clone())).collect(),
      )),
      message_id,
      channel_id,
      client,
      entity: Arc::new(OnceLock::new()),
    }
  }
}

impl ReactionList for DiscordReactionList {
  fn get_reactions(&self) -> &Arc<AtomicRefCell<Vec<impl MessageReaction>>> {
    &self.reactions
  }

  fn increment(&mut self, emoji: &ReactionEmoji, kind: MessageReactionType, user_is_self: bool, by: isize) {
    if let Some(reaction) = self.reactions.borrow_mut().iter_mut().find(|reaction| reaction.get_emoji() == *emoji) {
      reaction.increment(kind, user_is_self, by);
      if reaction.get_count(None) == 0 {
        self.reactions.borrow_mut().retain(|reaction| reaction.get_emoji() != *emoji);
      }
    } else if by > 0 {
      let mut reaction = DiscordMessageReaction {
        data: ReactionData::Local {
          count_normal: 0,
          count_burst: 0,
          me: None,
          emoji: emoji.clone(),
          burst_colours: vec![],
        },
        client: self.client.clone(),
        message_id: self.message_id.clone(),
        channel_id: self.channel_id.clone(),
        users: Arc::new(Mutex::new(None)),
      };

      reaction.increment(kind, user_is_self, by);
      self.reactions.borrow_mut().push(reaction);
    }
  }

  fn apply(&mut self, operation: ReactionOperation, cx: &mut App) {
    match operation {
      ReactionOperation::Add(emoji, ty) => {
        self.increment(&emoji, ty, false, 1);
      }
      ReactionOperation::AddSelf(emoji, ty) => {
        self.increment(&emoji, ty, true, 1);
      }
      ReactionOperation::Remove(emoji) => {
        self.increment(&emoji, Normal, false, -1);
      }
      ReactionOperation::RemoveSelf(emoji) => {
        self.increment(&emoji, Normal, true, -1);
      }
      ReactionOperation::RemoveAll => {
        self.reactions.borrow_mut().clear();
      }
      ReactionOperation::RemoveEmoji(emoji) => {
        self.reactions.borrow_mut().retain(|reaction| reaction.get_emoji() != emoji);
      }
      ReactionOperation::SetMembers(emoji, members) => {
        if let Some(reaction) = self.reactions.borrow_mut().iter_mut().find(|reaction| reaction.get_emoji() == emoji) {
          let mut reactions = reaction.users.lock().unwrap();
          reactions.replace(members);
          self.entity.get().as_ref().map(|entity| cx.notify(entity.entity_id()));
        }
      }
    }
  }

  fn get_content(&self, cx: &mut App) -> impl IntoElement {
    self.entity.get_or_init(|| cx.new(|_| RenderableReactionList::new(self.clone()))).clone()
  }
}

impl Debug for DiscordReactionList {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_set().entries(self.reactions.borrow().iter()).finish()
  }
}

struct RenderableReactionList {
  list: DiscordReactionList,
}

impl RenderableReactionList {
  fn new(list: DiscordReactionList) -> Self {
    Self { list }
  }
}

impl Render for RenderableReactionList {
  fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
    if self.list.reactions.borrow().is_empty() {
      return div();
    }

    let reactions: AtomicRef<Vec<DiscordMessageReaction>> = self.list.reactions.borrow();
    div().flex().gap_2().children(reactions.clone())
  }
}
