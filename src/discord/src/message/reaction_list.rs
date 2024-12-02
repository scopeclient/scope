use crate::message::reaction::{DiscordMessageReaction, ReactionData};
use gpui::{div, IntoElement, ParentElement, RenderOnce, Styled, WindowContext};
use scope_chat::reaction::MessageReactionType::Normal;
use scope_chat::reaction::{MessageReaction, MessageReactionType, ReactionEmoji, ReactionList, ReactionOperation};

#[derive(Clone, Debug, Default, IntoElement)]
pub struct DiscordReactionList {
  reactions: Vec<DiscordMessageReaction>,
}

impl DiscordReactionList {
  pub fn new(reactions: Vec<DiscordMessageReaction>) -> Self {
    DiscordReactionList { reactions }
  }
}

impl From<&Vec<serenity::all::MessageReaction>> for DiscordReactionList {
  fn from(reactions: &Vec<serenity::all::MessageReaction>) -> Self {
    DiscordReactionList {
      reactions: reactions.iter().map(DiscordMessageReaction::from_message).collect(),
    }
  }
}

impl ReactionList for DiscordReactionList {
  fn get_reactions(&self) -> &Vec<impl MessageReaction> {
    &self.reactions
  }

  fn get_reaction(&self, emoji: &ReactionEmoji) -> Option<&impl MessageReaction> {
    self.reactions.iter().find(|reaction| reaction.get_emoji() == *emoji)
  }

  fn increment(&mut self, emoji: &ReactionEmoji, kind: MessageReactionType, user_is_self: bool, by: isize) {
    if let Some(reaction) = self.reactions.iter_mut().find(|reaction| reaction.get_emoji() == *emoji) {
      reaction.increment(kind, user_is_self, by);
      if reaction.get_count(None) == 0 {
        self.reactions.retain(|reaction| reaction.get_emoji() != *emoji);
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
      };

      reaction.increment(kind, user_is_self, by);
      self.reactions.push(reaction);
    }
  }

  fn apply(&mut self, operation: ReactionOperation) {
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
        self.reactions.clear();
      }
      ReactionOperation::RemoveEmoji(emoji) => {
        self.reactions.retain(|reaction| reaction.get_emoji() != emoji);
      }
    }
  }
}

impl RenderOnce for DiscordReactionList {
  fn render(self, _cx: &mut WindowContext) -> impl IntoElement {
    if self.reactions.is_empty() {
      return div();
    }

    div()
        .flex()
        .gap_2()
        .children(self.reactions)
  }
}
