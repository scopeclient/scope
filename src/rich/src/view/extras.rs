//! Everything around the text body: reply header, attachments (images, video,
//! audio, voice messages, files), embeds, stickers, polls, reactions,
//! components and system notices.
//!
//! Everything here is read-only for now: media and links open in the browser,
//! spoiler covers reveal on click, buttons/selects/reactions are inert.

use std::{collections::HashSet, sync::Arc, time::Duration};

use chrono::{DateTime, Local, Utc};
use gpui::{
  AnyElement, App, ClickEvent, Context, Div, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement, ObjectFit, ParentElement, Pixels,
  StatefulInteractiveElement, Styled, StyledImage, Svg, Window, div, img, prelude::FluentBuilder, px, relative, svg, white,
};
use gpui_component::tooltip::Tooltip;
use scope_media::{
  MediaPlayer, MediaSource, PlaybackStatus, Track,
  element::{format_progress, progress_bar, seekable},
};
use scope_theme as tokens;

use crate::{
  model::{
    Attachment, AttachmentKind, ButtonStyle, Component, ComponentRow, Embed, EmbedAuthor, EmbedField, EmbedKind, EmbedMedia, Emoji, Poll, PollAnswer,
    Reaction, ReplyRef, RichMessage, Sticker, StickerFormat, SystemKind, VoiceClip,
  },
  view::{RichContentView, text::render_blocks},
};

/// Largest box an inline image / video preview may take.
const MEDIA_MAX_W: f32 = 400.;
const MEDIA_MAX_H: f32 = 300.;
/// Width of file cards, embeds and polls.
const CARD_W: f32 = 432.;
/// Bars drawn for a voice message waveform.
const VOICE_BARS: usize = 48;

/// Message body colour from the design (`#c4c8d4`), between `TEXT` and
/// `TEXT_SECONDARY`; no token matches. Mirrors `ui::channel::message`.
const BODY_TEXT: Hsla = tokens::hex(0xc4c8d4);

// ---- public entry points ---------------------------------------------------------------

/// One element per extra section, in Discord's order:
/// attachments → embeds → stickers → poll → components → reactions.
pub fn render_extras(
  rich: &RichMessage,
  revealed_attachments: &HashSet<u64>,
  on_reaction: Option<crate::view::ReactionHandler>,
  window: &mut Window,
  cx: &mut Context<RichContentView>,
) -> Vec<AnyElement> {
  let mut out = Vec::new();

  for attachment in &rich.attachments {
    out.push(render_attachment(attachment, revealed_attachments.contains(&attachment.id), cx));
  }

  for (index, embed) in rich.embeds.iter().enumerate() {
    out.push(render_embed(embed, index, window, cx));
  }

  for sticker in &rich.stickers {
    out.push(render_sticker(sticker));
  }

  if let Some(poll) = &rich.poll {
    out.push(render_poll(poll));
  }

  for (index, row) in rich.components.iter().enumerate() {
    out.push(render_component_row(row, index));
  }

  if !rich.reactions.is_empty() {
    out.push(render_reactions(&rich.reactions, on_reaction));
  }

  out
}

/// The "replying to" line above the body: a curved connector, the referenced
/// author's avatar and name, and the first line of their message.
pub fn render_reply(reply: &ReplyRef, _window: &mut Window, _cx: &mut Context<RichContentView>) -> AnyElement {
  let connector = div()
    .flex_shrink_0()
    .h_full()
    .flex()
    .items_end()
    .child(div().w(px(20.)).h(px(8.)).border_t_2().border_l_2().border_color(tokens::BORDER_TERTIARY).rounded_tl(px(6.)));

  let row = div().h(px(16.)).min_w_0().flex().flex_row().items_center().gap(px(4.)).text_size(tokens::TYPE_S).line_height(px(16.)).child(connector);

  if reply.deleted {
    return row.child(div().min_w_0().truncate().italic().text_color(tokens::TEXT_TERTIARY).child("Original message was deleted")).into_any_element();
  }

  let avatar: AnyElement = match &reply.author_avatar {
    Some(url) => img(url.clone()).size(px(16.)).rounded_full().object_fit(ObjectFit::Cover).into_any_element(),
    None => {
      let initial = reply.author_name.chars().next().unwrap_or('?').to_uppercase().to_string();
      div()
        .size(px(16.))
        .rounded_full()
        .bg(tokens::BG_FILL)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(9.))
        .line_height(px(9.))
        .font_weight(FontWeight::BOLD)
        .text_color(white())
        .child(initial)
        .into_any_element()
    }
  };

  row
    .child(div().flex_shrink_0().size(px(16.)).rounded_full().overflow_hidden().child(avatar))
    .child(div().flex_shrink_0().font_weight(FontWeight::MEDIUM).text_color(tokens::TEXT_SECONDARY).child(reply.author_name.clone()))
    .child(div().min_w_0().truncate().text_color(tokens::TEXT_TERTIARY).child(reply.snippet.clone()))
    .into_any_element()
}

/// A single muted line for server-generated messages. The author's name is
/// rendered by the row header, so the text starts with the verb.
pub fn render_system_notice(kind: &SystemKind, _rich: &RichMessage, _window: &mut Window, _cx: &mut Context<RichContentView>) -> AnyElement {
  let (path, color, text): (&'static str, Hsla, String) = match kind {
    SystemKind::MemberJoin => ("icons/arrow-right.svg", tokens::ICON_SUCCESS, "joined the server.".into()),
    SystemKind::PinsAdd => ("icons/scope/pin.svg", tokens::ICON, "pinned a message to this channel.".into()),
    SystemKind::Boost { tier } => {
      let text = match tier {
        Some(tier) => format!("just boosted the server! Server has achieved Level {tier}!"),
        None => "just boosted the server!".into(),
      };
      ("icons/scope/rich-sparkles.svg", tokens::ICON_BRAND, text)
    }
    SystemKind::ThreadCreated { name } => ("icons/scope/rich-message-square.svg", tokens::ICON, format!("started a thread: {name}")),
    SystemKind::ChannelFollowAdd => ("icons/arrow-right.svg", tokens::ICON, "added a news channel.".into()),
    SystemKind::GroupRecipientAdd => ("icons/arrow-right.svg", tokens::ICON_SUCCESS, "added a recipient to the group.".into()),
    SystemKind::GroupRecipientRemove => ("icons/arrow-left.svg", tokens::ICON_DANGER, "removed a recipient from the group.".into()),
    SystemKind::GroupNameUpdate => ("icons/scope/rich-pencil.svg", tokens::ICON, "changed the group name.".into()),
    SystemKind::GroupIconUpdate => ("icons/scope/rich-pencil.svg", tokens::ICON, "changed the group icon.".into()),
    SystemKind::Call => ("icons/scope/rich-phone.svg", tokens::ICON, "started a call.".into()),
    SystemKind::Other(text) => ("icons/info.svg", tokens::ICON, text.clone()),
  };

  div()
    .min_w_0()
    .flex()
    .flex_row()
    .items_center()
    .gap(px(8.))
    .child(icon(path, 16., color))
    .child(div().min_w_0().text_size(tokens::TYPE_M).font_weight(FontWeight::MEDIUM).text_color(tokens::TEXT_TERTIARY).child(text))
    .into_any_element()
}

// ---- shared helpers ------------------------------------------------------------------------

/// Click handler that opens `url` in the system browser.
fn open_url(url: &str) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
  let url = url.to_owned();
  move |_, _, cx| cx.open_url(&url)
}

/// Makes `element` clickable when there is a url; otherwise returns it as is.
fn linkify(element: Div, id: impl Into<ElementId>, url: Option<&str>) -> AnyElement {
  match url {
    Some(url) => element.id(id).cursor_pointer().on_click(open_url(url)).into_any_element(),
    None => element.into_any_element(),
  }
}

/// Tinted SVG icon at `size`.
fn icon(path: &'static str, size: f32, color: Hsla) -> Svg {
  svg().path(path).flex_shrink_0().size(px(size)).text_color(color)
}

/// Shrinks `width x height` to fit in `max_w x max_h` keeping the aspect
/// ratio, never upscaling. `None` when the dimensions are unknown.
fn fit(width: Option<u32>, height: Option<u32>, max_w: f32, max_h: f32) -> Option<(f32, f32)> {
  match (width, height) {
    (Some(w), Some(h)) if w > 0 && h > 0 => {
      let (w, h) = (w as f32, h as f32);
      let scale = (max_w / w).min(max_h / h).min(1.);
      Some(((w * scale).round().max(1.), (h * scale).round().max(1.)))
    }
    _ => None,
  }
}

/// `img` sized to `dims`, or to its natural size capped at the media maximum.
fn sized_img(src: String, dims: Option<(f32, f32)>) -> gpui::Img {
  match dims {
    Some((w, h)) => img(src).w(px(w)).h(px(h)).object_fit(ObjectFit::Cover),
    None => img(src).max_w(px(MEDIA_MAX_W)).max_h(px(MEDIA_MAX_H)).object_fit(ObjectFit::Contain),
  }
}

/// Circle with a white play triangle, used for video/audio/voice.
fn play_circle(size: f32, bg: Hsla) -> Div {
  div()
    .flex_shrink_0()
    .size(px(size))
    .rounded_full()
    .bg(bg)
    .flex()
    .items_center()
    .justify_center()
    .child(icon("icons/scope/rich-play.svg", size * 0.42, white()).ml(px(size * 0.06)))
}

/// Unicode emoji as text, custom emoji as an image, both `size` px tall.
fn emoji_element(emoji: &Emoji, size: f32) -> AnyElement {
  crate::emoji::render_emoji(emoji, size)
}

/// Two-line label used by audio/file cards: name on top, size below.
///
/// The name sits in its own row as a `flex_1` item: gpui caches the first
/// measurement of non-wrapping text, and as a direct column item that first
/// pass can run at zero width and leave nothing but the ellipsis behind.
fn name_and_size(name: &str, name_color: Hsla, size: &str) -> Div {
  div()
    .flex_1()
    .min_w_0()
    .flex()
    .flex_col()
    .child(
      div()
        .flex()
        .flex_row()
        .child(div().flex_1().min_w_0().truncate().text_size(tokens::TYPE_M).line_height(px(18.)).text_color(name_color).child(name.to_owned())),
    )
    .child(div().text_size(tokens::TYPE_S).line_height(px(16.)).text_color(tokens::TEXT_TERTIARY).child(size.to_owned()))
}

/// "Today at 1:52 PM", "Yesterday at 9:03 AM", otherwise "24/08/2026" (local time).
fn format_timestamp(timestamp: DateTime<Utc>) -> String {
  let local = timestamp.with_timezone(&Local);
  let today = Local::now().date_naive();
  let date = local.date_naive();

  if date == today {
    format!("Today at {}", local.format("%-I:%M %p"))
  } else if date.succ_opt() == Some(today) {
    format!("Yesterday at {}", local.format("%-I:%M %p"))
  } else {
    local.format("%d/%m/%Y").to_string()
  }
}

/// "3 hours left", "2 days left", … until `expires`.
fn time_left(expires: DateTime<Utc>) -> String {
  let plural = |n: i64, unit: &str| if n == 1 { format!("1 {unit} left") } else { format!("{n} {unit}s left") };
  let secs = (expires - Utc::now()).num_seconds().max(0);

  match secs {
    s if s < 60 => "Less than a minute left".into(),
    s if s < 3600 => plural(s / 60, "minute"),
    s if s < 86_400 => plural(s / 3600, "hour"),
    s => plural(s / 86_400, "day"),
  }
}

/// `m:ss`.
fn format_duration(secs: f32) -> String {
  let secs = secs.round().max(0.) as u32;
  format!("{}:{:02}", secs / 60, secs % 60)
}

// ---- attachments --------------------------------------------------------------------------

fn render_attachment(attachment: &Attachment, revealed: bool, cx: &mut Context<RichContentView>) -> AnyElement {
  let kind = attachment.kind();

  if attachment.spoiler && !revealed && matches!(kind, AttachmentKind::Image | AttachmentKind::Video) {
    return spoiler_cover(attachment, cx);
  }

  match (&attachment.voice, kind) {
    (Some(clip), _) => render_voice(attachment, clip, cx),
    (None, AttachmentKind::Image) => render_image(attachment),
    (None, AttachmentKind::Video) => render_video(attachment),
    (None, AttachmentKind::Audio) => render_audio(attachment, cx),
    (None, AttachmentKind::Voice | AttachmentKind::File) => render_file(attachment),
  }
}

/// Box the size of the hidden media with a "SPOILER" pill; click reveals.
fn spoiler_cover(attachment: &Attachment, cx: &mut Context<RichContentView>) -> AnyElement {
  let (w, h) = fit(attachment.width, attachment.height, MEDIA_MAX_W, MEDIA_MAX_H).unwrap_or((MEDIA_MAX_W, 225.));
  let id = attachment.id;

  div()
    .id(("attachment", id))
    .w(px(w))
    .h(px(h))
    .max_w_full()
    .rounded(tokens::RADIUS_200)
    .bg(tokens::BG_FILL)
    .flex()
    .items_center()
    .justify_center()
    .cursor_pointer()
    .child(
      div()
        .px(px(12.))
        .py(px(6.))
        .rounded_full()
        .bg(tokens::BG_SURFACE)
        .text_size(tokens::TYPE_S)
        .line_height(px(16.))
        .font_weight(FontWeight::BOLD)
        .text_color(white())
        .child("SPOILER"),
    )
    .on_click(cx.listener(move |this, _, _, cx| this.reveal_attachment(id, cx)))
    .into_any_element()
}

fn render_image(attachment: &Attachment) -> AnyElement {
  let src = attachment.proxy_url.clone().unwrap_or_else(|| attachment.url.clone());
  let dims = fit(attachment.width, attachment.height, MEDIA_MAX_W, MEDIA_MAX_H);

  div()
    .id(("attachment", attachment.id))
    .max_w_full()
    .rounded(tokens::RADIUS_200)
    .overflow_hidden()
    .cursor_pointer()
    .child(sized_img(src, dims))
    .on_click(open_url(&attachment.url))
    .into_any_element()
}

/// Dark box with a play button; the proxy url, when the backend provides one,
/// is shown underneath as the poster frame.
fn render_video(attachment: &Attachment) -> AnyElement {
  let (w, h) = fit(attachment.width, attachment.height, MEDIA_MAX_W, MEDIA_MAX_H).unwrap_or((MEDIA_MAX_W, 225.));

  div()
    .id(("attachment", attachment.id))
    .relative()
    .w(px(w))
    .h(px(h))
    .max_w_full()
    .rounded(tokens::RADIUS_200)
    .overflow_hidden()
    .bg(tokens::BG)
    .flex()
    .items_center()
    .justify_center()
    .cursor_pointer()
    .children(attachment.proxy_url.clone().map(|poster| img(poster).absolute().inset_0().size_full().object_fit(ObjectFit::Cover)))
    .child(play_circle(48., tokens::BG_SURFACE))
    .child(
      div()
        .absolute()
        .top(px(8.))
        .left(px(8.))
        .right(px(8.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.))
        .text_size(tokens::TYPE_S)
        .line_height(px(16.))
        .child(div().min_w_0().truncate().text_color(tokens::TEXT).child(attachment.filename.clone()))
        .child(div().flex_shrink_0().text_color(tokens::TEXT_TERTIARY).child(attachment.size_label())),
    )
    .on_click(open_url(&attachment.url))
    .into_any_element()
}

// ---- audio playback (global player) --------------------------------------------------------

/// What the audio renderers need to know about the global player, collapsed
/// to "not ours" when another attachment's track is loaded.
struct MediaSnapshot {
  current: bool,
  status: PlaybackStatus,
  fraction: f32,
  position: Duration,
  duration: Option<Duration>,
}

fn media_snapshot(url: &str, cx: &App) -> MediaSnapshot {
  let state = MediaPlayer::state(cx);
  let state = state.read(cx);

  if state.is_current(url) {
    MediaSnapshot {
      current: true,
      status: state.status.clone(),
      fraction: state.fraction().unwrap_or(0.),
      position: state.position,
      duration: state.duration,
    }
  } else {
    MediaSnapshot {
      current: false,
      status: PlaybackStatus::Stopped,
      fraction: 0.,
      position: Duration::ZERO,
      duration: None,
    }
  }
}

impl MediaSnapshot {
  /// The play button should show a pause glyph.
  fn active(&self) -> bool {
    self.current && matches!(self.status, PlaybackStatus::Playing | PlaybackStatus::Loading)
  }

  fn errored(&self) -> Option<&str> {
    match (&self.status, self.current) {
      (PlaybackStatus::Error(message), true) => Some(message),
      _ => None,
    }
  }

  /// Clicking the progress surface may seek.
  fn seekable(&self) -> bool {
    self.current && self.duration.is_some() && matches!(self.status, PlaybackStatus::Playing | PlaybackStatus::Paused)
  }
}

/// Click handler for audio cards / voice pills: toggles play/pause when the
/// clicked attachment is the loaded track, otherwise loads and plays it.
/// Errored tracks and unloadable sources fall back to opening the url.
fn media_click(attachment: &Attachment) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
  let url = attachment.url.clone();
  let title = attachment.filename.clone();
  let subtitle = if attachment.voice.is_some() {
    "Voice message".to_owned()
  } else {
    attachment.size_label()
  };
  let duration_hint = attachment.voice.as_ref().map(|clip| clip.duration_secs);

  move |_, _, cx| {
    {
      let state = MediaPlayer::state(cx);
      let state = state.read(cx);
      if state.is_current(&url) {
        if matches!(state.status, PlaybackStatus::Error(_)) {
          cx.open_url(&url);
        } else {
          MediaPlayer::toggle(cx);
        }
        return;
      }
    }

    let source = if url.starts_with("http://") || url.starts_with("https://") {
      Some(MediaSource::Url(url.clone()))
    } else {
      // Bundled demo assets and the like: read through the asset source.
      match cx.asset_source().load(&url) {
        Ok(Some(bytes)) => Some(MediaSource::Bytes(Arc::new(bytes.into_owned()))),
        _ => None,
      }
    };

    match source {
      Some(source) => MediaPlayer::play(
        Track {
          id: url.clone(),
          title: title.clone(),
          subtitle: Some(subtitle.clone()),
          source,
          duration_hint,
        },
        cx,
      ),
      None => cx.open_url(&url),
    }
  }
}

/// 32px circular play/pause button. Pause is drawn with two bars so no pause
/// asset is needed.
fn play_pause_button(pause: bool, bg: Hsla, hover_bg: Hsla) -> Div {
  let glyph: AnyElement = if pause {
    div().flex().flex_row().gap(px(3.)).children([(), ()].map(|_| div().w(px(3.5)).h(px(12.)).rounded(px(1.5)).bg(white()))).into_any_element()
  } else {
    icon("icons/scope/rich-play.svg", 13.5, white()).ml(px(2.)).into_any_element()
  };

  div().flex_shrink_0().size(px(32.)).rounded_full().bg(bg).hover(move |style| style.bg(hover_bg)).flex().items_center().justify_center().child(glyph)
}

/// Card driving the global player. While its track is loaded the size label
/// becomes a seekable progress bar with elapsed/total time.
fn render_audio(attachment: &Attachment, cx: &mut Context<RichContentView>) -> AnyElement {
  let snap = media_snapshot(&attachment.url, cx);
  let (button_bg, button_hover) = if snap.current {
    (tokens::BRAND, tokens::BRAND_HOVER)
  } else {
    (tokens::BG_FILL, tokens::BRAND)
  };

  let detail: AnyElement = if let Some(message) = snap.errored() {
    let open = {
      let open = open_url(&attachment.url);
      move |event: &ClickEvent, window: &mut Window, cx: &mut App| {
        cx.stop_propagation();
        open(event, window, cx)
      }
    };
    div()
      .flex()
      .flex_row()
      .items_center()
      .gap(px(8.))
      .text_size(tokens::TYPE_S)
      .line_height(px(16.))
      .child(div().min_w_0().truncate().text_color(tokens::TEXT_DANGER).child(message.to_owned()))
      .child(
        div()
          .id(("audio-open", attachment.id))
          .flex_shrink_0()
          .text_color(tokens::TEXT_LINK)
          .cursor_pointer()
          .hover(|style| style.text_color(tokens::TEXT_LINK_HOVER))
          .child("Open")
          .on_click(open),
      )
      .into_any_element()
  } else if snap.current {
    div()
      .flex()
      .flex_row()
      .items_center()
      .gap(px(8.))
      .child(progress_bar(snap.fraction, 16., tokens::BG_FILL, tokens::BRAND, snap.seekable()))
      .child(
        div()
          .flex_shrink_0()
          .text_size(tokens::TYPE_S)
          .line_height(px(16.))
          .text_color(tokens::TEXT_SECONDARY)
          .child(format_progress(snap.position, snap.duration)),
      )
      .into_any_element()
  } else {
    div().text_size(tokens::TYPE_S).line_height(px(16.)).text_color(tokens::TEXT_TERTIARY).child(attachment.size_label()).into_any_element()
  };

  let label = div().flex_1().min_w_0().flex().flex_col().child(
    // Name in its own flex row; see `name_and_size` for why.
    div().flex().flex_row().child(
      div().flex_1().min_w_0().truncate().text_size(tokens::TYPE_M).line_height(px(18.)).text_color(tokens::TEXT).child(attachment.filename.clone()),
    ),
  );

  div()
    .id(("attachment", attachment.id))
    .w(px(400.))
    .max_w_full()
    .h(px(56.))
    .rounded(tokens::RADIUS_200)
    .bg(tokens::BG_SURFACE_SECONDARY)
    .border_1()
    .border_color(tokens::BORDER)
    .px(px(12.))
    .flex()
    .flex_row()
    .items_center()
    .gap(px(12.))
    .cursor_pointer()
    .child(play_pause_button(snap.active(), button_bg, button_hover))
    .child(label.child(detail))
    .on_click(media_click(attachment))
    .into_any_element()
}

/// Pill with a play button, a bar waveform and the clip duration; drives the
/// global player, painting playback progress into the waveform. While playing
/// the time counts down; clicking the waveform seeks.
fn render_voice(attachment: &Attachment, clip: &VoiceClip, cx: &mut Context<RichContentView>) -> AnyElement {
  let bars = downsample(&clip.waveform, VOICE_BARS);
  let snap = media_snapshot(&attachment.url, cx);
  let errored = snap.errored().is_some();
  let played = (snap.fraction * VOICE_BARS as f32).round() as usize;

  let time_label = if errored {
    "Open".to_owned()
  } else if snap.current {
    let total = snap.duration.map(|d| d.as_secs_f32()).unwrap_or(clip.duration_secs);
    format_duration((total - snap.position.as_secs_f32()).max(0.))
  } else {
    format_duration(clip.duration_secs)
  };

  let current = snap.current;
  let waveform = div().flex_1().min_w_0().h_full().flex().flex_row().items_center().gap(px(1.)).overflow_hidden().children(
    bars.into_iter().enumerate().map(|(i, level)| {
      let color = if current && i < played { tokens::BRAND } else { tokens::TEXT_SECONDARY };
      div().flex_shrink_0().w(px(2.)).h(px(2. + 18. * level)).rounded_full().bg(color)
    }),
  );

  div()
    .id(("attachment", attachment.id))
    .w(px(280.))
    .max_w_full()
    .h(px(48.))
    .rounded(tokens::RADIUS_400)
    .bg(tokens::BG_SURFACE_SECONDARY)
    .border_1()
    .border_color(tokens::BORDER)
    .pl(px(8.))
    .pr(px(12.))
    .flex()
    .flex_row()
    .items_center()
    .gap(px(8.))
    .cursor_pointer()
    .child(play_pause_button(snap.active(), tokens::BRAND, tokens::BRAND_HOVER))
    .child(seekable(waveform, snap.seekable()))
    .child(
      div()
        .flex_shrink_0()
        .text_size(tokens::TYPE_S)
        .line_height(px(16.))
        .text_color(if errored { tokens::TEXT_LINK } else { tokens::TEXT_SECONDARY })
        .child(time_label),
    )
    .on_click(media_click(attachment))
    .into_any_element()
}

/// Averages `samples` (0–255) into `bars` buckets in `0.0..=1.0`.
fn downsample(samples: &[u8], bars: usize) -> Vec<f32> {
  if samples.is_empty() {
    return vec![0.; bars];
  }

  (0..bars)
    .map(|i| {
      let start = i * samples.len() / bars;
      let end = ((i + 1) * samples.len() / bars).clamp(start + 1, samples.len());
      let sum: u32 = samples[start..end].iter().map(|&s| u32::from(s)).sum();
      sum as f32 / (end - start) as f32 / 255.
    })
    .collect()
}

fn render_file(attachment: &Attachment) -> AnyElement {
  div()
    .id(("attachment", attachment.id))
    .w(px(CARD_W))
    .max_w_full()
    .h(px(56.))
    .rounded(tokens::RADIUS_200)
    .bg(tokens::BG_SURFACE_SECONDARY)
    .border_1()
    .border_color(tokens::BORDER)
    .px(px(12.))
    .flex()
    .flex_row()
    .items_center()
    .gap(px(12.))
    .cursor_pointer()
    .child(icon("icons/file.svg", 32., tokens::ICON))
    .child(name_and_size(&attachment.filename, tokens::TEXT_LINK, &attachment.size_label()))
    .child(icon("icons/scope/rich-download.svg", 20., tokens::ICON))
    .on_click(open_url(&attachment.url))
    .into_any_element()
}

// ---- embeds -------------------------------------------------------------------------------

fn render_embed(embed: &Embed, index: usize, window: &mut Window, cx: &mut Context<RichContentView>) -> AnyElement {
  // Bare image / gif embeds (link previews of an image) are just the picture.
  if matches!(embed.kind, EmbedKind::Image | EmbedKind::Gifv)
    && embed.title.is_none()
    && embed.description.is_none()
    && let Some(media) = embed.image.as_ref().or(embed.thumbnail.as_ref())
  {
    return embed_media(
      media,
      ("embed-media", index),
      embed.url.as_deref().or(Some(&media.url)),
      tokens::RADIUS_200,
    );
  }

  let is_video = embed.kind == EmbedKind::Video;
  let accent = embed.color.map(tokens::hex).unwrap_or(tokens::BORDER_TERTIARY);
  let thumbnail = if is_video { None } else { embed.thumbnail.as_ref() };

  let mut text = div().flex_1().min_w_0().flex().flex_col().gap(px(8.));

  if let Some(provider) = embed.provider.as_ref().and_then(|p| p.name.clone()) {
    text = text.child(div().text_size(tokens::TYPE_S).line_height(px(16.)).text_color(tokens::TEXT_TERTIARY).child(provider));
  }

  if let Some(author) = &embed.author {
    text = text.child(embed_author(author, index));
  }

  if let Some(title) = &embed.title {
    let color = if embed.url.is_some() { tokens::TEXT_LINK } else { tokens::TEXT };
    let title = div().text_size(tokens::TYPE_L).line_height(px(22.)).font_weight(FontWeight::MEDIUM).text_color(color).child(title.clone());
    text = text.child(linkify(title, ("embed-title", index), embed.url.as_deref()));
  }

  if let Some(description) = &embed.description {
    text = text.child(
      div().text_size(tokens::TYPE_M).line_height(px(18.)).font_weight(FontWeight::NORMAL).text_color(BODY_TEXT).child(render_blocks(
        description,
        &HashSet::new(),
        false,
        window,
        cx,
      )),
    );
  }

  if !embed.fields.is_empty() {
    text = text.child(embed_fields(&embed.fields, window, cx));
  }

  if is_video {
    text = text.child(embed_video(embed, index));
  } else if let Some(image) = &embed.image {
    text = text.child(embed_media(image, ("embed-image", index), Some(&image.url), tokens::RADIUS_100));
  }

  let footer_text = match (&embed.footer, embed.timestamp) {
    (Some(footer), Some(timestamp)) => Some(format!("{} • {}", footer.text, format_timestamp(timestamp))),
    (Some(footer), None) => Some(footer.text.clone()),
    (None, Some(timestamp)) => Some(format_timestamp(timestamp)),
    (None, None) => None,
  };

  if let Some(footer_text) = footer_text {
    let footer_icon = embed.footer.as_ref().and_then(|f| f.icon_url.clone());
    text = text.child(
      div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .children(footer_icon.map(|url| img(url).flex_shrink_0().size(px(16.)).rounded_full().object_fit(ObjectFit::Cover)))
        .child(div().min_w_0().text_size(tokens::TYPE_S).line_height(px(16.)).text_color(tokens::TEXT_TERTIARY).child(footer_text)),
    );
  }

  div()
    .max_w(px(CARD_W))
    .rounded(tokens::RADIUS_100)
    .bg(tokens::BG_SURFACE_SECONDARY)
    .border_l_4()
    .border_color(accent)
    .pl(px(12.))
    .pr(px(16.))
    .py(px(16.))
    .flex()
    .flex_row()
    .items_start()
    .gap(px(16.))
    .child(text)
    .children(thumbnail.map(|media| {
      let src = media.proxy_url.clone().unwrap_or_else(|| media.url.clone());
      let frame =
        div().flex_shrink_0().size(px(80.)).rounded(tokens::RADIUS_100).overflow_hidden().child(img(src).size_full().object_fit(ObjectFit::Cover));
      linkify(frame, ("embed-thumb", index), Some(&media.url))
    }))
    .into_any_element()
}

fn embed_author(author: &EmbedAuthor, index: usize) -> AnyElement {
  let row = div()
    .flex()
    .flex_row()
    .items_center()
    .gap(px(8.))
    .children(author.icon_url.clone().map(|url| img(url).flex_shrink_0().size(px(16.)).rounded_full().object_fit(ObjectFit::Cover)))
    .child(
      div()
        .min_w_0()
        .text_size(tokens::TYPE_M)
        .line_height(px(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(tokens::TEXT)
        .child(author.name.clone()),
    );

  linkify(row, ("embed-author", index), author.url.as_deref())
}

/// Inline fields share a row (up to three); the rest take the full width.
fn embed_fields(fields: &[EmbedField], window: &mut Window, cx: &mut Context<RichContentView>) -> Div {
  let mut rows: Vec<Vec<&EmbedField>> = Vec::new();
  for field in fields {
    match rows.last_mut() {
      Some(row) if field.inline && row.len() < 3 && row.iter().all(|f| f.inline) => row.push(field),
      _ => rows.push(vec![field]),
    }
  }

  let mut out = div().flex().flex_col().gap(px(8.));
  for row in rows {
    let mut row_el = div().flex().flex_row().items_start().gap(px(8.));
    for field in row {
      row_el = row_el.child(
        div()
          .flex_1()
          .min_w_0()
          .flex()
          .flex_col()
          .gap(px(2.))
          .child(
            div().text_size(tokens::TYPE_M).line_height(px(18.)).font_weight(FontWeight::BOLD).text_color(tokens::TEXT).child(field.name.clone()),
          )
          .child(
            div().text_size(tokens::TYPE_M).line_height(px(18.)).font_weight(FontWeight::NORMAL).text_color(BODY_TEXT).child(render_blocks(
              &field.value,
              &HashSet::new(),
              false,
              window,
              cx,
            )),
          ),
      );
    }
    out = out.child(row_el);
  }
  out
}

/// An embed image sized to fit the media box; click opens `url`.
fn embed_media(media: &EmbedMedia, id: impl Into<ElementId>, url: Option<&str>, radius: Pixels) -> AnyElement {
  let src = media.proxy_url.clone().unwrap_or_else(|| media.url.clone());
  let dims = fit(media.width, media.height, MEDIA_MAX_W, MEDIA_MAX_H);
  linkify(div().max_w_full().rounded(radius).overflow_hidden().child(sized_img(src, dims)), id, url)
}

/// Poster image (thumbnail or image) with a play button; click opens the embed url.
fn embed_video(embed: &Embed, index: usize) -> AnyElement {
  let poster = embed.thumbnail.as_ref().or(embed.image.as_ref());
  let (w, h) = poster
    .and_then(|p| fit(p.width, p.height, MEDIA_MAX_W, MEDIA_MAX_H))
    .or_else(|| embed.video.as_ref().and_then(|v| fit(v.width, v.height, MEDIA_MAX_W, MEDIA_MAX_H)))
    .unwrap_or((MEDIA_MAX_W, 225.));
  let url = embed.url.as_deref().or(embed.video.as_ref().map(|v| v.url.as_str()));

  let frame = div()
    .relative()
    .w(px(w))
    .h(px(h))
    .max_w_full()
    .rounded(tokens::RADIUS_100)
    .overflow_hidden()
    .bg(tokens::BG)
    .flex()
    .items_center()
    .justify_center()
    .children(poster.map(|p| img(p.proxy_url.clone().unwrap_or_else(|| p.url.clone())).absolute().inset_0().size_full().object_fit(ObjectFit::Cover)))
    .child(play_circle(48., tokens::BG_SURFACE));

  linkify(frame, ("embed-video", index), url)
}

// ---- stickers -----------------------------------------------------------------------------

fn render_sticker(sticker: &Sticker) -> AnyElement {
  match (&sticker.url, sticker.format) {
    (Some(url), format) if format != StickerFormat::Lottie => img(url.clone()).size(px(160.)).object_fit(ObjectFit::Contain).into_any_element(),
    _ => div()
      .size(px(160.))
      .rounded(tokens::RADIUS_200)
      .bg(tokens::BG_SURFACE)
      .flex()
      .items_center()
      .justify_center()
      .p(px(12.))
      .text_size(tokens::TYPE_S)
      .line_height(px(16.))
      .text_color(tokens::TEXT_TERTIARY)
      .child(sticker.name.clone())
      .into_any_element(),
  }
}

// ---- polls --------------------------------------------------------------------------------

fn render_poll(poll: &Poll) -> AnyElement {
  let total = poll.total_votes.max(poll.answers.iter().map(|a| a.votes).sum());
  let closed = poll.finalized || poll.expires_at.is_some_and(|t| t <= Utc::now());
  let hint = if poll.allow_multiselect {
    "Select one or more answers"
  } else {
    "Select one answer"
  };

  let votes = if total == 1 { "1 vote".to_owned() } else { format!("{total} votes") };
  let status = if closed {
    Some("Poll closed".to_owned())
  } else {
    poll.expires_at.map(time_left)
  };
  let footer = match status {
    Some(status) => format!("{votes} • {status}"),
    None => votes,
  };

  div()
    .w(px(CARD_W))
    .max_w_full()
    .rounded(tokens::RADIUS_200)
    .bg(tokens::BG_SURFACE_SECONDARY)
    .p(px(16.))
    .flex()
    .flex_col()
    .gap(px(8.))
    .child(div().text_size(tokens::TYPE_L).line_height(px(22.)).font_weight(FontWeight::BOLD).text_color(tokens::TEXT).child(poll.question.clone()))
    .child(div().text_size(tokens::TYPE_S).line_height(px(16.)).text_color(tokens::TEXT_TERTIARY).child(hint))
    .children(poll.answers.iter().map(|answer| poll_answer(answer, total)))
    .child(div().mt(px(4.)).text_size(tokens::TYPE_S).line_height(px(16.)).text_color(tokens::TEXT_TERTIARY).child(footer))
    .into_any_element()
}

fn poll_answer(answer: &PollAnswer, total: u64) -> Div {
  let fraction = if total == 0 { 0. } else { answer.votes as f32 / total as f32 };
  let percent = (fraction * 100.).round() as u32;
  let fill = if answer.me_voted {
    tokens::BG_FILL_BRAND_SECONDARY
  } else {
    tokens::BG_FILL
  };

  div()
    .relative()
    .h(px(40.))
    .rounded(tokens::RADIUS_150)
    .border_1()
    .border_color(tokens::BORDER)
    .overflow_hidden()
    .child(div().absolute().top_0().bottom_0().left_0().w(relative(fraction)).bg(fill))
    .child(
      div()
        .relative()
        .h_full()
        .px(px(12.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .children(answer.emoji.as_ref().map(|emoji| emoji_element(emoji, 16.)))
        .child(div().flex_1().min_w_0().truncate().text_size(tokens::TYPE_M).line_height(px(18.)).text_color(tokens::TEXT).child(answer.text.clone()))
        .child(
          div()
            .flex_shrink_0()
            .text_size(tokens::TYPE_M)
            .line_height(px(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(tokens::TEXT_SECONDARY)
            .child(format!("{percent}%")),
        ),
    )
}

// ---- components ---------------------------------------------------------------------------

fn render_component_row(row: &ComponentRow, row_index: usize) -> AnyElement {
  div()
    .flex()
    .flex_row()
    .flex_wrap()
    .items_center()
    .gap(px(8.))
    .children(row.components.iter().enumerate().filter_map(|(index, component)| render_component(component, row_index * 8 + index)))
    .into_any_element()
}

fn render_component(component: &Component, id: usize) -> Option<AnyElement> {
  match component {
    Component::Button {
      label,
      style,
      url,
      emoji,
      disabled,
    } => {
      let bg = match style {
        ButtonStyle::Primary => tokens::TEXT_LINK,
        ButtonStyle::Secondary | ButtonStyle::Link => tokens::BG_FILL,
        ButtonStyle::Success => tokens::TEXT_SUCCESS,
        ButtonStyle::Danger => tokens::TEXT_DANGER,
      };

      let button = div()
        .h(px(32.))
        .px(px(16.))
        .rounded(tokens::RADIUS_100)
        .bg(bg)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.))
        .text_size(tokens::TYPE_M)
        .line_height(px(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(white())
        .when(*disabled, |this| this.opacity(0.5))
        .children(emoji.as_ref().map(|emoji| emoji_element(emoji, 16.)))
        .children(label.clone().map(|label| div().whitespace_nowrap().child(label)))
        .when(*style == ButtonStyle::Link, |this| {
          this.child(icon("icons/external-link.svg", 16., white()))
        });

      let url = if *style == ButtonStyle::Link && !disabled {
        url.as_deref()
      } else {
        None
      };
      Some(linkify(button, ("component", id), url))
    }
    Component::Select { placeholder, disabled } => Some(
      div()
        .w(px(400.))
        .max_w_full()
        .h(px(40.))
        .rounded(tokens::RADIUS_100)
        .bg(tokens::BG)
        .border_1()
        .border_color(tokens::BORDER)
        .px(px(12.))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .when(*disabled, |this| this.opacity(0.5))
        .child(
          div()
            .min_w_0()
            .truncate()
            .text_size(tokens::TYPE_M)
            .line_height(px(18.))
            .text_color(tokens::TEXT_SECONDARY)
            .child(placeholder.clone().unwrap_or_else(|| "Make a selection".into())),
        )
        .child(icon("icons/chevron-down.svg", 16., tokens::ICON))
        .into_any_element(),
    ),
    Component::Other => None,
  }
}

// ---- reactions ----------------------------------------------------------------------------

/// "zach, luke and 3 others reacted with 🔥" — names are best-effort, so fall
/// back to the bare count when none are known.
fn reaction_tooltip(reaction: &Reaction) -> String {
  let label = reaction.emoji.label();
  let shown: Vec<&str> = reaction.users.iter().take(5).map(String::as_str).collect();
  let extra = (reaction.count as usize).saturating_sub(shown.len());

  match (shown.is_empty(), extra) {
    (true, 1) => format!("1 person reacted with {label}"),
    (true, n) => format!("{n} people reacted with {label}"),
    (false, 0) => format!("{} reacted with {label}", join_names(&shown)),
    (false, 1) => format!("{} and 1 other reacted with {label}", shown.join(", ")),
    (false, n) => format!("{} and {n} others reacted with {label}", shown.join(", ")),
  }
}

fn join_names(names: &[&str]) -> String {
  match names {
    [] => String::new(),
    [a] => (*a).to_string(),
    [head @ .., last] => format!("{} and {last}", head.join(", ")),
  }
}

fn render_reactions(reactions: &[Reaction], on_reaction: Option<crate::view::ReactionHandler>) -> AnyElement {
  div()
    .flex()
    .flex_row()
    .flex_wrap()
    .items_center()
    .gap(px(4.))
    .mt(px(4.))
    .children(reactions.iter().enumerate().map(|(index, reaction)| {
      let emoji = reaction.emoji.clone();
      let handler = on_reaction.clone();
      let me = reaction.me;
      let who = reaction_tooltip(reaction);

      div()
        .id(("reaction", index))
        .tooltip(move |window, cx| Tooltip::new(who.clone()).build(window, cx))
        .h(px(22.))
        .pl(px(6.))
        .pr(px(7.))
        .rounded(tokens::RADIUS_200)
        .border_1()
        .bg(if me { tokens::BRAND.opacity(0.16) } else { tokens::BG_SURFACE_SECONDARY })
        .border_color(if me { tokens::BRAND.opacity(0.55) } else { tokens::BORDER })
        .hover(|style| {
          if me {
            style.bg(tokens::BRAND.opacity(0.24))
          } else {
            style.bg(tokens::BG_SURFACE).border_color(tokens::BORDER_TERTIARY)
          }
        })
        .active(|style| style.opacity(0.85))
        .cursor_pointer()
        .on_click(move |_, window, cx| {
          if let Some(handler) = &handler {
            handler(emoji.clone(), window, cx);
          }
        })
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.))
        .child(emoji_element(&reaction.emoji, 15.))
        .child(
          div()
            .text_size(px(13.))
            .line_height(px(16.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if me { tokens::TEXT } else { tokens::TEXT_SECONDARY })
            .child(reaction.count.to_string()),
        )
    }))
    .into_any_element()
}
