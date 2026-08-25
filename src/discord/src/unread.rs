//! Session-local unread bookkeeping.
//!
//! The serenity fork does not expose Discord's `read_state`, so there is no record of
//! what was read before this session started. Instead, messages that arrive in a
//! channel that has not been opened in this app are counted here. A channel is cleared
//! when it is opened, when the signed-in user posts in it, or when another Discord
//! client acknowledges it (`MESSAGE_ACK`).

use dashmap::DashMap;
use serenity::all::{ChannelId, GuildId};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Unread {
  pub guild_id: Option<GuildId>,
  /// Messages since the channel was last read.
  pub messages: u32,
  /// Of those, messages that mention the signed-in user. Every DM message counts.
  pub mentions: u32,
}

impl Unread {
  /// Badge value: the mention count when mentioned, otherwise `1` to mark "has unread"
  /// (the UI hides the badge on `0`).
  pub fn badge(&self) -> u32 {
    if self.mentions > 0 {
      self.mentions
    } else {
      u32::from(self.messages > 0)
    }
  }
}

#[derive(Default)]
pub(crate) struct UnreadTracker(DashMap<ChannelId, Unread>);

impl UnreadTracker {
  /// Count a message in `channel`.
  pub fn record(&self, channel: ChannelId, guild_id: Option<GuildId>, mentioned: bool) {
    let mut entry = self.0.entry(channel).or_insert(Unread {
      guild_id,
      ..Default::default()
    });
    entry.messages += 1;

    if mentioned {
      entry.mentions += 1;
    }
  }

  /// Forget `channel`; returns what was cleared, if anything.
  pub fn mark_read(&self, channel: ChannelId) -> Option<Unread> {
    self.0.remove(&channel).map(|(_, unread)| unread)
  }

  pub fn badge(&self, channel: ChannelId) -> u32 {
    self.0.get(&channel).map_or(0, |unread| unread.badge())
  }

  /// Mentions across all of a guild's channels: the guild badge.
  pub fn guild_mentions(&self, guild: GuildId) -> u32 {
    self.0.iter().filter(|unread| unread.guild_id == Some(guild)).map(|unread| unread.mentions).sum()
  }
}
