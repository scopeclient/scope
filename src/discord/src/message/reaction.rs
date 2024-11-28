use components::theme::ActiveTheme;
use gpui::{div, img, px, AnyElement, IntoElement, ParentElement, RenderOnce, Rgba, Styled, WindowContext};
use gpui::prelude::FluentBuilder;
use scope_chat::reaction::MessageReactionType::Normal;
use scope_chat::reaction::{MessageReaction, MessageReactionType, ReactionEmoji};
use serenity::all::ReactionType;
use MessageReactionType::Burst;

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

#[derive(Clone, Debug, IntoElement)]
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
      ReactionData::Local { me, .. } => *me,
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

impl DiscordMessageReaction {
  fn render_emoji(emoji: &ReactionEmoji) -> AnyElement {
    match emoji {
      ReactionEmoji::Simple(character) => div().text_size(px(12f32)).child(character.clone()).into_any_element(),
      ReactionEmoji::Custom { url, .. } => img(url.clone()).w(px(16f32)).h(px(16f32)).into_any_element(),
    }
  }
}

impl RenderOnce for DiscordMessageReaction {
  fn render(self, cx: &mut WindowContext) -> impl IntoElement {
    let emoji = self.get_emoji();
    let theme = cx.theme();
    div()
        .p_1()
        .border_1()
        .border_color(theme.border)
        .when(self.get_self_reaction().is_some(), |s| s.border_color(theme.accent))
        .bg(theme.panel)
        .rounded_md()
        .child(Self::render_emoji(&emoji))
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
