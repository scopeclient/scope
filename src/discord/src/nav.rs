//! Navigation data (user, guilds, channels, members) read from the serenity cache.

use std::cmp::Reverse;

use scope_chat::nav::{ChannelInfo, ChannelKind, GuildInfo, Id, MemberInfo, Presence, UserInfo};
use serenity::all::{Activity, ActivityType, ChannelType, GuildChannel, OnlineStatus, Role, User};

use crate::{client::DiscordClient, dm::DmChannel, snowflake::Snowflake};

/// Convert any serenity id into a backend-agnostic [`Id`].
pub(crate) fn id(value: impl Into<Snowflake>) -> Id {
  value.into().into()
}

pub use scope_chat::event::ClientEvent;

pub(crate) fn presence_from(status: OnlineStatus) -> Presence {
  match status {
    OnlineStatus::Online => Presence::Online,
    OnlineStatus::Idle => Presence::Idle,
    OnlineStatus::DoNotDisturb => Presence::DoNotDisturb,
    OnlineStatus::Offline | OnlineStatus::Invisible => Presence::Offline,
    _ => Presence::Offline,
  }
}

pub(crate) fn custom_status(activities: &[Activity]) -> Option<String> {
  activities.iter().find(|a| a.kind == ActivityType::Custom).and_then(|a| a.state.clone()).filter(|s| !s.is_empty())
}

pub(crate) fn channel_kind(kind: ChannelType) -> ChannelKind {
  match kind {
    ChannelType::Text => ChannelKind::Text,
    ChannelType::News => ChannelKind::Announcement,
    ChannelType::Voice => ChannelKind::Voice,
    ChannelType::Stage => ChannelKind::Stage,
    ChannelType::Forum => ChannelKind::Forum,
    ChannelType::NewsThread | ChannelType::PublicThread | ChannelType::PrivateThread => ChannelKind::Thread,
    ChannelType::Category => ChannelKind::Category,
    ChannelType::Private => ChannelKind::DirectMessage,
    ChannelType::GroupDm => ChannelKind::GroupDm,
    _ => ChannelKind::Other,
  }
}

fn user_info(user: &User, presence: Presence, status_text: Option<String>) -> UserInfo {
  UserInfo {
    id: id(user.id),
    username: user.name.clone(),
    display_name: user.display_name().to_owned(),
    tag: user.tag(),
    avatar_url: Some(user.face()),
    presence,
    status_text,
  }
}

fn guild_channel_info(channel: &GuildChannel, unread: u32) -> ChannelInfo {
  ChannelInfo {
    id: id(channel.id),
    guild_id: Some(id(channel.guild_id)),
    name: channel.name.clone(),
    kind: channel_kind(channel.kind),
    parent_id: channel.parent_id.map(id),
    position: channel.position as i64,
    unread,
    muted: false,
    icon_url: None,
  }
}

fn dm_channel_info(dm: &DmChannel, position: i64, unread: u32) -> ChannelInfo {
  ChannelInfo {
    id: id(dm.id),
    guild_id: None,
    name: dm.display_name(),
    kind: channel_kind(dm.kind),
    parent_id: None,
    position,
    unread,
    muted: false,
    icon_url: dm.icon_url(),
  }
}

impl DiscordClient {
  /// The signed-in user.
  ///
  /// Presence comes from the account's own sessions (`SESSIONS_REPLACE`) when Discord has
  /// sent them, else from whichever guild presence the cache holds for the user.
  pub fn current_user(&self) -> UserInfo {
    let user = self.own_user();

    if let Some(own) = self.own_presence() {
      return user_info(&user, presence_from(own.status), own.status_text);
    }

    let cache = &self.discord().cache;
    let presence = cache.guilds().into_iter().find_map(|gid| cache.guild(gid).and_then(|g| g.presences.get(&user.id).cloned()));

    user_info(
      &user,
      presence.as_ref().map_or(Presence::Online, |p| presence_from(p.status)),
      presence.as_ref().and_then(|p| custom_status(&p.activities)),
    )
  }

  /// All guilds the user is in, sorted by name.
  pub fn guilds(&self) -> Vec<GuildInfo> {
    let cache = &self.discord().cache;

    let mut guilds: Vec<GuildInfo> = cache
      .guilds()
      .into_iter()
      .filter_map(|gid| {
        let guild = cache.guild(gid)?;
        Some(GuildInfo {
          id: id(guild.id),
          name: guild.name.clone(),
          icon_url: guild.icon_url(),
          banner_url: guild.banner_url(),
          member_count: Some(guild.member_count),
          online_count: Some(guild.presences.values().filter(|p| p.status != OnlineStatus::Offline).count() as u64),
          unread: self.unread.guild_mentions(guild.id),
        })
      })
      .collect();

    guilds.sort_by_key(|guild| guild.name.to_lowercase());
    guilds
  }

  /// Categories and channels of a guild, sorted by (category position, channel position).
  pub fn guild_channels(&self, guild: Id) -> Vec<ChannelInfo> {
    let cache = &self.discord().cache;
    let Some(guild) = cache.guild(guild.0) else { return Vec::new() };

    let mut channels: Vec<ChannelInfo> = guild.channels.values().map(|channel| guild_channel_info(channel, self.unread.badge(channel.id))).collect();
    channels.sort_by_key(|c| (c.kind != ChannelKind::Category, c.position, c.id));
    channels
  }

  /// Members of a guild the cache knows about, with presence.
  ///
  /// Grouped under their highest hoisted role (highest role position first, members
  /// without one last), then online before offline, then by name.
  pub fn guild_members(&self, guild: Id) -> Vec<MemberInfo> {
    let cache = &self.discord().cache;
    let Some(guild) = cache.guild(guild.0) else { return Vec::new() };

    let mut hoisted: Vec<&Role> = guild.roles.values().filter(|role| role.hoist).collect();
    hoisted.sort_by_key(|role| Reverse((role.position, role.id)));

    let mut members: Vec<(usize, MemberInfo)> = guild
      .members
      .values()
      .map(|member| {
        let presence = guild.presences.get(&member.user.id);
        let rank = hoisted.iter().position(|role| member.roles.contains(&role.id)).unwrap_or(hoisted.len());

        let info = MemberInfo {
          id: id(member.user.id),
          display_name: member.display_name().to_owned(),
          avatar_url: Some(member.face()),
          presence: presence.map(|p| presence_from(p.status)).unwrap_or_default(),
          status_text: presence.and_then(|p| custom_status(&p.activities)),
          role_group: hoisted.get(rank).map(|role| role.name.clone()),
        };

        (rank, info)
      })
      .collect();

    members.sort_by(|(rank_a, a), (rank_b, b)| {
      rank_a
        .cmp(rank_b)
        .then_with(|| (a.presence == Presence::Offline).cmp(&(b.presence == Presence::Offline)))
        .then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
    });

    members.into_iter().map(|(_, member)| member).collect()
  }

  /// Direct and group messages, most recently active first.
  pub fn dm_channels(&self) -> Vec<ChannelInfo> {
    let mut dms: Vec<DmChannel> = self.dms.iter().map(|dm| dm.clone()).collect();
    dms.sort_by_key(|dm| Reverse(dm.activity()));

    dms.iter().enumerate().map(|(position, dm)| dm_channel_info(dm, position as i64, self.unread.badge(dm.id))).collect()
  }
}
