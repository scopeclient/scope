//! Events a backend emits so the UI can refresh navigation state.

use crate::nav::Id;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientEvent {
  /// The backend finished connecting; everything should be (re)loaded.
  Ready,
  /// The set of guilds (or their metadata) changed.
  GuildsUpdated,
  /// Channels of a guild changed (new channel, unread counts, …).
  ChannelsUpdated(Id),
  /// Members of a guild changed.
  MembersUpdated(Id),
  /// Someone's presence in a guild changed.
  PresenceUpdated(Id),
  /// Someone started typing in a channel.
  Typing { channel: Id, user: String },
}
