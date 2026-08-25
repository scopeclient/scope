//! Everything a chat message can contain, independent of the backend.

use chrono::{DateTime, Utc};

/// A fully described message body, ready to render.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichMessage {
  pub kind: MessageKind,
  /// The parsed text body (Discord-flavoured markdown → blocks).
  pub blocks: Vec<Block>,
  pub attachments: Vec<Attachment>,
  pub embeds: Vec<Embed>,
  pub stickers: Vec<Sticker>,
  pub reactions: Vec<Reaction>,
  pub poll: Option<Poll>,
  /// The message this one replies to, if any.
  pub reply: Option<ReplyRef>,
  /// Interactive components (buttons, selects); rendered read-only.
  pub components: Vec<ComponentRow>,
  pub edited_at: Option<DateTime<Utc>>,
  pub pinned: bool,
  /// Optimistic message that the server has not echoed back yet.
  pub pending: bool,
  /// The original markdown source (copy / search).
  pub source: String,
}

impl RichMessage {
  /// Plain text body with no resolver for mentions.
  pub fn plain(text: impl Into<String>) -> Self {
    let source = text.into();
    RichMessage {
      blocks: crate::markdown::parse(&source, &crate::markdown::NoResolver),
      source,
      ..Default::default()
    }
  }

  pub fn pending(text: impl Into<String>) -> Self {
    RichMessage {
      pending: true,
      ..Self::plain(text)
    }
  }

  /// True when there is nothing but the text body.
  pub fn is_text_only(&self) -> bool {
    self.attachments.is_empty()
      && self.embeds.is_empty()
      && self.stickers.is_empty()
      && self.reactions.is_empty()
      && self.poll.is_none()
      && self.components.is_empty()
  }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MessageKind {
  #[default]
  Default,
  Reply,
  /// Server-generated notice rendered as a single muted line.
  System(SystemKind),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemKind {
  MemberJoin,
  PinsAdd,
  Boost { tier: Option<u8> },
  ThreadCreated { name: String },
  ChannelFollowAdd,
  GroupRecipientAdd,
  GroupRecipientRemove,
  GroupNameUpdate,
  GroupIconUpdate,
  Call,
  Other(String),
}

// ---- text body --------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
  Paragraph(Vec<Inline>),
  /// `#`, `##`, `###` headings (level 1–3).
  Heading {
    level: u8,
    content: Vec<Inline>,
  },
  /// `-# small muted text`.
  Subtext(Vec<Inline>),
  /// `> quoted` (consecutive lines merge into one quote).
  Quote(Vec<Block>),
  /// Fenced ``` code ``` block.
  CodeBlock {
    language: Option<String>,
    code: String,
  },
  List {
    ordered: bool,
    start: u32,
    items: Vec<Vec<Block>>,
  },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Inline {
  Text(String),
  Styled {
    style: TextStyle,
    content: Vec<Inline>,
  },
  /// `||spoiler||` — hidden until clicked.
  Spoiler(Vec<Inline>),
  /// `` `inline code` ``.
  Code(String),
  /// Bare URL or `[label](url)`; `label == None` shows the URL.
  Link {
    url: String,
    label: Option<Vec<Inline>>,
  },
  Mention(Mention),
  Emoji(Emoji),
  /// `<t:unix:style>`.
  Timestamp {
    unix: i64,
    style: TimestampStyle,
  },
  LineBreak,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyle {
  Bold,
  Italic,
  Underline,
  Strikethrough,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mention {
  User {
    id: u64,
    name: String,
  },
  Channel {
    id: u64,
    name: String,
  },
  Role {
    id: u64,
    name: String,
    color: Option<u32>,
  },
  /// `</name:id>` slash-command mention.
  Command {
    name: String,
  },
  Everyone,
  Here,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Emoji {
  Unicode(String),
  Custom {
    id: u64,
    name: String,
    animated: bool,
    /// Backend-supplied image; `None` falls back to the Discord CDN.
    url: Option<String>,
  },
}

impl Emoji {
  /// CDN image for custom emoji; unicode emoji render as text.
  pub fn image_url(&self, size: u32) -> Option<String> {
    match self {
      Emoji::Unicode(_) => None,
      Emoji::Custom { url: Some(url), .. } => Some(url.clone()),
      Emoji::Custom { id, animated, url: None, .. } => {
        let ext = if *animated { "gif" } else { "png" };
        Some(format!("https://cdn.discordapp.com/emojis/{id}.{ext}?size={size}&quality=lossless"))
      }
    }
  }

  pub fn label(&self) -> String {
    match self {
      Emoji::Unicode(s) => s.clone(),
      Emoji::Custom { name, .. } => format!(":{name}:"),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TimestampStyle {
  ShortTime,
  LongTime,
  ShortDate,
  LongDate,
  ShortDateTime,
  LongDateTime,
  #[default]
  Default,
  Relative,
}

// ---- attachments --------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Attachment {
  pub id: u64,
  pub filename: String,
  pub url: String,
  pub proxy_url: Option<String>,
  pub content_type: Option<String>,
  pub size_bytes: u64,
  pub width: Option<u32>,
  pub height: Option<u32>,
  /// Alt text.
  pub description: Option<String>,
  pub spoiler: bool,
  /// Present for voice messages.
  pub voice: Option<VoiceClip>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceClip {
  pub duration_secs: f32,
  /// Amplitude samples, 0–255, as Discord sends them.
  pub waveform: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentKind {
  Image,
  Video,
  Audio,
  Voice,
  File,
}

impl Attachment {
  pub fn kind(&self) -> AttachmentKind {
    if self.voice.is_some() {
      return AttachmentKind::Voice;
    }

    let mime = self.content_type.as_deref().unwrap_or("").to_ascii_lowercase();
    let ext = self.filename.rsplit('.').next().unwrap_or("").to_ascii_lowercase();

    if mime.starts_with("image/") || matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif") {
      AttachmentKind::Image
    } else if mime.starts_with("video/") || matches!(ext.as_str(), "mp4" | "webm" | "mov") {
      AttachmentKind::Video
    } else if mime.starts_with("audio/") || matches!(ext.as_str(), "mp3" | "ogg" | "wav" | "flac" | "m4a") {
      AttachmentKind::Audio
    } else {
      AttachmentKind::File
    }
  }

  /// Human readable size, e.g. `1.2 MB`.
  pub fn size_label(&self) -> String {
    let bytes = self.size_bytes as f64;
    if bytes < 1024. {
      format!("{} B", self.size_bytes)
    } else if bytes < 1024. * 1024. {
      format!("{:.1} KB", bytes / 1024.)
    } else if bytes < 1024. * 1024. * 1024. {
      format!("{:.1} MB", bytes / 1024. / 1024.)
    } else {
      format!("{:.2} GB", bytes / 1024. / 1024. / 1024.)
    }
  }
}

// ---- embeds ------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Embed {
  pub kind: EmbedKind,
  pub title: Option<String>,
  pub url: Option<String>,
  pub description: Option<Vec<Block>>,
  /// `0xRRGGBB` accent colour of the left bar.
  pub color: Option<u32>,
  pub author: Option<EmbedAuthor>,
  pub provider: Option<EmbedProvider>,
  pub thumbnail: Option<EmbedMedia>,
  pub image: Option<EmbedMedia>,
  pub video: Option<EmbedMedia>,
  pub fields: Vec<EmbedField>,
  pub footer: Option<EmbedFooter>,
  pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EmbedKind {
  #[default]
  Rich,
  Image,
  Video,
  Gifv,
  Article,
  Link,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedAuthor {
  pub name: String,
  pub url: Option<String>,
  pub icon_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedProvider {
  pub name: Option<String>,
  pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedMedia {
  pub url: String,
  pub proxy_url: Option<String>,
  pub width: Option<u32>,
  pub height: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmbedField {
  pub name: String,
  pub value: Vec<Block>,
  pub inline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedFooter {
  pub text: String,
  pub icon_url: Option<String>,
}

// ---- stickers, reactions, polls, replies, components -----------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sticker {
  pub id: u64,
  pub name: String,
  pub format: StickerFormat,
  /// Image URL for PNG/APNG/GIF stickers; Lottie stickers have none.
  pub url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StickerFormat {
  Png,
  Apng,
  Lottie,
  Gif,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reaction {
  pub emoji: Emoji,
  pub count: u64,
  /// The signed-in user reacted.
  pub me: bool,
  pub burst: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Poll {
  pub question: String,
  pub answers: Vec<PollAnswer>,
  pub total_votes: u64,
  pub expires_at: Option<DateTime<Utc>>,
  pub allow_multiselect: bool,
  pub finalized: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PollAnswer {
  pub id: u64,
  pub text: String,
  pub emoji: Option<Emoji>,
  pub votes: u64,
  pub me_voted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplyRef {
  pub message_id: Option<u64>,
  pub author_name: String,
  pub author_avatar: Option<String>,
  /// First line of the referenced message, already flattened to plain text.
  pub snippet: String,
  /// The referenced message no longer exists.
  pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentRow {
  pub components: Vec<Component>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Component {
  Button {
    label: Option<String>,
    style: ButtonStyle,
    url: Option<String>,
    emoji: Option<Emoji>,
    disabled: bool,
  },
  Select {
    placeholder: Option<String>,
    disabled: bool,
  },
  Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonStyle {
  Primary,
  Secondary,
  Success,
  Danger,
  Link,
}
