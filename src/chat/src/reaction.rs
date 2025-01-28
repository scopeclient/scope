use std::fmt::{Debug, Formatter};
use gpui::{IntoElement, Rgba};

pub type ReactionEvent<T> = (T, ReactionOperation);

#[derive(Copy, Clone, Debug)]
pub enum MessageReactionType {
  Normal,
  Burst,
}

#[derive(Clone, PartialEq)]
pub enum ReactionEmoji {
  Simple(String),
  Custom { url: String, animated: bool, name: Option<String>, id: u64 },
}

impl Debug for ReactionEmoji {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      ReactionEmoji::Simple(s) => write!(f, "{}", s),
      ReactionEmoji::Custom { name, .. } => write!(f, ":{}:", name.clone().unwrap_or("<unknown>".to_string())),
    }
  }
}

pub trait MessageReaction: IntoElement {

  fn get_count(&self, kind: Option<MessageReactionType>) -> u64;
  fn get_self_reaction(&self) -> Option<MessageReactionType>;
  fn get_emoji(&self) -> ReactionEmoji;
  fn get_burst_colors(&self) -> Vec<Rgba>;
  fn increment(&mut self, kind: MessageReactionType, user_is_self: bool, by: isize);
}

#[derive(Clone, Debug)]
pub enum ReactionOperation {
  Add(ReactionEmoji, MessageReactionType),
  AddSelf(ReactionEmoji, MessageReactionType),
  Remove(ReactionEmoji),
  RemoveSelf(ReactionEmoji),
  RemoveAll,
  RemoveEmoji(ReactionEmoji),
}

pub trait ReactionList: IntoElement {
  fn get_reactions(&self) -> &Vec<impl MessageReaction>;
  fn get_reaction(&self, emoji: &ReactionEmoji) -> Option<&impl MessageReaction>;
  fn increment(&mut self, emoji: &ReactionEmoji, kind: MessageReactionType, user_is_self: bool, by: isize);
  fn apply(&mut self, operation: ReactionOperation);
}
