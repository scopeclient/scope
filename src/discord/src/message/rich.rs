//! `serenity::Message` → [`RichMessage`].
//!
//! Mentions resolve through the message's own `mentions` / `mention_channels`
//! first (Discord sends those even for users the cache has never seen), then
//! through the serenity cache. Everything else is a straight field mapping.

use chrono::{DateTime, Utc};
use scope_rich::{
  Attachment, ButtonStyle, Component, ComponentRow, Embed, EmbedAuthor, EmbedField, EmbedFooter, EmbedKind, EmbedMedia, EmbedProvider, Emoji,
  MessageKind, Poll, PollAnswer, Reaction, ReplyRef, RichMessage, Sticker, StickerFormat, SystemKind, VoiceClip,
  markdown::{MentionResolver, parse, to_plain_text},
};
use serenity::{
  all::{
    ActionRow, ActionRowComponent, ButtonKind, Cache, Channel, ChannelId, EmojiId, GuildId, Member, Message, MessageFlags, MessageReferenceKind,
    MessageType, PollMediaEmoji, ReactionType, RoleId, StickerFormatType, StickerItem, Timestamp, UserId,
  },
  model::channel::{Embed as SerenityEmbed, MessageReaction},
};

use crate::client::DiscordClient;

/// Longest reply snippet, in characters, before it is cut with an ellipsis.
const SNIPPET_MAX_CHARS: usize = 80;

/// Placeholder name for custom emoji Discord sends without one.
const UNNAMED_EMOJI: &str = "emoji";

pub fn from_serenity(message: &Message, _member: Option<&Member>, channel: &Channel, client: &DiscordClient) -> RichMessage {
  let cache = &client.discord().cache;
  let guild_id = message_guild_id(message, channel);
  let resolver = CacheResolver { message, cache, guild_id };

  let is_voice = message.flags.is_some_and(|f| f.contains(MessageFlags::IS_VOICE_MESSAGE));
  let reply = reply_ref(message, cache, guild_id);

  RichMessage {
    kind: message_kind(message.kind, &message.content, reply.is_some()),
    blocks: parse(&message.content, &resolver),
    attachments: message.attachments.iter().map(|a| attachment(a, is_voice)).collect(),
    embeds: message.embeds.iter().map(|e| embed(e, &resolver)).collect(),
    stickers: message.sticker_items.iter().map(sticker).collect(),
    reactions: message.reactions.iter().map(reaction).collect(),
    poll: message.poll.as_deref().map(|p| poll(p, cache, guild_id)),
    reply,
    components: message.components.iter().map(component_row).collect(),
    edited_at: message.edited_timestamp.and_then(to_chrono),
    pinned: message.pinned,
    pending: false,
    source: message.content.clone(),
  }
}

// ---- mentions --------------------------------------------------------------------

/// Resolves mention ids from the message payload first, then the serenity cache.
struct CacheResolver<'a> {
  message: &'a Message,
  cache: &'a Cache,
  guild_id: Option<GuildId>,
}

impl MentionResolver for CacheResolver<'_> {
  fn user(&self, id: u64) -> Option<String> {
    let id = UserId::new(id);

    if let Some(user) = self.message.mentions.iter().find(|u| u.id == id) {
      return Some(user.display_name().to_owned());
    }

    // A nickname beats the global name inside a guild.
    cached_member_name(self.cache, self.guild_id, id).or_else(|| self.cache.user(id).map(|u| u.display_name().to_owned()))
  }

  fn channel(&self, id: u64) -> Option<String> {
    let id = ChannelId::new(id);

    if let Some(channel) = self.message.mention_channels.iter().find(|c| c.id == id) {
      return Some(channel.name.clone());
    }

    // DMs and group DMs are not in the cache; the parser falls back to the id.
    cached_channel_name(self.cache, self.guild_id, id)
  }

  fn role(&self, id: u64) -> Option<(String, Option<u32>)> {
    let guild = self.cache.guild(self.guild_id?)?;
    let role = guild.roles.get(&RoleId::new(id))?;
    let color = (role.colour.0 != 0).then_some(role.colour.0);
    Some((role.name.clone(), color))
  }
}

/// Display name (nick, global name or username) of a guild member the cache holds.
fn cached_member_name(cache: &Cache, guild_id: Option<GuildId>, user_id: UserId) -> Option<String> {
  let guild = cache.guild(guild_id?)?;
  guild.members.get(&user_id).map(|m| m.display_name().to_owned())
}

/// Name of a cached guild channel or thread: the message's guild first, then every other guild.
fn cached_channel_name(cache: &Cache, guild_id: Option<GuildId>, channel_id: ChannelId) -> Option<String> {
  let name_in = |gid: GuildId| {
    let guild = cache.guild(gid)?;
    guild.channels.get(&channel_id).map(|c| c.name.clone()).or_else(|| guild.threads.iter().find(|t| t.id == channel_id).map(|t| t.name.clone()))
  };

  guild_id.and_then(name_in).or_else(|| cache.guilds().into_iter().filter(|g| Some(*g) != guild_id).find_map(name_in))
}

/// The guild a message was sent in, from the message itself or the channel it arrived on.
pub(crate) fn message_guild_id(message: &Message, channel: &Channel) -> Option<GuildId> {
  message.guild_id.or(match channel {
    Channel::Guild(guild_channel) => Some(guild_channel.guild_id),
    _ => None,
  })
}

// ---- kind / reply ------------------------------------------------------------------

/// Classify a message. `has_reply` is whether a (non-forward) reference could be built.
fn message_kind(kind: MessageType, content: &str, has_reply: bool) -> MessageKind {
  match kind {
    MessageType::InlineReply => MessageKind::Reply,
    MessageType::Regular | MessageType::ChatInputCommand | MessageType::ContextMenuCommand | MessageType::ThreadStarterMessage => {
      if has_reply {
        MessageKind::Reply
      } else {
        MessageKind::Default
      }
    }
    other => MessageKind::System(system_kind(other, content)),
  }
}

/// Map a non-regular [`MessageType`] onto a [`SystemKind`].
fn system_kind(kind: MessageType, content: &str) -> SystemKind {
  match kind {
    MessageType::GroupRecipientAddition => SystemKind::GroupRecipientAdd,
    MessageType::GroupRecipientRemoval => SystemKind::GroupRecipientRemove,
    MessageType::GroupCallCreation => SystemKind::Call,
    MessageType::GroupNameUpdate => SystemKind::GroupNameUpdate,
    MessageType::GroupIconUpdate => SystemKind::GroupIconUpdate,
    MessageType::PinsAdd => SystemKind::PinsAdd,
    MessageType::MemberJoin => SystemKind::MemberJoin,
    MessageType::NitroBoost => SystemKind::Boost { tier: None },
    MessageType::NitroTier1 => SystemKind::Boost { tier: Some(1) },
    MessageType::NitroTier2 => SystemKind::Boost { tier: Some(2) },
    MessageType::NitroTier3 => SystemKind::Boost { tier: Some(3) },
    MessageType::ChannelFollowAdd => SystemKind::ChannelFollowAdd,
    // Discord puts the new thread's name in the content.
    MessageType::ThreadCreated => SystemKind::ThreadCreated { name: content.to_owned() },
    other => SystemKind::Other(format!("{other:?}")),
  }
}

/// The message this one replies to, when it has a reply-style reference.
///
/// Forwards also carry a `message_reference`, but the fork has no `message_snapshots`,
/// so they are left without a reply rather than shown as a deleted one.
fn reply_ref(message: &Message, cache: &Cache, guild_id: Option<GuildId>) -> Option<ReplyRef> {
  let reference = message.message_reference.as_ref()?;

  if reference.kind == MessageReferenceKind::Forward {
    log::debug!(
      "Discord: message {} forwards {:?}; the fork has no message_snapshots",
      message.id,
      reference.message_id
    );
    return None;
  }

  let Some(referenced) = message.referenced_message.as_deref() else {
    return Some(ReplyRef {
      message_id: reference.message_id.map(|id| id.get()),
      author_name: "Unknown".to_owned(),
      author_avatar: None,
      snippet: String::new(),
      deleted: true,
    });
  };

  Some(reply_ref_to(referenced, cache, guild_id))
}

/// The reply header for a message replying to `referenced`.
pub(crate) fn reply_ref_to(referenced: &Message, cache: &Cache, guild_id: Option<GuildId>) -> ReplyRef {
  let resolver = CacheResolver {
    message: referenced,
    cache,
    guild_id,
  };
  let text = to_plain_text(&parse(&referenced.content, &resolver));
  let snippet = snippet(&text).unwrap_or_else(|| reply_placeholder(referenced).to_owned());

  ReplyRef {
    message_id: Some(referenced.id.get()),
    author_name: reply_author_name(referenced, cache, guild_id),
    author_avatar: Some(referenced.author.face()),
    snippet,
    deleted: false,
  }
}

/// A reply header for a message we have not loaded: shows the reply is there until
/// Discord echoes the sent message back with the full reference.
pub(crate) fn unresolved_reply_ref(message_id: u64) -> ReplyRef {
  ReplyRef {
    message_id: Some(message_id),
    author_name: "…".to_owned(),
    author_avatar: None,
    snippet: String::new(),
    deleted: false,
  }
}

/// Nickname from the cached member or the message's partial member, else the global display name.
fn reply_author_name(referenced: &Message, cache: &Cache, guild_id: Option<GuildId>) -> String {
  if let Some(name) = cached_member_name(cache, guild_id, referenced.author.id) {
    return name;
  }

  if let Some(nick) = referenced.member.as_deref().and_then(|m| m.nick.clone()) {
    return nick;
  }

  referenced.author.display_name().to_owned()
}

/// What to show for a referenced message that has no text.
fn reply_placeholder(referenced: &Message) -> &'static str {
  if !referenced.attachments.is_empty() {
    "Click to see attachment"
  } else if !referenced.sticker_items.is_empty() {
    "[sticker]"
  } else if !referenced.embeds.is_empty() {
    "[embed]"
  } else if referenced.poll.is_some() {
    "[poll]"
  } else {
    ""
  }
}

/// First non-blank line of `text`, cut to [`SNIPPET_MAX_CHARS`]; `None` when there is no text.
fn snippet(text: &str) -> Option<String> {
  let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
  let mut chars = line.chars();
  let head: String = chars.by_ref().take(SNIPPET_MAX_CHARS).collect();

  Some(if chars.next().is_some() { format!("{head}…") } else { head })
}

// ---- attachments -------------------------------------------------------------------

fn attachment(a: &serenity::model::channel::Attachment, is_voice: bool) -> Attachment {
  Attachment {
    id: a.id.get(),
    filename: a.filename.clone(),
    url: a.url.clone(),
    proxy_url: Some(a.proxy_url.clone()),
    content_type: a.content_type.clone(),
    size_bytes: a.size as u64,
    width: a.width,
    height: a.height,
    description: a.description.clone(),
    spoiler: a.filename.starts_with("SPOILER_"),
    voice: if is_voice {
      a.duration_secs.map(|d| VoiceClip {
        duration_secs: d as f32,
        waveform: a.waveform.clone().unwrap_or_default(),
      })
    } else {
      None
    },
  }
}

// ---- embeds -------------------------------------------------------------------------

fn embed(e: &SerenityEmbed, resolver: &dyn MentionResolver) -> Embed {
  Embed {
    kind: e.kind.as_deref().map(embed_kind).unwrap_or_default(),
    title: e.title.clone(),
    url: e.url.clone(),
    description: e.description.as_deref().map(|d| parse(d, resolver)),
    color: e.colour.map(|c| c.0),
    author: e.author.as_ref().map(|a| EmbedAuthor {
      name: a.name.clone(),
      url: a.url.clone(),
      icon_url: a.icon_url.clone(),
    }),
    provider: e.provider.as_ref().map(|p| EmbedProvider {
      name: p.name.clone(),
      url: p.url.clone(),
    }),
    thumbnail: e.thumbnail.as_ref().map(|t| EmbedMedia {
      url: t.url.clone(),
      proxy_url: t.proxy_url.clone(),
      width: t.width,
      height: t.height,
    }),
    image: e.image.as_ref().map(|i| EmbedMedia {
      url: i.url.clone(),
      proxy_url: i.proxy_url.clone(),
      width: i.width,
      height: i.height,
    }),
    video: e.video.as_ref().map(|v| EmbedMedia {
      url: v.url.clone(),
      proxy_url: v.proxy_url.clone(),
      width: v.width,
      height: v.height,
    }),
    fields: e
      .fields
      .iter()
      .map(|f| EmbedField {
        name: f.name.clone(),
        value: parse(&f.value, resolver),
        inline: f.inline,
      })
      .collect(),
    footer: e.footer.as_ref().map(|f| EmbedFooter {
      text: f.text.clone(),
      icon_url: f.icon_url.clone(),
    }),
    timestamp: e.timestamp.and_then(to_chrono),
  }
}

/// Discord's embed `type` string; anything unknown renders as a rich embed.
fn embed_kind(kind: &str) -> EmbedKind {
  match kind {
    "image" => EmbedKind::Image,
    "video" => EmbedKind::Video,
    "gifv" => EmbedKind::Gifv,
    "article" => EmbedKind::Article,
    "link" => EmbedKind::Link,
    _ => EmbedKind::Rich,
  }
}

// ---- stickers / reactions / polls / components ----------------------------------------

fn sticker(item: &StickerItem) -> Sticker {
  Sticker {
    id: item.id.get(),
    name: item.name.clone(),
    format: match item.format_type {
      StickerFormatType::Apng => StickerFormat::Apng,
      StickerFormatType::Lottie => StickerFormat::Lottie,
      StickerFormatType::Gif => StickerFormat::Gif,
      _ => StickerFormat::Png,
    },
    url: item.image_url(),
  }
}

fn reaction(r: &MessageReaction) -> Reaction {
  Reaction {
    emoji: emoji(&r.reaction_type),
    count: r.count,
    me: r.me,
    burst: r.me_burst,
  }
}

fn emoji(reaction: &ReactionType) -> Emoji {
  match reaction {
    ReactionType::Unicode(s) => Emoji::Unicode(s.clone()),
    ReactionType::Custom { animated, id, name } => Emoji::Custom {
      id: id.get(),
      name: name.clone().unwrap_or_else(|| UNNAMED_EMOJI.to_owned()),
      animated: *animated,
      url: None,
    },
    _ => Emoji::Unicode(reaction.to_string()),
  }
}

/// The [`ReactionType`] to send when reacting with `emoji`; the inverse of [`emoji`].
pub(crate) fn reaction_type(emoji: &Emoji) -> ReactionType {
  match emoji {
    Emoji::Unicode(s) => ReactionType::Unicode(s.clone()),
    Emoji::Custom { id, name, animated, .. } => ReactionType::Custom {
      animated: *animated,
      id: EmojiId::new(*id),
      name: Some(name.clone()),
    },
  }
}

fn poll(p: &serenity::all::Poll, cache: &Cache, guild_id: Option<GuildId>) -> Poll {
  let results = p.results.as_ref();
  let count_for = |id: u64| results.and_then(|r| r.answer_counts.iter().find(|c| c.id.get() == id));

  let answers: Vec<PollAnswer> = p
    .answers
    .iter()
    .map(|a| {
      let id = a.answer_id.get();
      let count = count_for(id);

      PollAnswer {
        id,
        text: a.poll_media.text.clone().unwrap_or_default(),
        emoji: a.poll_media.emoji.as_ref().map(|e| poll_emoji(e, cache, guild_id)),
        votes: count.map_or(0, |c| c.count),
        me_voted: count.is_some_and(|c| c.me_voted),
      }
    })
    .collect();

  Poll {
    question: p.question.text.clone().unwrap_or_default(),
    total_votes: answers.iter().map(|a| a.votes).sum(),
    answers,
    expires_at: p.expiry.and_then(to_chrono),
    allow_multiselect: p.allow_multiselect,
    finalized: results.is_some_and(|r| r.is_finalized),
  }
}

/// Poll emoji arrive as either a unicode name or a bare custom emoji id; the guild cache fills in the rest.
fn poll_emoji(e: &PollMediaEmoji, cache: &Cache, guild_id: Option<GuildId>) -> Emoji {
  match e {
    PollMediaEmoji::Name(name) => Emoji::Unicode(name.clone()),
    PollMediaEmoji::Id(id) => custom_emoji(*id, cache, guild_id),
  }
}

fn custom_emoji(id: EmojiId, cache: &Cache, guild_id: Option<GuildId>) -> Emoji {
  let cached = guild_id.and_then(|g| cache.guild(g)).and_then(|guild| guild.emojis.get(&id).map(|e| (e.name.clone(), e.animated)));
  let (name, animated) = cached.unwrap_or_else(|| (UNNAMED_EMOJI.to_owned(), false));

  Emoji::Custom {
    id: id.get(),
    name,
    animated,
    url: None,
  }
}

fn component_row(row: &ActionRow) -> ComponentRow {
  ComponentRow {
    components: row.components.iter().map(component).collect(),
  }
}

fn component(c: &ActionRowComponent) -> Component {
  match c {
    ActionRowComponent::Button(button) => {
      let (style, url) = match &button.data {
        ButtonKind::Link { url } => (ButtonStyle::Link, Some(url.clone())),
        ButtonKind::NonLink { style, .. } => (button_style(*style), None),
        // Premium (SKU) buttons render like primary buttons.
        _ => (ButtonStyle::Primary, None),
      };

      Component::Button {
        label: button.label.clone(),
        style,
        url,
        emoji: button.emoji.as_ref().map(emoji),
        disabled: button.disabled,
      }
    }
    ActionRowComponent::SelectMenu(select) => Component::Select {
      placeholder: select.placeholder.clone(),
      disabled: select.disabled,
    },
    _ => Component::Other,
  }
}

fn button_style(style: serenity::all::ButtonStyle) -> ButtonStyle {
  match style {
    serenity::all::ButtonStyle::Primary => ButtonStyle::Primary,
    serenity::all::ButtonStyle::Secondary => ButtonStyle::Secondary,
    serenity::all::ButtonStyle::Success => ButtonStyle::Success,
    serenity::all::ButtonStyle::Danger => ButtonStyle::Danger,
    _ => ButtonStyle::Secondary,
  }
}

// ---- misc ----------------------------------------------------------------------------

fn to_chrono(t: Timestamp) -> Option<DateTime<Utc>> {
  DateTime::from_timestamp_millis(t.timestamp_millis())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn system_kinds_map_every_notice_type() {
    assert_eq!(system_kind(MessageType::MemberJoin, ""), SystemKind::MemberJoin);
    assert_eq!(system_kind(MessageType::PinsAdd, ""), SystemKind::PinsAdd);
    assert_eq!(system_kind(MessageType::NitroBoost, ""), SystemKind::Boost { tier: None });
    assert_eq!(system_kind(MessageType::NitroTier1, ""), SystemKind::Boost { tier: Some(1) });
    assert_eq!(system_kind(MessageType::NitroTier2, ""), SystemKind::Boost { tier: Some(2) });
    assert_eq!(system_kind(MessageType::NitroTier3, ""), SystemKind::Boost { tier: Some(3) });
    assert_eq!(
      system_kind(MessageType::ThreadCreated, "help-me"),
      SystemKind::ThreadCreated { name: "help-me".into() }
    );
    assert_eq!(system_kind(MessageType::ChannelFollowAdd, ""), SystemKind::ChannelFollowAdd);
    assert_eq!(system_kind(MessageType::GroupRecipientAddition, ""), SystemKind::GroupRecipientAdd);
    assert_eq!(system_kind(MessageType::GroupRecipientRemoval, ""), SystemKind::GroupRecipientRemove);
    assert_eq!(system_kind(MessageType::GroupNameUpdate, ""), SystemKind::GroupNameUpdate);
    assert_eq!(system_kind(MessageType::GroupIconUpdate, ""), SystemKind::GroupIconUpdate);
    assert_eq!(system_kind(MessageType::GroupCallCreation, ""), SystemKind::Call);
    assert_eq!(system_kind(MessageType::AutoModAction, ""), SystemKind::Other("AutoModAction".into()));
    assert_eq!(system_kind(MessageType::StageStart, ""), SystemKind::Other("StageStart".into()));
  }

  #[test]
  fn regular_and_command_messages_are_default_unless_replying() {
    assert_eq!(message_kind(MessageType::Regular, "", false), MessageKind::Default);
    assert_eq!(message_kind(MessageType::ChatInputCommand, "", false), MessageKind::Default);
    assert_eq!(message_kind(MessageType::ContextMenuCommand, "", false), MessageKind::Default);
    assert_eq!(message_kind(MessageType::Regular, "", true), MessageKind::Reply);
    assert_eq!(message_kind(MessageType::InlineReply, "", false), MessageKind::Reply);
    assert_eq!(
      message_kind(MessageType::MemberJoin, "", true),
      MessageKind::System(SystemKind::MemberJoin)
    );
  }

  #[test]
  fn embed_kind_parses_discord_type_strings() {
    assert_eq!(embed_kind("rich"), EmbedKind::Rich);
    assert_eq!(embed_kind("image"), EmbedKind::Image);
    assert_eq!(embed_kind("video"), EmbedKind::Video);
    assert_eq!(embed_kind("gifv"), EmbedKind::Gifv);
    assert_eq!(embed_kind("article"), EmbedKind::Article);
    assert_eq!(embed_kind("link"), EmbedKind::Link);
    assert_eq!(embed_kind("something-new"), EmbedKind::Rich);
  }

  #[test]
  fn snippet_takes_first_non_blank_line() {
    assert_eq!(snippet("hello\nworld").as_deref(), Some("hello"));
    assert_eq!(snippet("\n\n  second  \nthird").as_deref(), Some("second"));
    assert_eq!(snippet(""), None);
    assert_eq!(snippet("   \n\t\n"), None);
  }

  #[test]
  fn snippet_truncates_long_lines_by_chars() {
    let exact: String = "a".repeat(SNIPPET_MAX_CHARS);
    assert_eq!(snippet(&exact).as_deref(), Some(exact.as_str()));

    let long: String = "é".repeat(SNIPPET_MAX_CHARS + 5);
    let cut = snippet(&long).unwrap();
    assert_eq!(cut.chars().count(), SNIPPET_MAX_CHARS + 1);
    assert!(cut.ends_with('…'));
    assert!(cut.starts_with(&"é".repeat(SNIPPET_MAX_CHARS)));
  }

  #[test]
  fn emoji_from_reaction_types() {
    assert_eq!(emoji(&ReactionType::Unicode("🔥".into())), Emoji::Unicode("🔥".into()));
    assert_eq!(
      emoji(&ReactionType::Custom {
        animated: true,
        id: EmojiId::new(42),
        name: Some("party".into())
      }),
      Emoji::Custom {
        id: 42,
        name: "party".into(),
        animated: true,
        url: None
      }
    );
    assert_eq!(
      emoji(&ReactionType::Custom {
        animated: false,
        id: EmojiId::new(7),
        name: None
      }),
      Emoji::Custom {
        id: 7,
        name: UNNAMED_EMOJI.into(),
        animated: false,
        url: None
      }
    );
  }

  #[test]
  fn reaction_type_from_emoji() {
    assert_eq!(reaction_type(&Emoji::Unicode("🔥".into())), ReactionType::Unicode("🔥".into()));
    assert_eq!(
      reaction_type(&Emoji::Custom {
        id: 42,
        name: "party".into(),
        animated: true,
        url: Some("https://example.invalid/party.gif".into()),
      }),
      ReactionType::Custom {
        animated: true,
        id: EmojiId::new(42),
        name: Some("party".into()),
      }
    );
  }

  #[test]
  fn reaction_type_round_trips_through_emoji() {
    for original in [
      Emoji::Unicode("👍".into()),
      Emoji::Custom {
        id: 7,
        name: "wave".into(),
        animated: false,
        url: None,
      },
    ] {
      assert_eq!(emoji(&reaction_type(&original)), original);
    }
  }

  #[test]
  fn unresolved_reply_ref_keeps_the_id() {
    let reply = unresolved_reply_ref(99);
    assert_eq!(reply.message_id, Some(99));
    assert!(!reply.deleted);
    assert!(reply.snippet.is_empty());
  }

  #[test]
  fn button_styles_map_and_unknown_falls_back() {
    assert_eq!(button_style(serenity::all::ButtonStyle::Primary), ButtonStyle::Primary);
    assert_eq!(button_style(serenity::all::ButtonStyle::Secondary), ButtonStyle::Secondary);
    assert_eq!(button_style(serenity::all::ButtonStyle::Success), ButtonStyle::Success);
    assert_eq!(button_style(serenity::all::ButtonStyle::Danger), ButtonStyle::Danger);
    assert_eq!(button_style(serenity::all::ButtonStyle::Unknown(99)), ButtonStyle::Secondary);
  }
}
