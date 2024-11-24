use gpui::Rgba;
use serenity::all::ReactionType;
use scope_chat::reaction::{MessageReaction, ReactionEmoji, MessageReactionType};

#[derive(Clone, Debug)]
pub struct DiscordMessageReaction {
    pub data: serenity::all::MessageReaction,
}

impl DiscordMessageReaction {
    pub fn from_serenity(reaction: &serenity::all::MessageReaction) -> Self {
        DiscordMessageReaction {
            data: reaction.clone(),
        }
    }
}

impl MessageReaction for DiscordMessageReaction {
    fn get_count(&self, kind: Option<MessageReactionType>) -> u64 {
        match kind {
            Some(MessageReactionType::Burst) => self.data.count_details.burst,
            Some(MessageReactionType::Normal) => self.data.count_details.normal,
            None => self.data.count,
        }
    }

    fn get_self_reaction(&self) -> Option<MessageReactionType> {
        if self.data.me {
            Some(MessageReactionType::Normal)
        } else if self.data.me_burst {
            Some(MessageReactionType::Burst)
        } else {
            None
        }
    }

    fn get_emoji(&self) -> ReactionEmoji {
        match &self.data.reaction_type {
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
                eprintln!("Unsupported reaction type: {:?}", ty);
                ReactionEmoji::Simple("❓".to_string())
            }
        }
    }

    fn get_burst_colors(&self) -> Vec<Rgba> {
        self.data.burst_colours.iter().map(|color| {
            Rgba {
                r: color.r() as f32,
                g: color.g() as f32,
                b: color.b() as f32,
                a: 1f32,
            }
        }).collect()
    }
}
