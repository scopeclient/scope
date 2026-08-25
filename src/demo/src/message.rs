use std::sync::Arc;

use chrono::{DateTime, Utc};
use gpui::{App, Entity, Hsla, IntoElement, ObjectFit, ParentElement, RenderOnce, SharedString, Styled, StyledImage as _, Window, div, img, px};
use scope_chat::{
  async_list::AsyncListItem,
  message::{IconRenderConfig, Message, MessageAuthor},
  nav::Id,
};
use scope_rich::{ContentCell, Emoji, MessageKind, Reaction, ReplyRef, RichContentView, RichMessage, markdown};

use crate::data;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemoAuthor {
  pub id: Id,
  pub name: SharedString,
  /// Embedded asset path; `None` draws a coloured initial instead.
  pub avatar: Option<SharedString>,
}

impl DemoAuthor {
  pub fn from_id(id: Id) -> Self {
    let person = data::person(id);
    let name = if id == data::SELF_ID {
      data::current_user().display_name
    } else {
      person.map(|p| p.name.to_string()).unwrap_or_else(|| format!("user-{}", id.0))
    };
    DemoAuthor {
      id,
      name: name.into(),
      avatar: person.and_then(|p| p.avatar).map(Into::into),
    }
  }
}

impl MessageAuthor for DemoAuthor {
  type Identifier = Id;
  type DisplayName = DemoName;
  type Icon = DemoAvatar;

  fn get_display_name(&self) -> Self::DisplayName {
    DemoName(self.name.clone())
  }

  fn get_icon(&self, config: IconRenderConfig) -> Self::Icon {
    DemoAvatar {
      id: self.id,
      initial: self.name.chars().next().unwrap_or('?').to_ascii_uppercase(),
      size: config.size(),
      url: self.avatar.clone(),
    }
  }

  fn get_identifier(&self) -> Self::Identifier {
    self.id
  }
}

#[derive(Clone, IntoElement, Debug)]
pub struct DemoName(pub SharedString);

impl RenderOnce for DemoName {
  fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
    div().truncate().child(self.0)
  }
}

/// The author's avatar image when they have one, otherwise a coloured circle with their initial.
#[derive(Clone, IntoElement, Debug)]
pub struct DemoAvatar {
  pub id: Id,
  pub initial: char,
  pub size: usize,
  pub url: Option<SharedString>,
}

pub fn avatar_color(id: Id) -> Hsla {
  let hue = ((id.0 * 47) % 360) as f32 / 360.;
  Hsla {
    h: hue,
    s: 0.45,
    l: 0.45,
    a: 1.,
  }
}

impl RenderOnce for DemoAvatar {
  fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
    if let Some(url) = self.url {
      return img(url).size_full().rounded_full().object_fit(ObjectFit::Cover).into_any_element();
    }

    div()
      .size_full()
      .rounded_full()
      .bg(avatar_color(self.id))
      .flex()
      .items_center()
      .justify_center()
      .text_color(gpui::white())
      .text_size(px((self.size as f32 * 0.45).clamp(9., 18.)))
      .font_weight(gpui::FontWeight::BOLD)
      .child(self.initial.to_string())
      .into_any_element()
  }
}

#[derive(Clone)]
pub struct DemoMessage {
  /// Stable id; pending messages get a random one that is replaced on confirmation.
  pub id: Id,
  pub author: DemoAuthor,
  pub content: String,
  pub timestamp: DateTime<Utc>,
  pub nonce: Option<String>,
  pub pending: bool,
  /// Pre-built rich body for showcase messages (and anything the server has
  /// since touched); `None` means "parse `content`".
  pub rich: Option<Arc<RichMessage>>,
  content_cell: ContentCell,
}

impl DemoMessage {
  pub fn new(id: Id, author: Id, content: impl Into<String>, timestamp: DateTime<Utc>) -> Self {
    DemoMessage {
      id,
      author: DemoAuthor::from_id(author),
      content: content.into(),
      timestamp,
      nonce: None,
      pending: false,
      rich: None,
      content_cell: ContentCell::new(),
    }
  }

  pub fn pending(content: String, nonce: String) -> Self {
    DemoMessage {
      id: Id(rand::random()),
      author: DemoAuthor::from_id(data::SELF_ID),
      content,
      timestamp: Utc::now(),
      nonce: Some(nonce),
      pending: true,
      rich: None,
      content_cell: ContentCell::new(),
    }
  }

  /// Optimistic reply: like [`Self::pending`], but quoting `reply`.
  pub fn pending_reply(content: String, nonce: String, reply: ReplyRef) -> Self {
    let rich = RichMessage {
      kind: MessageKind::Reply,
      reply: Some(reply),
      pending: true,
      ..data::rich_text(content.clone())
    };
    DemoMessage {
      rich: Some(Arc::new(rich)),
      ..Self::pending(content, nonce)
    }
  }

  pub fn confirmed(mut self, id: Id) -> Self {
    self.id = id;
    self.pending = false;
    if let Some(rich) = &mut self.rich {
      Arc::make_mut(rich).pending = false;
    }
    self.content_cell = ContentCell::new();
    self
  }

  /// Attach a pre-built rich body (attachments, embeds, polls, …).
  pub fn with_rich(mut self, rich: RichMessage) -> Self {
    self.rich = Some(Arc::new(rich));
    self
  }

  /// The rich body, parsed from `content` when no pre-built one is attached.
  pub fn body(&self) -> RichMessage {
    match &self.rich {
      Some(rich) => (**rich).clone(),
      None => RichMessage {
        pending: self.pending,
        ..data::rich_text(self.content.clone())
      },
    }
  }

  /// Swap in a new body and drop the cached renderer so the next paint shows it.
  fn set_body(&mut self, rich: RichMessage) {
    self.rich = Some(Arc::new(rich));
    self.content_cell = ContentCell::new();
  }

  pub fn is_system(&self) -> bool {
    matches!(
      self.rich.as_deref(),
      Some(RichMessage {
        kind: MessageKind::System(_),
        ..
      })
    )
  }

  pub fn is_edited(&self) -> bool {
    self.rich.as_deref().is_some_and(|rich| rich.edited_at.is_some())
  }

  /// The message this one replies to, if any.
  pub fn reply_target(&self) -> Option<Id> {
    self.rich.as_deref().and_then(|rich| rich.reply.as_ref()).and_then(|reply| reply.message_id).map(Id)
  }

  /// Replace the text, keeping attachments, reactions, embeds and everything else.
  pub fn edit(&mut self, content: impl Into<String>, now: DateTime<Utc>) {
    let content = content.into();
    let text = data::rich_text(content.clone());
    let body = RichMessage {
      blocks: text.blocks,
      source: text.source,
      edited_at: Some(now),
      ..self.body()
    };
    self.content = content;
    self.set_body(body);
  }

  /// Add one reaction; `me` when the signed-in user is the one reacting.
  /// Returns whether anything changed.
  pub fn react(&mut self, emoji: Emoji, me: bool, user: Option<&str>) -> bool {
    let mut body = self.body();
    let changed = add_reaction(&mut body.reactions, emoji, me, user);
    if changed {
      self.set_body(body);
    }
    changed
  }

  /// Take one reaction away; the inverse of [`Self::react`].
  pub fn unreact(&mut self, emoji: &Emoji, me: bool, user: Option<&str>) -> bool {
    let mut body = self.body();
    let changed = remove_reaction(&mut body.reactions, emoji, me, user);
    if changed {
      self.set_body(body);
    }
    changed
  }

  /// The message this one quotes was deleted; show the placeholder instead.
  pub fn orphan_reply(&mut self) -> bool {
    let mut body = self.body();
    let Some(reply) = body.reply.as_mut() else { return false };
    if reply.deleted {
      return false;
    }
    reply.deleted = true;
    reply.snippet.clear();
    self.set_body(body);
    true
  }

  /// How this message appears when quoted by a reply.
  pub fn reply_ref(&self) -> ReplyRef {
    let body = self.body();
    let text = markdown::to_plain_text(&body.blocks);
    let snippet = text.lines().map(str::trim).find(|line| !line.is_empty()).map(str::to_owned).unwrap_or_else(|| {
      if body.is_text_only() {
        String::new()
      } else {
        "Click to see attachment".into()
      }
    });

    ReplyRef {
      message_id: Some(self.id.0),
      author_name: self.author.name.to_string(),
      author_avatar: self.author.avatar.as_ref().map(ToString::to_string),
      snippet,
      deleted: false,
    }
  }
}

/// Reaction pill arithmetic, shared by the user's own toggles and the live feed.
/// Adding a reaction the user already has is a no-op, like the real server.
pub fn add_reaction(reactions: &mut Vec<Reaction>, emoji: Emoji, me: bool, user: Option<&str>) -> bool {
  match reactions.iter_mut().find(|r| r.emoji == emoji) {
    Some(pill) if me && pill.me => false,
    Some(pill) => {
      pill.count += 1;
      pill.me |= me;
      remember_reactor(pill, user);
      true
    }
    None => {
      let mut pill = Reaction {
        emoji,
        count: 1,
        me,
        burst: false,
        users: Vec::new(),
      };
      remember_reactor(&mut pill, user);
      reactions.push(pill);
      true
    }
  }
}

/// Removing a reaction the user never added is a no-op; pills disappear at zero.
fn remember_reactor(pill: &mut Reaction, user: Option<&str>) {
  if let Some(name) = user {
    pill.users.retain(|existing| existing != name);
    pill.users.insert(0, name.to_string());
    pill.users.truncate(8);
  }
}

pub fn remove_reaction(reactions: &mut Vec<Reaction>, emoji: &Emoji, me: bool, user: Option<&str>) -> bool {
  let Some(index) = reactions.iter().position(|r| r.emoji == *emoji) else {
    return false;
  };
  let pill = &mut reactions[index];
  if me && !pill.me {
    return false;
  }
  pill.count = pill.count.saturating_sub(1);
  if me {
    pill.me = false;
  }
  if let Some(name) = user {
    pill.users.retain(|existing| existing != name);
  }
  if pill.count == 0 {
    reactions.remove(index);
  }
  true
}

/// Nonces only match when both sides have one.
#[derive(Clone, Copy)]
pub struct Nonce<'a>(Option<&'a str>);

impl PartialEq for Nonce<'_> {
  fn eq(&self, other: &Self) -> bool {
    matches!((self.0, other.0), (Some(a), Some(b)) if a == b)
  }
}

impl Message for DemoMessage {
  type Identifier = Id;
  type Author = DemoAuthor;

  fn is_own(&self) -> bool {
    self.author.id == data::SELF_ID
  }

  fn get_author(&self) -> Self::Author {
    self.author.clone()
  }

  fn get_content(&self, _window: &mut Window, cx: &mut App) -> Entity<RichContentView> {
    self.content_cell.get_or_create(cx, || match &self.rich {
      Some(rich) => rich.clone(),
      None => Arc::new(self.body()),
    })
  }

  fn get_identifier(&self) -> Option<<Self as Message>::Identifier> {
    if self.pending { None } else { Some(self.id) }
  }

  fn get_nonce(&self) -> impl PartialEq {
    Nonce(self.nonce.as_deref())
  }

  fn should_group(&self, previous: &Self) -> bool {
    const MAX_GAP_SECS: i64 = 5 * 60;
    self.timestamp.signed_duration_since(previous.timestamp).num_seconds().abs() <= MAX_GAP_SECS
  }

  fn get_timestamp(&self) -> Option<DateTime<Utc>> {
    Some(self.timestamp)
  }
}

impl AsyncListItem for DemoMessage {
  type Identifier = Id;

  fn get_list_identifier(&self) -> Self::Identifier {
    self.id
  }
}

#[cfg(test)]
mod tests {
  use scope_rich::Attachment;

  use super::*;

  fn thumbs() -> Emoji {
    Emoji::Unicode("👍".into())
  }

  fn fire() -> Emoji {
    Emoji::Unicode("🔥".into())
  }

  fn pill(emoji: Emoji, count: u64, me: bool) -> Reaction {
    Reaction {
      emoji,
      count,
      me,
      burst: false,
      users: Vec::new(),
    }
  }

  #[test]
  fn add_reaction_appends_a_new_pill() {
    let mut reactions = vec![];
    assert!(add_reaction(&mut reactions, thumbs(), true, None));
    assert_eq!(reactions, vec![pill(thumbs(), 1, true)]);

    assert!(add_reaction(&mut reactions, fire(), false, None));
    assert_eq!(reactions, vec![pill(thumbs(), 1, true), pill(fire(), 1, false)]);
  }

  #[test]
  fn add_reaction_bumps_existing_pill_and_marks_me() {
    let mut reactions = vec![pill(thumbs(), 3, false)];
    assert!(add_reaction(&mut reactions, thumbs(), false, None));
    assert_eq!(reactions, vec![pill(thumbs(), 4, false)]);

    assert!(add_reaction(&mut reactions, thumbs(), true, None));
    assert_eq!(reactions, vec![pill(thumbs(), 5, true)]);
  }

  #[test]
  fn add_reaction_twice_by_me_is_a_no_op() {
    let mut reactions = vec![pill(thumbs(), 1, true)];
    assert!(!add_reaction(&mut reactions, thumbs(), true, None));
    assert_eq!(reactions, vec![pill(thumbs(), 1, true)]);
  }

  #[test]
  fn remove_reaction_decrements_and_drops_empty_pills() {
    let mut reactions = vec![pill(thumbs(), 2, true), pill(fire(), 1, false)];
    assert!(remove_reaction(&mut reactions, &thumbs(), true, None));
    assert_eq!(reactions, vec![pill(thumbs(), 1, false), pill(fire(), 1, false)]);

    assert!(remove_reaction(&mut reactions, &fire(), false, None));
    assert_eq!(reactions, vec![pill(thumbs(), 1, false)]);
  }

  #[test]
  fn remove_reaction_ignores_missing_or_not_mine() {
    let mut reactions = vec![pill(thumbs(), 2, false)];
    assert!(!remove_reaction(&mut reactions, &fire(), false, None));
    assert!(!remove_reaction(&mut reactions, &thumbs(), true, None));
    assert_eq!(reactions, vec![pill(thumbs(), 2, false)]);
  }

  #[test]
  fn react_toggle_round_trips_on_a_plain_message() {
    let mut message = DemoMessage::new(Id(10), Id(2), "hello", Utc::now());
    assert!(message.rich.is_none());

    assert!(message.react(thumbs(), true, None));
    assert_eq!(message.body().reactions, vec![pill(thumbs(), 1, true)]);
    assert_eq!(message.body().source, "hello", "reacting keeps the text");

    assert!(message.unreact(&thumbs(), true, None));
    assert!(message.body().reactions.is_empty());
  }

  #[test]
  fn edit_rewrites_text_and_keeps_everything_else() {
    let attachment = Attachment {
      id: 1,
      filename: "a.png".into(),
      url: "demo/a.png".into(),
      proxy_url: None,
      content_type: Some("image/png".into()),
      size_bytes: 10,
      width: None,
      height: None,
      description: None,
      spoiler: false,
      voice: None,
    };
    let rich = RichMessage {
      attachments: vec![attachment.clone()],
      reactions: vec![pill(fire(), 4, false)],
      ..data::rich_text("teh fix")
    };
    let mut message = DemoMessage::new(Id(10), Id(2), "teh fix", Utc::now()).with_rich(rich);

    let now = Utc::now();
    message.edit("the fix", now);

    let body = message.body();
    assert_eq!(message.content, "the fix");
    assert_eq!(body.source, "the fix");
    assert_eq!(body.blocks, data::rich_text("the fix").blocks);
    assert_eq!(body.edited_at, Some(now));
    assert_eq!(body.attachments, vec![attachment]);
    assert_eq!(body.reactions, vec![pill(fire(), 4, false)]);
    assert!(message.is_edited());
  }

  #[test]
  fn reply_ref_uses_first_plain_text_line() {
    let message = DemoMessage::new(Id(77), Id(2), "\n**bold** start\nsecond line", Utc::now());
    let reply = message.reply_ref();
    assert_eq!(reply.message_id, Some(77));
    assert_eq!(reply.author_name, "zach");
    assert_eq!(reply.snippet, "bold start");
    assert!(!reply.deleted);
  }

  #[test]
  fn pending_reply_confirms_without_pending_flag() {
    let target = DemoMessage::new(Id(5), Id(2), "ship it", Utc::now());
    let pending = DemoMessage::pending_reply("yes".into(), "n1".into(), target.reply_ref());
    assert!(pending.body().pending);
    assert_eq!(pending.body().kind, MessageKind::Reply);
    assert_eq!(pending.reply_target(), Some(Id(5)));

    let confirmed = pending.confirmed(Id(6));
    assert!(!confirmed.pending);
    assert!(!confirmed.body().pending);
    assert_eq!(confirmed.body().reply.as_ref().map(|r| r.snippet.as_str()), Some("ship it"));
  }

  #[test]
  fn orphan_reply_marks_the_quote_deleted_once() {
    let target = DemoMessage::new(Id(5), Id(2), "ship it", Utc::now());
    let mut reply = DemoMessage::pending_reply("yes".into(), "n1".into(), target.reply_ref()).confirmed(Id(6));
    assert!(reply.orphan_reply());
    let quote = reply.body().reply.unwrap();
    assert!(quote.deleted);
    assert!(quote.snippet.is_empty());
    assert!(!reply.orphan_reply());
  }
}
