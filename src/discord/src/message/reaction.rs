use gpui::Rgba;
use serenity::all::ReactionType;
use scope_chat::reaction::{MessageReaction, ReactionEmoji, MessageReactionType};

#[derive(Clone, Debug)]
pub struct DiscordMessageReaction {
    pub count: u32,
    pub count_burst: u32,
    pub self_reaction: Option<MessageReactionType>,
    pub emoji: ReactionEmoji,
    pub burst_colors: Vec<Rgba>,
}

impl DiscordMessageReaction {
    pub fn from_serenity(reaction: &serenity::all::MessageReaction) -> Result<Self, String> {
        let emoji = match &reaction.reaction_type {
            ReactionType::Custom { animated, id, name } => {
                ReactionEmoji::Custom {
                    url: format!("https://cdn.discordapp.com/emojis/{}.png", id),
                    animated: *animated,
                    name: name.clone(),
                }
            }
            ReactionType::Unicode(character) => {
                ReactionEmoji::Simple(character.clone())
            }
            ty => {
                return Err(format!("Unsupported reaction type: {:?}", ty));
            }
        };

        let count = reaction.count_details.normal as u32;
        let count_burst = reaction.count_details.burst as u32;
        let burst_colors = reaction.burst_colours.iter().map(|color| {
            Rgba {
                r: color.r() as f32,
                g: color.g() as f32,
                b: color.b() as f32,
                a: 1f32,
            }
        }).collect();

        let self_reaction = if reaction.me {
            Some(MessageReactionType::Normal)
        } else if reaction.me_burst {
            Some(MessageReactionType::Burst)
        } else {
            None
        };

        Ok(DiscordMessageReaction {
            count,
            count_burst,
            self_reaction,
            emoji,
            burst_colors,
        })
    }
}

impl MessageReaction for DiscordMessageReaction {
    fn get_count(&self, kind: Option<MessageReactionType>) -> u32 {
        match kind {
            Some(MessageReactionType::Burst) => self.count_burst,
            Some(MessageReactionType::Normal) => self.count,
            None => self.count + self.count_burst,
        }
    }

    fn get_self_reaction(&self) -> Option<MessageReactionType> {
        self.self_reaction
    }

    fn get_emoji(&self) -> ReactionEmoji {
        self.emoji.clone()
    }

    fn get_burst_colors(&self) -> Vec<Rgba> {
        self.burst_colors.clone()
    }
}
