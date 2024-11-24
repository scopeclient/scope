use gpui::Rgba;

#[derive(Copy, Clone, Debug)]
pub enum MessageReactionType {
  Normal,
  Burst,
}

#[derive(Clone, Debug)]
pub enum ReactionEmoji {
  Simple(String),
  Custom { url: String, animated: bool, name: Option<String> },
}

pub trait MessageReaction {
  fn get_count(&self, kind: Option<MessageReactionType>) -> u64;
  fn get_self_reaction(&self) -> Option<MessageReactionType>;
  fn get_emoji(&self) -> ReactionEmoji;
  fn get_burst_colors(&self) -> Vec<Rgba>;
}
