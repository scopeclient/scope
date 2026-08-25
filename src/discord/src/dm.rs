//! Private channels (DMs and group DMs) of the signed-in user.
//!
//! The serenity fork marks `Ready::private_channels` as `#[serde(skip)]` because group
//! DMs do not fit its single-recipient [`PrivateChannel`] model, and its cache never
//! stores private channels. The list is therefore fetched from
//! `GET /users/@me/channels` and parsed loosely here so both kinds survive.

use serenity::{
  all::{Channel, ChannelId, ChannelType, Http, MessageId, PrivateChannel, User},
  http::{LightMethod, Request, Route},
  json::{Value, from_value},
};

#[derive(Clone, Debug)]
pub(crate) struct DmChannel {
  pub id: ChannelId,
  /// Either [`ChannelType::Private`] or [`ChannelType::GroupDm`].
  pub kind: ChannelType,
  /// Explicit group name, when the group has one.
  pub name: Option<String>,
  /// Group icon hash.
  pub icon: Option<String>,
  pub last_message_id: Option<MessageId>,
  /// Everyone in the channel except the signed-in user.
  pub recipients: Vec<User>,
}

impl DmChannel {
  fn parse(value: &Value) -> Option<Self> {
    let id: ChannelId = from_value(value.get("id")?.clone()).ok()?;
    let kind: ChannelType = from_value(value.get("type")?.clone()).ok()?;

    if !matches!(kind, ChannelType::Private | ChannelType::GroupDm) {
      return None;
    }

    let recipients = value
      .get("recipients")
      .and_then(|recipients| {
        from_value::<Vec<User>>(recipients.clone()).map_err(|why| log::debug!("Discord: unreadable recipients of {id}: {why:?}")).ok()
      })
      .unwrap_or_default();

    Some(Self {
      id,
      kind,
      name: value.get("name").and_then(Value::as_str).filter(|name| !name.is_empty()).map(str::to_owned),
      icon: value.get("icon").and_then(Value::as_str).map(str::to_owned),
      last_message_id: value.get("last_message_id").and_then(|id| from_value::<Option<MessageId>>(id.clone()).ok()).flatten(),
      recipients,
    })
  }

  /// Group name, else the recipients' display names.
  pub fn display_name(&self) -> String {
    if let Some(name) = &self.name {
      return name.clone();
    }

    let names: Vec<&str> = self.recipients.iter().map(User::display_name).collect();

    if names.is_empty() { "Unnamed".to_owned() } else { names.join(", ") }
  }

  /// Group icon, or the other party's avatar for a 1:1 DM.
  pub fn icon_url(&self) -> Option<String> {
    match self.kind {
      ChannelType::GroupDm => self.icon.as_ref().map(|icon| format!("https://cdn.discordapp.com/channel-icons/{}/{icon}.png?size=128", self.id)),
      _ => self.recipients.first().map(User::face),
    }
  }

  /// Recency key: the last message, or the channel's own creation time when it has none.
  pub fn activity(&self) -> u64 {
    self.last_message_id.map_or(self.id.get(), MessageId::get)
  }

  /// The closest serenity [`Channel`] for this DM.
  ///
  /// `PrivateChannel` is the only non-guild variant, so a group DM becomes a private
  /// channel whose `kind` is [`ChannelType::GroupDm`] and whose `recipient` is the first
  /// member; `fallback` stands in when the group has no other members.
  pub fn to_channel(&self, fallback: &User) -> Channel {
    let mut channel = PrivateChannel::default();
    channel.id = self.id;
    channel.kind = self.kind;
    channel.last_message_id = self.last_message_id;
    channel.recipient = self.recipients.first().cloned().unwrap_or_else(|| fallback.clone());

    Channel::Private(channel)
  }
}

/// Fetch every private channel of the signed-in user.
pub(crate) async fn fetch_dm_channels(http: &Http) -> Result<Vec<DmChannel>, Box<serenity::Error>> {
  let raw: Vec<Value> = http.fire(Request::new(Route::UserMeDmChannels, LightMethod::Get)).await?;

  Ok(raw.iter().filter_map(DmChannel::parse).collect())
}

/// Stand-in for a channel that could not be resolved, so message fetching (which only
/// needs the id) keeps working instead of panicking.
pub(crate) fn placeholder_channel(id: ChannelId, own_user: &User) -> Channel {
  let mut channel = PrivateChannel::default();
  channel.id = id;
  channel.kind = ChannelType::Private;
  channel.recipient = own_user.clone();

  Channel::Private(channel)
}
