//! Backend-agnostic view models for navigation chrome: the signed-in user,
//! servers ("guilds"), their channels, and members.
//!
//! Identifiers are plain `u64` snowflakes so every backend (Discord, Spacebar,
//! …) can map onto them without dragging generic parameters through the UI.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(pub u64);

impl fmt::Display for Id {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Presence {
  Online,
  Idle,
  DoNotDisturb,
  #[default]
  Offline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserInfo {
  pub id: Id,
  /// Unique handle, e.g. `user` (or `user#0001` for legacy accounts — see `tag`).
  pub username: String,
  /// Friendly name to show in large type.
  pub display_name: String,
  /// Full tag as the backend prints it, e.g. `user#0001` or `@user`.
  pub tag: String,
  pub avatar_url: Option<String>,
  pub presence: Presence,
  /// Custom status text ("building chatrooms").
  pub status_text: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuildInfo {
  pub id: Id,
  pub name: String,
  pub icon_url: Option<String>,
  pub banner_url: Option<String>,
  pub member_count: Option<u64>,
  pub online_count: Option<u64>,
  /// Mentions / unread count to show as a badge. `0` hides the badge.
  pub unread: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChannelKind {
  Text,
  Announcement,
  Voice,
  Stage,
  Forum,
  Thread,
  Category,
  DirectMessage,
  GroupDm,
  Other,
}

impl ChannelKind {
  pub fn is_messageable(self) -> bool {
    matches!(self, Self::Text | Self::Announcement | Self::Thread | Self::DirectMessage | Self::GroupDm)
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelInfo {
  pub id: Id,
  pub guild_id: Option<Id>,
  pub name: String,
  pub kind: ChannelKind,
  /// Category (or parent channel for threads) this channel sits under.
  pub parent_id: Option<Id>,
  pub position: i64,
  /// Unread / mention count. `0` hides the badge.
  pub unread: u32,
  pub muted: bool,
  /// For DMs: the other party's avatar.
  pub icon_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberInfo {
  pub id: Id,
  pub display_name: String,
  pub avatar_url: Option<String>,
  pub presence: Presence,
  /// Custom status / activity line shown under the name.
  pub status_text: Option<String>,
  /// Hoisted role name used to group the member list ("MEMBERS", "ADMINS", …).
  pub role_group: Option<String>,
}
