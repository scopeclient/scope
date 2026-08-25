use std::{
  collections::HashMap,
  sync::{Arc, RwLock, Weak},
  time::Duration,
};

use chrono::{Duration as ChronoDuration, Utc};
use scope_chat::{
  async_list::{AsyncList, AsyncListIndex},
  event::ClientEvent,
  nav::{ChannelInfo, ChannelKind, GuildInfo, Id, MemberInfo, Presence, UserInfo},
};
use scope_rich::{MessageKind, ReplyRef, RichMessage};
use tokio::sync::broadcast;

use crate::{channel::DemoChannel, data, message::DemoMessage};

pub struct DemoClient {
  user: UserInfo,
  guilds: RwLock<Vec<GuildInfo>>,
  channels: RwLock<HashMap<Id, Vec<ChannelInfo>>>,
  members: RwLock<HashMap<Id, Vec<MemberInfo>>>,
  dms: RwLock<Vec<ChannelInfo>>,
  open: RwLock<HashMap<Id, Arc<DemoChannel>>>,
  events: broadcast::Sender<ClientEvent>,
}

impl DemoClient {
  /// Build the sample world and start the background "life" task.
  pub fn new() -> Arc<Self> {
    let guilds = data::guilds();
    let channels = guilds.iter().map(|g| (g.id, data::channels(g.id))).collect();
    let members = guilds.iter().map(|g| (g.id, data::members(g.id))).collect();

    let client = Arc::new(DemoClient {
      user: data::current_user(),
      guilds: RwLock::new(guilds),
      channels: RwLock::new(channels),
      members: RwLock::new(members),
      dms: RwLock::new(data::dms()),
      open: RwLock::new(HashMap::new()),
      events: broadcast::channel(256).0,
    });

    tokio::spawn(Self::live(Arc::downgrade(&client)));

    client
  }

  pub fn events(&self) -> broadcast::Receiver<ClientEvent> {
    self.events.subscribe()
  }

  fn emit(&self, event: ClientEvent) {
    let _ = self.events.send(event);
  }

  pub fn current_user(&self) -> UserInfo {
    self.user.clone()
  }

  pub fn guilds(&self) -> Vec<GuildInfo> {
    self.guilds.read().unwrap().clone()
  }

  pub fn guild_channels(&self, guild: Id) -> Vec<ChannelInfo> {
    self.channels.read().unwrap().get(&guild).cloned().unwrap_or_default()
  }

  pub fn guild_members(&self, guild: Id) -> Vec<MemberInfo> {
    self.members.read().unwrap().get(&guild).cloned().unwrap_or_default()
  }

  pub fn dm_channels(&self) -> Vec<ChannelInfo> {
    self.dms.read().unwrap().clone()
  }

  fn channel_info(&self, id: Id) -> Option<ChannelInfo> {
    self.channels.read().unwrap().values().flatten().chain(self.dms.read().unwrap().iter()).find(|c| c.id == id).cloned()
  }

  /// Open (or reuse) a channel. Opening clears its unread badge.
  pub async fn channel(self: Arc<Self>, id: Id) -> Arc<DemoChannel> {
    if let Some(existing) = self.open.read().unwrap().get(&id) {
      return existing.clone();
    }

    let info = self.channel_info(id);
    let name = info.as_ref().map(|c| c.name.clone()).unwrap_or_else(|| format!("channel-{}", id.0));
    let guild = info.as_ref().and_then(|c| c.guild_id);

    let channel = Arc::new(DemoChannel::new(id, guild, name, self.seed_history(id, info.as_ref())));
    self.open.write().unwrap().insert(id, channel.clone());

    self.set_unread(id, 0);
    channel
  }

  fn seed_history(&self, id: Id, info: Option<&ChannelInfo>) -> Vec<DemoMessage> {
    let now = Utc::now();

    if id == Id(102) {
      return data::announcements_history()
        .into_iter()
        .enumerate()
        .map(|(i, (author, text, minutes_ago))| DemoMessage::new(Id(1_000_000 + i as u64), author, text, now - ChronoDuration::minutes(minutes_ago)))
        .collect();
    }

    // #dev-announcements exercises every kind of rich content.
    if id == Id(103) {
      return data::dev_announcements_history(now);
    }

    let is_dm = info.is_some_and(|c| matches!(c.kind, ChannelKind::DirectMessage | ChannelKind::GroupDm));
    let partner = if is_dm { Id(id.0 - 900) } else { Id(2) };
    let count = 6 + (id.0 % 7) as usize;

    (0..count)
      .map(|i| {
        let author = if i % 3 == 2 {
          data::SELF_ID
        } else if is_dm {
          partner
        } else {
          data::PEOPLE[(i + id.0 as usize) % data::PEOPLE.len()].id
        };
        let text = data::CHATTER[(i * 7 + id.0 as usize) % data::CHATTER.len()];
        DemoMessage::new(
          Id(id.0 * 10_000 + i as u64),
          author,
          text,
          now - ChronoDuration::minutes((count - i) as i64 * 9),
        )
      })
      .collect()
  }

  fn set_unread(&self, channel: Id, unread: u32) {
    let mut guild_changed = None;

    for (guild, channels) in self.channels.write().unwrap().iter_mut() {
      if let Some(c) = channels.iter_mut().find(|c| c.id == channel) {
        c.unread = unread;
        guild_changed = Some(*guild);
      }
    }

    if let Some(c) = self.dms.write().unwrap().iter_mut().find(|c| c.id == channel) {
      c.unread = unread;
    }

    if let Some(guild) = guild_changed {
      let total: u32 = self.channels.read().unwrap().get(&guild).map(|cs| cs.iter().map(|c| c.unread).sum()).unwrap_or(0);
      if let Some(g) = self.guilds.write().unwrap().iter_mut().find(|g| g.id == guild) {
        g.unread = total;
      }
      self.emit(ClientEvent::ChannelsUpdated(guild));
      self.emit(ClientEvent::GuildsUpdated);
    } else {
      self.emit(ClientEvent::GuildsUpdated);
    }
  }

  /// Background activity: random messages, unread bumps, presence changes.
  async fn live(weak: Weak<DemoClient>) {
    // Let the UI settle before the first event.
    tokio::time::sleep(Duration::from_secs(3)).await;

    loop {
      let wait = rand::random_range(2500..7000u64);
      tokio::time::sleep(Duration::from_millis(wait)).await;

      let Some(client) = weak.upgrade() else { break };
      let roll: u8 = rand::random_range(0..100);

      if roll < 55 {
        client.post_random_message();
      } else if roll < 80 {
        client.bump_random_unread();
      } else if roll < 95 {
        client.flip_random_presence();
      } else {
        client.change_random_status();
      }
    }
  }

  fn post_random_message(&self) {
    let open: Vec<Arc<DemoChannel>> = self.open.read().unwrap().values().cloned().collect();
    let Some(channel) = open.get(rand::random_range(0..open.len().max(1))).cloned() else {
      self.bump_random_unread();
      return;
    };

    let people: Vec<Id> = data::PEOPLE.iter().filter(|p| p.presence != Presence::Offline).map(|p| p.id).collect();
    let same_author = rand::random_range(0..100) < 45;
    let author = match channel.last_author().filter(|_| same_author).filter(|id| *id != data::SELF_ID) {
      Some(id) => id,
      None => people[rand::random_range(0..people.len())],
    };

    let author = if channel.guild.is_none() { Id(channel.id.0 - 900) } else { author };
    let name = data::person(author).map(|p| p.name.to_string()).unwrap_or_default();

    self.emit(ClientEvent::Typing {
      channel: channel.id,
      user: name,
    });

    let flavour = LiveFlavour::pick();
    let channel = channel.clone();
    tokio::spawn(async move {
      tokio::time::sleep(Duration::from_millis(rand::random_range(900..2200u64))).await;

      // Replies quote whatever is at the bottom of the channel once the "typing" finishes.
      let previous = match flavour {
        LiveFlavour::Reply => channel.get(AsyncListIndex::RelativeToBottom(0)).await.map(|r| r.content),
        _ => None,
      };

      channel.post(Self::live_message(flavour, author, previous.as_ref()));
    });
  }

  fn live_message(flavour: LiveFlavour, author: Id, previous: Option<&DemoMessage>) -> DemoMessage {
    let pick = |list: &[&'static str]| list[rand::random_range(0..list.len())];
    let id = Id(rand::random());
    let now = Utc::now();

    let rich = match flavour {
      LiveFlavour::Plain => return DemoMessage::new(id, author, pick(data::CHATTER), now),
      LiveFlavour::Reactions => RichMessage {
        reactions: data::hot_reactions(rand::random_range(2..15), rand::random_range(0..2) == 0),
        ..data::rich_text(pick(data::CHATTER))
      },
      LiveFlavour::Image => RichMessage {
        attachments: vec![data::image_attachment(rand::random(), "square.png", 400, 400, 19_760)],
        ..data::rich_text(pick(&["look at this", "first pass at the icon", ""]))
      },
      LiveFlavour::Markdown => data::rich_text(pick(data::RICH_CHATTER)),
      LiveFlavour::Reply => match previous {
        Some(previous) => RichMessage {
          kind: MessageKind::Reply,
          reply: Some(Self::reply_to(previous)),
          ..data::rich_text(pick(&["^ this", "agreed", "wait really?", "lol", "based", "can you elaborate"]))
        },
        None => data::rich_text(pick(data::CHATTER)),
      },
    };

    let content = rich.source.clone();
    DemoMessage::new(id, author, content, now).with_rich(rich)
  }

  fn reply_to(previous: &DemoMessage) -> ReplyRef {
    let snippet = previous.content.lines().find(|l| !l.trim().is_empty()).map(str::to_owned).unwrap_or_else(|| "Click to see attachment".into());
    ReplyRef {
      message_id: Some(previous.id.0),
      author_name: previous.author.name.to_string(),
      author_avatar: previous.author.avatar.as_ref().map(ToString::to_string),
      snippet,
      deleted: false,
    }
  }

  fn bump_random_unread(&self) {
    let open: Vec<Id> = self.open.read().unwrap().keys().copied().collect();
    let candidates: Vec<Id> = self
      .channels
      .read()
      .unwrap()
      .values()
      .flatten()
      .filter(|c| c.kind.is_messageable() && !c.muted && !open.contains(&c.id))
      .map(|c| c.id)
      .collect();

    if candidates.is_empty() {
      return;
    }

    let target = candidates[rand::random_range(0..candidates.len())];
    let current = self.channel_info(target).map(|c| c.unread).unwrap_or(0);
    self.set_unread(target, current + 1);
  }

  fn flip_random_presence(&self) {
    let mut members = self.members.write().unwrap();
    let guilds: Vec<Id> = members.keys().copied().collect();
    let Some(guild) = guilds.get(rand::random_range(0..guilds.len().max(1))).copied() else {
      return;
    };
    let Some(list) = members.get_mut(&guild) else { return };
    if list.is_empty() {
      return;
    }

    let index = rand::random_range(0..list.len());
    let member = &mut list[index];
    member.presence = match member.presence {
      Presence::Online => Presence::Idle,
      Presence::Idle => Presence::DoNotDisturb,
      Presence::DoNotDisturb => Presence::Offline,
      Presence::Offline => Presence::Online,
    };

    drop(members);
    self.emit(ClientEvent::PresenceUpdated(guild));
  }

  fn change_random_status(&self) {
    const STATUSES: &[&str] = &[
      "Constructing Chatrooms",
      "shipping scope",
      "in a meeting",
      "listening to Jon Hopkins",
      "touching grass",
      "debugging gpui",
    ];

    let mut members = self.members.write().unwrap();
    let guilds: Vec<Id> = members.keys().copied().collect();
    let Some(guild) = guilds.get(rand::random_range(0..guilds.len().max(1))).copied() else {
      return;
    };
    let Some(list) = members.get_mut(&guild) else { return };
    if list.is_empty() {
      return;
    }

    let index = rand::random_range(0..list.len());
    let member = &mut list[index];
    member.status_text = Some(STATUSES[rand::random_range(0..STATUSES.len())].into());

    drop(members);
    self.emit(ClientEvent::MembersUpdated(guild));
  }
}

/// Shape of a live-feed message; roughly one in five is rich.
#[derive(Clone, Copy, Debug)]
enum LiveFlavour {
  Plain,
  Reactions,
  Image,
  Markdown,
  Reply,
}

impl LiveFlavour {
  fn pick() -> Self {
    if rand::random_range(0..5) != 0 {
      return Self::Plain;
    }
    match rand::random_range(0..4) {
      0 => Self::Reactions,
      1 => Self::Image,
      2 => Self::Markdown,
      _ => Self::Reply,
    }
  }
}
