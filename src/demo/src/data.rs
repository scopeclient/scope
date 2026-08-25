//! Seed content for the demo backend, lifted from the Figma mockups.

use chrono::{DateTime, Duration, Utc};
use scope_chat::nav::{ChannelInfo, ChannelKind, GuildInfo, Id, MemberInfo, MemberSection, Presence, UserInfo};
use scope_rich::markdown::{MentionResolver, parse};
use scope_rich::{
  Attachment, Block, ButtonStyle, Component, ComponentRow, Embed, EmbedAuthor, EmbedField, EmbedFooter, EmbedKind, EmbedMedia, EmbedProvider, Emoji,
  MessageKind, Poll, PollAnswer, Reaction, ReplyRef, RichMessage, Sticker, StickerFormat, SystemKind, VoiceClip, markdown,
};

use crate::message::DemoMessage;

pub const SELF_ID: Id = Id(1);

pub struct Person {
  pub id: Id,
  pub name: &'static str,
  pub status: &'static str,
  pub presence: Presence,
  pub role_group: Option<&'static str>,
  /// Embedded asset path (`assets/demo/…`); most demo accounts fall back to an initial.
  pub avatar: Option<&'static str>,
}

pub const PEOPLE: &[Person] = &[
  Person {
    id: Id(2),
    name: "zach",
    status: "Constructing Chatrooms",
    presence: Presence::Online,
    role_group: Some("Admins"),
    avatar: Some("demo/avatar-a.png"),
  },
  Person {
    id: Id(3),
    name: "luke",
    status: "Constructing Chatrooms",
    presence: Presence::Online,
    role_group: Some("Admins"),
    avatar: Some("demo/avatar-b.png"),
  },
  Person {
    id: Id(4),
    name: "dfg",
    status: "Constructing Chatrooms",
    presence: Presence::Online,
    role_group: None,
    avatar: None,
  },
  Person {
    id: Id(5),
    name: "milky",
    status: "shipping scope",
    presence: Presence::Idle,
    role_group: None,
    avatar: None,
  },
  Person {
    id: Id(6),
    name: "sanae",
    status: "Do not disturb",
    presence: Presence::DoNotDisturb,
    role_group: None,
    avatar: None,
  },
  Person {
    id: Id(7),
    name: "rose",
    status: "away",
    presence: Presence::Offline,
    role_group: None,
    avatar: None,
  },
];

pub fn person(id: Id) -> Option<&'static Person> {
  PEOPLE.iter().find(|p| p.id == id)
}

pub fn current_user() -> UserInfo {
  UserInfo {
    id: SELF_ID,
    username: "user".into(),
    display_name: "user".into(),
    tag: "user#0001".into(),
    avatar_url: None,
    presence: Presence::Online,
    status_text: Some("building chatrooms".into()),
  }
}

pub fn guilds() -> Vec<GuildInfo> {
  let guild = |id: u64, name: &str| GuildInfo {
    id: Id(id),
    name: name.into(),
    // Embedded asset paths work anywhere `img()` accepts a URL.
    icon_url: match id {
      10 => Some("brand/placeholder-server-a.png".into()),
      11 => Some("brand/placeholder-server-b.png".into()),
      _ => None,
    },
    banner_url: None,
    member_count: Some(20),
    online_count: Some(2),
    unread: 0,
  };

  vec![
    guild(10, "Prism AIO"),
    guild(11, "Wrath"),
    guild(12, "Cyber"),
    guild(13, "Dashe"),
    guild(14, "Milk FNF"),
  ]
}

#[allow(clippy::too_many_arguments)]
fn channel(id: u64, guild: u64, name: &str, kind: ChannelKind, parent: Option<u64>, position: i64, unread: u32, muted: bool) -> ChannelInfo {
  ChannelInfo {
    id: Id(id),
    guild_id: Some(Id(guild)),
    name: name.into(),
    kind,
    parent_id: parent.map(Id),
    position,
    unread,
    muted,
    icon_url: None,
  }
}

pub fn channels(guild: Id) -> Vec<ChannelInfo> {
  match guild.0 {
    10 => {
      let mut out = vec![channel(100, 10, "information", ChannelKind::Category, None, 0, 0, false)];
      let rows = [
        ("welcome", 0, false),
        ("announcements", 0, false),
        ("dev-announcements", 0, false),
        ("release-announcements", 1, false),
        ("release-guides", 21, false),
        ("run-for-restocks", 0, true),
        ("updates", 5, false),
        ("guides-faq", 0, false),
        ("resources", 0, false),
        ("checkouts", 0, false),
      ];
      for (i, (name, unread, muted)) in rows.into_iter().enumerate() {
        out.push(channel(101 + i as u64, 10, name, ChannelKind::Text, Some(100), i as i64, unread, muted));
      }
      out.push(channel(120, 10, "voice", ChannelKind::Category, None, 1, 0, false));
      out.push(channel(121, 10, "lounge", ChannelKind::Voice, Some(120), 0, 0, false));
      out
    }
    g @ 11..=14 => vec![
      channel(g * 100, g, "general", ChannelKind::Text, None, 0, 5, false),
      channel(g * 100 + 1, g, "member-important", ChannelKind::Text, None, 1, 5, false),
      channel(g * 100 + 2, g, "off-topic", ChannelKind::Text, None, 2, 0, false),
    ],
    _ => Vec::new(),
  }
}

pub fn members(guild: Id) -> Vec<MemberInfo> {
  let everyone = PEOPLE.iter().map(|p| MemberInfo {
    id: p.id,
    display_name: p.name.into(),
    avatar_url: p.avatar.map(Into::into),
    presence: p.presence,
    status_text: Some(p.status.into()),
    role_group: p.role_group.map(Into::into),
  });

  let mut list: Vec<MemberInfo> = match guild.0 {
    10 => everyone.collect(),
    _ => everyone.take(3).collect(),
  };
  arrange_members(&mut list);
  list
}

/// Roles that "hoist" (display separately) in the demo server, highest first.
const HOISTED_ROLES: [&str; 1] = ["Admins"];

/// Discord's member-list order via [`MemberSection`]: hoisted roles, then
/// online, then offline — alphabetical inside each — and relabel `role_group`
/// to the section shown in the list.
pub fn arrange_members(members: &mut [MemberInfo]) {
  let section = |m: &MemberInfo| {
    let rank = m.role_group.as_deref().and_then(|group| HOISTED_ROLES.iter().position(|role| *role == group));
    MemberSection::of(rank, m.presence)
  };

  members.sort_by(|a, b| section(a).cmp(&section(b)).then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase())));

  for member in members {
    member.role_group = Some(match section(member) {
      MemberSection::Role(rank) => HOISTED_ROLES[rank].to_owned(),
      MemberSection::Online => "online".to_owned(),
      MemberSection::Offline => "offline".to_owned(),
    });
  }
}

pub fn dms() -> Vec<ChannelInfo> {
  PEOPLE
    .iter()
    .take(4)
    .enumerate()
    .map(|(i, p)| ChannelInfo {
      id: Id(900 + p.id.0),
      guild_id: None,
      name: p.name.into(),
      kind: ChannelKind::DirectMessage,
      parent_id: None,
      position: i as i64,
      unread: if i == 0 { 2 } else { 0 },
      muted: false,
      icon_url: p.avatar.map(Into::into),
    })
    .collect()
}

/// The conversation shown in the mockup's `#announcements`, oldest first: (author id, text, minutes ago).
pub fn announcements_history() -> Vec<(Id, &'static str, i64)> {
  vec![
    (Id(2), "i haven't started", 63),
    (Id(3), "i need the other one to hold you make you feel make you feel better", 61),
    (Id(2), "Can't count the years on one hand\nThat we've been together", 58),
    (Id(2), "Can't count the years on one hand\nThat we've been together", 52),
    (Id(3), "i need the other one to hold you make you feel make you feel better", 47),
    (
      Id(2),
      "It's not a walk in the park\nTo love each other\nBut when our fingers interlock\nCan't deny, can't deny you're worth it",
      41,
    ),
    (Id(3), "[Pre-Chorus]\n'Cause after all this time, I'm still into you", 35),
    (
      Id(2),
      "[Chorus]\nI should be over all the butterflies\nBut I'm into you (I'm into you)\nAnd baby even on our worst nights\nI'm into you (I'm into you)\nLet 'em wonder how we got this far\n'Cause I don't really need to wonder at all\nYeah, after all this time, I'm still into you",
      29,
    ),
    (Id(3), "hows the Scope landing page coming along", 12),
    (Id(2), "i haven't started", 11),
    (Id(3), "we literally launch tomorrow", 10),
  ]
}

/// Generic chatter for other channels and for the live feed.
pub const CHATTER: &[&str] = &[
  "anyone around?",
  "pushing a fix for the tab bar now",
  "lgtm",
  "brb coffee",
  "did the nightly build go out?",
  "the new member list looks clean",
  "can someone test wayland",
  "ok that's actually sick",
  "merge it",
  "who broke the cache tests",
  "not me",
  "scope one shot",
  "gpui 0.2 is so much nicer",
  "ship it",
  "reading the spec again",
  "we literally launch tomorrow",
  "i'll write the changelog",
  "hows the landing page",
  "i haven't started",
  "based",
];

/// Markdown-heavy one-liners the live feed occasionally posts.
pub const RICH_CHATTER: &[&str] = &[
  "**merged** `#47` — *finally*",
  "||the landing page is still not started||",
  "<@2> did you see https://github.com/scopeclient/scope/pull/44",
  "~~tomorrow~~ today we launch 🚀",
  "check <#102> for the lyrics",
  "> who broke the cache tests\nnot me",
  "release call <t:1700000000:R> <:scope:999>",
  "-# (this is a test of the subtext renderer)",
  "__underline__ and `inline code` in one line",
  "# big news\nnothing, just testing headings",
];

// ---- #dev-announcements showcase ---------------------------------------------------

/// Path of an embedded showcase image (`assets/demo/<file>`).
pub fn asset(file: &str) -> String {
  format!("demo/{file}")
}

fn blocks(text: &str) -> Vec<Block> {
  markdown::parse(text, &DemoResolver)
}

fn unicode(s: &str) -> Emoji {
  Emoji::Unicode(s.into())
}

fn scope_emoji() -> Emoji {
  Emoji::Custom {
    id: 999,
    name: "scope".into(),
    animated: false,
    url: Some(asset("emoji-scope.png")),
  }
}

/// Resolves mentions and custom emoji against the demo world, so nothing
/// in demo mode reaches out to Discord's CDN.
pub struct DemoResolver;

impl MentionResolver for DemoResolver {
  fn user(&self, id: u64) -> Option<String> {
    if Id(id) == SELF_ID {
      return Some(current_user().display_name);
    }
    person(Id(id)).map(|p| p.name.to_string())
  }

  fn channel(&self, id: u64) -> Option<String> {
    guilds().iter().flat_map(|g| channels(g.id)).chain(dms()).find(|c| c.id == Id(id)).map(|c| c.name)
  }

  fn role(&self, _id: u64) -> Option<(String, Option<u32>)> {
    None
  }

  fn custom_emoji_url(&self, _id: u64, _animated: bool) -> Option<String> {
    Some(asset("emoji-scope.png"))
  }
}

/// Parse a markdown body against the demo resolver.
pub fn rich_text(text: impl Into<String>) -> RichMessage {
  let source = text.into();
  RichMessage {
    blocks: parse(&source, &DemoResolver),
    source,
    ..Default::default()
  }
}

fn attachment(id: u64, filename: &str, url: &str, content_type: Option<&str>, size_bytes: u64) -> Attachment {
  Attachment {
    id,
    filename: filename.into(),
    url: url.into(),
    proxy_url: None,
    content_type: content_type.map(Into::into),
    size_bytes,
    width: None,
    height: None,
    description: None,
    spoiler: false,
    voice: None,
  }
}

/// An image attachment backed by one of the embedded showcase PNGs.
pub fn image_attachment(id: u64, file: &str, width: u32, height: u32, size_bytes: u64) -> Attachment {
  Attachment {
    width: Some(width),
    height: Some(height),
    description: Some(format!("demo image {file}")),
    ..attachment(id, file, &asset(file), Some("image/png"), size_bytes)
  }
}

fn media(file: &str, width: u32, height: u32) -> EmbedMedia {
  EmbedMedia {
    url: asset(file),
    proxy_url: None,
    width: Some(width),
    height: Some(height),
  }
}

fn button(label: &str, style: ButtonStyle, disabled: bool) -> Component {
  Component::Button {
    label: Some(label.into()),
    style,
    url: None,
    emoji: None,
    disabled,
  }
}

/// Reactions for a message that went down well.
pub fn hot_reactions(fire: u64, me: bool) -> Vec<Reaction> {
  vec![
    Reaction {
      emoji: unicode("🔥"),
      count: fire,
      me,
      burst: false,
      users: vec!["zach".into(), "luke".into(), "dfg".into(), "milky".into(), "sanae".into(), "rose".into()],
    },
    Reaction {
      emoji: unicode("👀"),
      count: 3,
      me: false,
      burst: false,
      users: vec!["luke".into(), "dfg".into(), "rose".into()],
    },
    Reaction {
      emoji: scope_emoji(),
      count: 5,
      me: false,
      burst: false,
      users: vec!["zach".into(), "milky".into(), "sanae".into()],
    },
    Reaction {
      emoji: unicode("🚀"),
      count: 1,
      me: false,
      burst: true,
      users: Vec::new(),
    },
  ]
}

/// Builds the showcase history for one channel, assigning sequential ids.
struct Showcase {
  now: DateTime<Utc>,
  first_id: u64,
  messages: Vec<DemoMessage>,
}

impl Showcase {
  fn at(&self, minutes_ago: i64) -> DateTime<Utc> {
    self.now - Duration::minutes(minutes_ago)
  }

  fn push(&mut self, author: Id, minutes_ago: i64, rich: RichMessage) {
    let id = Id(self.first_id + self.messages.len() as u64);
    let content = rich.source.clone();
    self.messages.push(DemoMessage::new(id, author, content, self.at(minutes_ago)).with_rich(rich));
  }

  fn text(&mut self, author: Id, minutes_ago: i64, text: &str) {
    self.push(author, minutes_ago, rich_text(text));
  }

  fn system(&mut self, author: Id, minutes_ago: i64, kind: SystemKind) {
    self.push(
      author,
      minutes_ago,
      RichMessage {
        kind: MessageKind::System(kind),
        ..Default::default()
      },
    );
  }
}

/// `#dev-announcements`: one message per thing the renderer can show, oldest first,
/// spread over the last two hours. Text bodies go through the markdown parser so
/// this doubles as a parser test bed. Ids start at `2_000_000`.
pub fn dev_announcements_history(now: DateTime<Utc>) -> Vec<DemoMessage> {
  let (zach, luke, dfg, milky, sanae, rose) = (Id(2), Id(3), Id(4), Id(5), Id(6), Id(7));
  let mut sc = Showcase {
    now,
    first_id: 2_000_000,
    messages: Vec::new(),
  };

  // -- system notice + markdown blocks --------------------------------------------
  sc.system(milky, 118, SystemKind::MemberJoin);

  sc.push(
    zach,
    115,
    RichMessage {
      pinned: true,
      ..rich_text("# Release notes\nScope **v0.3.0** landed on `main` this morning. Everything below is in the nightly already.")
    },
  );

  sc.text(
    zach,
    114,
    "## Fixes\n1. tab bar no longer jumps on hover\n2. member list groups by hoisted role\n3. group DMs no longer break the ready message\n\n## Still open\n- wayland clipboard\n- voice channels\n- *proper* image zoom",
  );

  sc.text(
    zach,
    113,
    "the new per-message cache, for the curious:\n```rust\nimpl ContentCell {\n  pub fn get_or_create(&self, cx: &mut App, build: impl FnOnce() -> Arc<RichMessage>) -> Entity<RichContentView> {\n    let mut slot = self.0.lock().unwrap();\n    slot.get_or_insert_with(|| cx.new(|_| RichContentView::new(build()))).clone()\n  }\n}\n```",
  );

  sc.text(
    luke,
    110,
    "inline styles: **bold**, *italic*, __underline__, ~~strike~~, ||spoiler||, `inline code`, a [masked link](https://scopeclient.com) and a bare https://github.com/scopeclient/scope",
  );

  sc.text(
    luke,
    109,
    "<@2> can you pin this in <#102>? @everyone heads up, the release call is <t:1700000000:R> (<t:1700000000:F>) <:scope:999> <a:party:998>",
  );

  sc.text(dfg, 108, "🔥🔥🔥");
  sc.text(dfg, 108, "<:scope:999>");

  sc.text(
    sanae,
    105,
    "> It's not a walk in the park\n> To love each other\n> But when our fingers interlock\nthis is what zach pastes every time CI goes red",
  );
  sc.text(sanae, 104, "-# subtext: this message brought to you by the markdown parser");

  sc.text(
    milky,
    100,
    "ok so the rendering pipeline now goes markdown → blocks → gpui elements, and every message caches its RichContentView entity in a ContentCell so scrolling never re-parses anything. the parser is a hand-rolled recursive descent thing with a tiny lookahead because discord's flavour is not quite commonmark: double underscores are underline rather than bold, headings only count at the start of a line, and spoilers can nest inside everything except code. if you find a string that renders wrong, paste it in here and we'll turn it into a test case.",
  );

  // -- attachments -----------------------------------------------------------------
  sc.push(
    rose,
    96,
    RichMessage {
      attachments: vec![image_attachment(1, "landscape.png", 800, 450, 26_010)],
      ..Default::default()
    },
  );

  sc.push(
    rose,
    95,
    RichMessage {
      attachments: vec![image_attachment(2, "square.png", 400, 400, 19_760)],
      ..rich_text("new hero render for the landing page, thoughts?")
    },
  );

  sc.push(
    luke,
    92,
    RichMessage {
      attachments: vec![Attachment {
        filename: "SPOILER_tall.png".into(),
        spoiler: true,
        ..image_attachment(3, "tall.png", 300, 600, 4_920)
      }],
      ..rich_text("don't open if you haven't seen the ending")
    },
  );

  sc.push(
    zach,
    90,
    RichMessage {
      attachments: vec![Attachment {
        width: Some(1280),
        height: Some(720),
        // No real video offline; the poster frame stands in via `proxy_url`.
        proxy_url: Some(asset("poster.png")),
        ..attachment(4, "poster.mp4", &asset("poster.mp4"), Some("video/mp4"), 12 * 1024 * 1024)
      }],
      ..rich_text("screen recording of the tab drag")
    },
  );

  sc.push(
    milky,
    87,
    RichMessage {
      attachments: vec![attachment(5, "set.wav", &asset("set.wav"), Some("audio/wav"), 529_244)],
      ..rich_text("set from friday, two hours of progressive")
    },
  );

  sc.push(
    zach,
    85,
    RichMessage {
      attachments: vec![Attachment {
        voice: Some(VoiceClip {
          duration_secs: 7.0,
          // RMS per bucket of the generated clip (60 buckets over 7 s).
          waveform: vec![
            2, 2, 90, 73, 188, 143, 119, 93, 159, 160, 234, 131, 179, 114, 172, 87, 74, 88, 6, 2, 2, 168, 148, 87, 141, 255, 140, 177, 128, 194, 139,
            199, 67, 98, 84, 222, 156, 164, 224, 106, 2, 2, 112, 208, 119, 169, 108, 187, 161, 91, 84, 158, 167, 144, 143, 247, 72, 2, 2, 2,
          ],
        }),
        ..attachment(6, "voice-message.wav", &asset("voice-message.wav"), Some("audio/wav"), 308_744)
      }],
      ..Default::default()
    },
  );

  sc.push(
    luke,
    82,
    RichMessage {
      attachments: vec![attachment(
        7,
        "scope-design.fig",
        &asset("scope-design.fig"),
        Some("application/octet-stream"),
        4_404_019,
      )],
      ..rich_text("latest design file, comments welcome")
    },
  );

  sc.push(
    rose,
    80,
    RichMessage {
      attachments: vec![
        image_attachment(8, "landscape.png", 800, 450, 26_010),
        image_attachment(9, "square.png", 400, 400, 19_760),
        image_attachment(10, "tall.png", 300, 600, 4_920),
      ],
      ..rich_text("three options for the empty state")
    },
  );

  // -- embeds ----------------------------------------------------------------------
  sc.push(
    zach,
    75,
    RichMessage {
      embeds: vec![Embed {
        kind: EmbedKind::Rich,
        title: Some("Scope v0.3.0".into()),
        url: Some("https://github.com/scopeclient/scope/releases/tag/v0.3.0".into()),
        description: Some(blocks(
          "The **rich content** release: attachments, embeds, polls and stickers all render natively.\nFull notes in `CHANGELOG.md`.",
        )),
        color: Some(0xfc3b8c),
        author: Some(EmbedAuthor {
          name: "zach".into(),
          url: Some("https://github.com/zach".into()),
          icon_url: Some(asset("avatar-a.png")),
        }),
        provider: None,
        thumbnail: Some(media("thumb.png", 160, 160)),
        image: Some(media("landscape.png", 800, 450)),
        video: None,
        fields: vec![
          EmbedField {
            name: "Commits".into(),
            value: blocks("**142**"),
            inline: true,
          },
          EmbedField {
            name: "Contributors".into(),
            value: blocks("6"),
            inline: true,
          },
          EmbedField {
            name: "Closed issues".into(),
            value: blocks("~~38~~ 37"),
            inline: true,
          },
          EmbedField {
            name: "Breaking".into(),
            value: blocks("`Message::get_content` now returns an `Entity<RichContentView>`; backends build a `RichMessage` instead of a string."),
            inline: false,
          },
        ],
        footer: Some(EmbedFooter {
          text: "release bot".into(),
          icon_url: Some(asset("thumb.png")),
        }),
        timestamp: Some(now - Duration::minutes(76)),
      }],
      ..rich_text("release bot output:")
    },
  );

  sc.push(
    luke,
    72,
    RichMessage {
      embeds: vec![Embed {
        kind: EmbedKind::Link,
        title: Some("Refactor (#44) · scopeclient/scope".into()),
        url: Some("https://github.com/scopeclient/scope/pull/44".into()),
        description: Some(blocks(
          "Splits the UI into `shell`, `channel` and `rich` crates and moves the demo backend behind a trait.",
        )),
        color: Some(0x1f6feb),
        provider: Some(EmbedProvider {
          name: Some("GitHub".into()),
          url: Some("https://github.com".into()),
        }),
        thumbnail: Some(media("thumb.png", 160, 160)),
        ..Default::default()
      }],
      ..rich_text("https://github.com/scopeclient/scope/pull/44")
    },
  );

  sc.push(
    dfg,
    70,
    RichMessage {
      embeds: vec![Embed {
        kind: EmbedKind::Image,
        url: Some("https://scopeclient.com/media/hero.png".into()),
        image: Some(media("square.png", 400, 400)),
        ..Default::default()
      }],
      ..rich_text("https://scopeclient.com/media/hero.png")
    },
  );

  sc.push(
    sanae,
    68,
    RichMessage {
      embeds: vec![Embed {
        kind: EmbedKind::Video,
        title: Some("Scope — tab drag demo".into()),
        url: Some("https://www.youtube.com/watch?v=scope-tab-drag".into()),
        color: Some(0xff0000),
        provider: Some(EmbedProvider {
          name: Some("YouTube".into()),
          url: Some("https://www.youtube.com".into()),
        }),
        thumbnail: Some(media("poster.png", 640, 360)),
        video: Some(EmbedMedia {
          url: "https://www.youtube.com/embed/scope-tab-drag".into(),
          proxy_url: None,
          width: Some(1280),
          height: Some(720),
        }),
        ..Default::default()
      }],
      ..rich_text("https://www.youtube.com/watch?v=scope-tab-drag")
    },
  );

  // -- stickers --------------------------------------------------------------------
  sc.push(
    milky,
    64,
    RichMessage {
      stickers: vec![Sticker {
        id: 7001,
        name: "blob".into(),
        format: StickerFormat::Png,
        url: Some(asset("sticker.png")),
      }],
      ..Default::default()
    },
  );

  sc.push(
    dfg,
    63,
    RichMessage {
      stickers: vec![Sticker {
        id: 7002,
        name: "wave".into(),
        format: StickerFormat::Lottie,
        url: None,
      }],
      ..Default::default()
    },
  );

  // -- polls -----------------------------------------------------------------------
  sc.push(
    zach,
    60,
    RichMessage {
      poll: Some(Poll {
        question: "Should v1 ship with voice?".into(),
        answers: vec![
          PollAnswer {
            id: 1,
            text: "yes, it's table stakes".into(),
            emoji: Some(unicode("🎙️")),
            votes: 12,
            me_voted: true,
          },
          PollAnswer {
            id: 2,
            text: "text first, voice in v1.1".into(),
            emoji: Some(unicode("⌨️")),
            votes: 7,
            me_voted: false,
          },
          PollAnswer {
            id: 3,
            text: "no opinion".into(),
            emoji: Some(unicode("🤷")),
            votes: 3,
            me_voted: false,
          },
        ],
        total_votes: 22,
        expires_at: Some(now + Duration::days(2)),
        allow_multiselect: false,
        finalized: false,
      }),
      ..Default::default()
    },
  );

  sc.push(
    luke,
    55,
    RichMessage {
      poll: Some(Poll {
        question: "Dark theme by default?".into(),
        answers: vec![
          PollAnswer {
            id: 1,
            text: "yes".into(),
            emoji: None,
            votes: 18,
            me_voted: true,
          },
          PollAnswer {
            id: 2,
            text: "no".into(),
            emoji: None,
            votes: 4,
            me_voted: false,
          },
          PollAnswer {
            id: 3,
            text: "ask on first launch".into(),
            emoji: None,
            votes: 9,
            me_voted: false,
          },
        ],
        total_votes: 31,
        expires_at: Some(now - Duration::days(1)),
        allow_multiselect: true,
        finalized: true,
      }),
      ..Default::default()
    },
  );

  // -- reactions, replies -------------------------------------------------------------
  sc.push(
    rose,
    50,
    RichMessage {
      reactions: hot_reactions(12, true),
      pinned: true,
      ..rich_text("the launch trailer is done. we're shipping.")
    },
  );

  sc.push(
    SELF_ID,
    45,
    RichMessage {
      kind: MessageKind::Reply,
      reply: Some(ReplyRef {
        message_id: Some(2_000_003),
        author_name: "zach".into(),
        author_avatar: Some(asset("avatar-a.png")),
        snippet: "the new per-message cache, for the curious:".into(),
        deleted: false,
      }),
      ..rich_text("this is so much cleaner than the string path")
    },
  );

  sc.push(
    dfg,
    44,
    RichMessage {
      kind: MessageKind::Reply,
      reply: Some(ReplyRef {
        message_id: None,
        author_name: String::new(),
        author_avatar: None,
        snippet: String::new(),
        deleted: true,
      }),
      ..rich_text("wait what did they say")
    },
  );

  // -- components --------------------------------------------------------------------
  sc.push(
    zach,
    40,
    RichMessage {
      components: vec![
        ComponentRow {
          components: vec![
            button("Approve", ButtonStyle::Primary, false),
            button("Later", ButtonStyle::Secondary, false),
            button("Ship it", ButtonStyle::Success, false),
            button("Rollback", ButtonStyle::Danger, true),
            Component::Button {
              label: Some("Changelog".into()),
              style: ButtonStyle::Link,
              url: Some("https://github.com/scopeclient/scope/releases".into()),
              emoji: Some(unicode("📝")),
              disabled: false,
            },
          ],
        },
        ComponentRow {
          components: vec![Component::Select {
            placeholder: Some("Pick a release channel…".into()),
            disabled: false,
          }],
        },
      ],
      ..rich_text("release checklist — hit the buttons when you're done")
    },
  );

  // -- more system notices --------------------------------------------------------------
  sc.system(luke, 36, SystemKind::PinsAdd);
  sc.system(rose, 30, SystemKind::Boost { tier: Some(2) });
  sc.system(
    sanae,
    25,
    SystemKind::ThreadCreated {
      name: "design feedback".into(),
    },
  );

  // -- edited + wrap-up -------------------------------------------------------------------
  sc.push(
    luke,
    20,
    RichMessage {
      edited_at: Some(now - Duration::minutes(5)),
      ..rich_text("release call moved to **6pm**, not 5pm")
    },
  );

  sc.text(
    zach,
    6,
    "that's every block the renderer knows about. if anything above looks off, screenshot it and drop it in <#102>",
  );
  sc.text(luke, 5, "ship it 🚀");

  sc.messages
}
