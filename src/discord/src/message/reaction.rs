use gpui::Rgba;
use scope_chat::reaction::{MessageReaction, MessageReactionType, ReactionEmoji, ReactionList, ReactionOperation};
use serenity::all::ReactionType;
use MessageReactionType::Burst;
use scope_chat::reaction::MessageReactionType::Normal;

#[derive(Clone, Debug)]
pub enum ReactionData {
  Message(serenity::all::MessageReaction),
  Local {
    count_normal: u64,
    count_burst: u64,
    me: Option<MessageReactionType>,
    emoji: ReactionEmoji,
    burst_colours: Vec<Rgba>,
  },
}

impl ReactionData {
  fn count(&self) -> u64 {
    match self {
      ReactionData::Message(reaction) => reaction.count,
      ReactionData::Local { count_normal, .. } => *count_normal,
    }
  }

  fn count_burst(&self) -> u64 {
    match self {
      ReactionData::Message(reaction) => reaction.count,
      ReactionData::Local { count_burst, .. } => *count_burst,
    }
  }
}

#[derive(Clone, Debug)]
pub struct DiscordMessageReaction {
  pub data: ReactionData,
}

impl DiscordMessageReaction {
  pub fn from_message(reaction: &serenity::all::MessageReaction) -> Self {
    DiscordMessageReaction {
      data: ReactionData::Message(reaction.clone()),
    }
  }

  fn use_local(&mut self) {
    let (count_normal, count_burst) = match &self.data {
      ReactionData::Message(reaction) => (reaction.count_details.normal, reaction.count_details.burst),
      ReactionData::Local {
        count_normal, count_burst, ..
      } => (*count_normal, *count_burst),
    };
    let me = self.get_self_reaction();
    let emoji = self.get_emoji();
    let burst_colours = self.get_burst_colors();
    self.data = ReactionData::Local {
      count_normal,
      count_burst,
      me,
      emoji,
      burst_colours,
    }
  }
}

impl MessageReaction for DiscordMessageReaction {
  fn get_count(&self, kind: Option<MessageReactionType>) -> u64 {
    match kind {
      Some(Burst) => self.data.count_burst(),
      Some(Normal) => self.data.count(),
      None => self.data.count_burst() + self.data.count(),
    }
  }

  fn get_self_reaction(&self) -> Option<MessageReactionType> {
    match &self.data {
      ReactionData::Message(reaction) => {
        if reaction.me {
          Some(Normal)
        } else if reaction.me_burst {
          Some(Burst)
        } else {
          None
        }
      }
      ReactionData::Local { me, .. } => me.clone(),
    }
  }

  fn get_emoji(&self) -> ReactionEmoji {
    match &self.data {
      ReactionData::Message(reaction) => discord_reaction_to_emoji(&reaction.reaction_type),
      ReactionData::Local { emoji, .. } => emoji.clone(),
    }
  }

  fn get_burst_colors(&self) -> Vec<Rgba> {
    match &self.data {
      ReactionData::Message(reaction) => reaction
        .burst_colours
        .iter()
        .map(|color| Rgba {
          r: color.r() as f32,
          g: color.g() as f32,
          b: color.b() as f32,
          a: 1f32,
        })
        .collect(),
      ReactionData::Local { burst_colours, .. } => burst_colours.clone(),
    }
  }

  fn increment(&mut self, kind: MessageReactionType, user_is_self: bool, by: isize) {
    self.use_local();
    match kind {
      Burst => {
        if let ReactionData::Local { count_burst, .. } = &mut self.data {
          *count_burst = (*count_burst as isize + by).max(0) as u64;
        }
      }
      Normal => {
        if let ReactionData::Local { count_normal, .. } = &mut self.data {
          *count_normal = (*count_normal as isize + by).max(0) as u64;
        }
      }
    }

    if user_is_self {
      if let ReactionData::Local { me, .. } = &mut self.data {
        if by < 0 {
          *me = None;
        } else {
          *me = Some(kind);
        }
      }
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct DiscordReactionList {
  reactions: Vec<DiscordMessageReaction>,
}

impl DiscordReactionList {
  pub fn new(reactions: Vec<DiscordMessageReaction>) -> Self {
    DiscordReactionList { reactions }
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

pub fn discord_reaction_to_emoji(reaction: &serenity::all::ReactionType) -> ReactionEmoji {
  match reaction {
    ReactionType::Custom { animated, id, name } => ReactionEmoji::Custom {
      url: format!("https://cdn.discordapp.com/emojis/{}.png", id),
      animated: *animated,
      name: name.clone(),
    },
    ReactionType::Unicode(character) => ReactionEmoji::Simple(character.clone()),
    ty => {
      eprintln!("Unsupported reaction type: {:?}", ty);
      ReactionEmoji::Simple("❓".to_string())
    }
  }
}