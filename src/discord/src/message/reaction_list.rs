use crate::client::DiscordClient;
use crate::message::reaction::{DiscordMessageReaction, ReactionData};
use gpui::{div, App, IntoElement, ParentElement, RenderOnce, Styled};
use scope_chat::reaction::MessageReactionType::Normal;
use scope_chat::reaction::{MessageReaction, MessageReactionType, ReactionEmoji, ReactionList, ReactionOperation};
use serenity::all::{ChannelId, MessageId};
use std::fmt::Debug;
use std::sync::Arc;

#[derive(Clone, IntoElement)]
pub struct DiscordReactionList {
  reactions: Vec<DiscordMessageReaction>,
  message_id: MessageId,
  channel_id: ChannelId,
  client: Arc<DiscordClient>,
}

impl DiscordReactionList {
  pub fn new(reactions: Vec<DiscordMessageReaction>, channel_id: ChannelId, message_id: MessageId, client: Arc<DiscordClient>) -> Self {
    DiscordReactionList {
      reactions,
      message_id,
      channel_id,
      client,
    }
  }

  pub fn new_serenity(
    reactions: Vec<serenity::all::MessageReaction>,
    channel_id: ChannelId,
    message_id: MessageId,
    client: Arc<DiscordClient>,
  ) -> Self {
    DiscordReactionList {
      reactions: reactions.iter().map(DiscordMessageReaction::from_message).collect(),
      message_id,
      channel_id,
      client,
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
  fn render(self, _: &mut gpui::Window, _: &mut App) -> impl IntoElement {
    if self.reactions.is_empty() {
      return div();
    }

    div().flex().gap_2().children(self.reactions)
  }
}

impl Debug for DiscordReactionList {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_set().entries(self.reactions.iter()).finish()
  }
}
