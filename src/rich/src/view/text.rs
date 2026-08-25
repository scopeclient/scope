//! Markdown body renderer: blocks and inlines.
//!
//! Every paragraph becomes a single `InteractiveText`, so wrapping, hit-testing
//! and (eventually) selection behave like one run of text. Inline styles —
//! bold/italic/underline/strike, links, mention pills, inline code, spoilers,
//! timestamps — are `TextRun`s over that text; links and hidden spoilers are
//! clickable ranges. The one exception is custom emoji: gpui cannot embed an
//! image in a text run, so a paragraph containing them is split at the emoji
//! into a wrapping row of text segments and `img`s (and an emoji-only paragraph
//! is shown "jumbo" at 48px).

use std::{collections::HashSet, ops::Range};

use chrono::{DateTime, Local, Utc};
use gpui::{
  AnyElement, Context, Div, FontStyle, FontWeight, Hsla, InteractiveText, IntoElement, ObjectFit, ParentElement, Pixels, StrikethroughStyle, Styled,
  StyledImage, StyledText, TextRun, TextStyle, UnderlineStyle, WeakEntity, Window, div, img, px,
};
use scope_theme as tokens;

use crate::{
  model::{Block, Emoji, Inline, Mention, TextStyle as MdStyle, TimestampStyle},
  view::RichContentView,
};

/// Monospace family for code. gpui resolves unknown family names to the system
/// UI font on macOS, so the generic `monospace` alias only works where
/// fontconfig expands it.
const MONO_FONT: &str = if cfg!(target_os = "macos") {
  "Menlo"
} else if cfg!(target_os = "windows") {
  "Consolas"
} else {
  "monospace"
};

/// Custom emoji inline with text (Discord: 1.375em of 16px).
const INLINE_EMOJI: f32 = 22.;
/// Emoji in a paragraph that is nothing but emoji.
const JUMBO_EMOJI: f32 = 48.;
const CODE_TEXT: Pixels = px(13.);
const CODE_LINE: Pixels = px(18.);
/// Vertical gap between blocks of different kinds.
const BLOCK_GAP: Pixels = px(4.);
const QUOTE_BAR: Pixels = px(4.);
const QUOTE_GAP: Pixels = px(12.);
/// Foreground and background of a spoiler that has not been revealed: the text
/// is drawn in its own background colour so the run reads as a solid pill.
const SPOILER_HIDDEN: Hsla = tokens::BG_FILL;
const SPOILER_REVEALED: Hsla = tokens::BG_SURFACE_SECONDARY;
const CODE_INLINE_BG: Hsla = tokens::BG_SURFACE_SECONDARY;
const MENTION_BG: Hsla = tokens::BG_FILL_BRAND_SECONDARY;
/// Alpha applied to a role colour used as a mention pill background.
const ROLE_BG_ALPHA: u32 = 0x4d;

pub fn render_blocks(
  blocks: &[Block],
  revealed_spoilers: &HashSet<usize>,
  edited: bool,
  window: &mut Window,
  cx: &mut Context<RichContentView>,
) -> AnyElement {
  let base = window.text_style();
  let line_height = base.line_height_in_pixels(window.rem_size());
  let mut renderer = Renderer {
    revealed: revealed_spoilers,
    view: cx.weak_entity(),
    spoiler_ix: 0,
    text_ix: 0,
  };
  let mut marker = edited.then(|| edited_marker(line_height));

  let mut root = div().w_full().flex().flex_col().gap(BLOCK_GAP);
  let mut previous: Option<&Block> = None;
  for (i, block) in blocks.iter().enumerate() {
    let is_last = i + 1 == blocks.len();
    let mut child = match block {
      // The marker flows after the last line of a final paragraph.
      Block::Paragraph(inlines) if is_last => renderer.paragraph(inlines, &base, marker.take()),
      _ => renderer.block(block, &base),
    };
    // Two paragraphs in a row came from a blank source line: show the blank line.
    if let (Some(Block::Paragraph(_)), Block::Paragraph(_)) = (previous, block) {
      child = div().mt(line_height - BLOCK_GAP).child(child).into_any_element();
    }
    root = root.child(child);
    previous = Some(block);
  }

  root.children(marker).into_any_element()
}

struct Renderer<'a> {
  revealed: &'a HashSet<usize>,
  view: WeakEntity<RichContentView>,
  /// Document-order spoiler counter; must match the indices the view stores.
  spoiler_ix: usize,
  /// Makes each `InteractiveText` id unique within the message.
  text_ix: usize,
}

/// What a click on a range of text does.
enum Action {
  Link(String),
  Spoiler(usize),
}

/// A run of text plus the styles and click targets laid over it.
#[derive(Default)]
struct TextBuilder {
  text: String,
  runs: Vec<TextRun>,
  actions: Vec<(Range<usize>, Action)>,
}

impl TextBuilder {
  fn push(&mut self, text: &str, style: &TextStyle) {
    if text.is_empty() {
      return;
    }
    self.text.push_str(text);
    let run = style.to_run(text.len());
    match self.runs.last_mut() {
      Some(last) if same_style(last, &run) => last.len += run.len,
      _ => self.runs.push(run),
    }
  }
}

fn same_style(a: &TextRun, b: &TextRun) -> bool {
  a.font == b.font
    && a.color == b.color
    && a.background_color == b.background_color
    && a.underline == b.underline
    && a.strikethrough == b.strikethrough
}

/// A paragraph is a sequence of text segments broken only by custom emoji.
enum Segment {
  Text(TextBuilder),
  Emoji(Emoji),
}

/// The text builder at the end of `segments`, starting a new one after an emoji.
fn current(segments: &mut Vec<Segment>) -> &mut TextBuilder {
  if !matches!(segments.last(), Some(Segment::Text(_))) {
    segments.push(Segment::Text(TextBuilder::default()));
  }
  match segments.last_mut() {
    Some(Segment::Text(builder)) => builder,
    _ => unreachable!(),
  }
}

impl Renderer<'_> {
  fn block(&mut self, block: &Block, base: &TextStyle) -> AnyElement {
    match block {
      Block::Paragraph(inlines) => self.paragraph(inlines, base, None),

      Block::Heading { level, content } => {
        let (size, line) = match level {
          1 => (tokens::HEADING_M, tokens::HEADING_M_LINE),
          2 => (tokens::HEADING_S, tokens::HEADING_S_LINE),
          _ => (tokens::TYPE_L, tokens::TYPE_L_LINE),
        };
        let style = TextStyle {
          font_weight: FontWeight::BOLD,
          color: tokens::TEXT,
          ..base.clone()
        };
        div()
          .w_full()
          .text_size(size)
          .line_height(line)
          .font_weight(FontWeight::BOLD)
          .text_color(tokens::TEXT)
          .child(self.paragraph(content, &style, None))
          .into_any_element()
      }

      Block::Subtext(content) => {
        let style = TextStyle {
          color: tokens::TEXT_TERTIARY,
          ..base.clone()
        };
        div()
          .w_full()
          .text_size(tokens::TYPE_S)
          .line_height(tokens::TYPE_S_LINE)
          .text_color(tokens::TEXT_TERTIARY)
          .child(self.paragraph(content, &style, None))
          .into_any_element()
      }

      Block::Quote(inner) => div()
        .w_full()
        .flex()
        .flex_row()
        .gap(QUOTE_GAP)
        .child(div().w(QUOTE_BAR).flex_shrink_0().rounded(tokens::RADIUS_050).bg(tokens::BORDER_TERTIARY))
        .child(self.blocks(inner, base).flex_1().min_w_0())
        .into_any_element(),

      Block::CodeBlock { language, code } => code_block(language.as_deref(), code, base),

      Block::List { ordered, start, items } => {
        let mut list = div().w_full().flex().flex_col().gap(px(2.));
        for (i, item) in items.iter().enumerate() {
          let marker = if *ordered {
            format!("{}.", start.saturating_add(i as u32))
          } else {
            "•".to_string()
          };
          list = list.child(
            div()
              .flex()
              .flex_row()
              .items_start()
              .gap(px(6.))
              .child(div().flex_shrink_0().min_w(px(14.)).whitespace_nowrap().child(marker))
              .child(self.blocks(item, base).flex_1().min_w_0()),
          );
        }
        list.into_any_element()
      }
    }
  }

  fn blocks(&mut self, blocks: &[Block], base: &TextStyle) -> Div {
    let mut column = div().flex().flex_col().gap(BLOCK_GAP);
    for block in blocks {
      column = column.child(self.block(block, base));
    }
    column
  }

  /// Inline content as one text, or a wrapping row when custom emoji split it.
  /// `trailing` is appended after the last segment (the "(edited)" marker).
  fn paragraph(&mut self, inlines: &[Inline], base: &TextStyle, trailing: Option<AnyElement>) -> AnyElement {
    if is_emoji_only(inlines) {
      let mut row = div().w_full().flex().flex_row().flex_wrap().items_center().gap(px(4.));
      for inline in inlines {
        if let Inline::Emoji(emoji) = inline {
          row = row.child(emoji_element(emoji, JUMBO_EMOJI));
        }
      }
      return row.children(trailing).into_any_element();
    }

    let mut segments = Vec::new();
    self.walk(inlines, base, false, &mut segments);

    let mut elements: Vec<AnyElement> = Vec::with_capacity(segments.len());
    let mut only_text = true;
    for segment in segments {
      match segment {
        Segment::Text(builder) => elements.extend(self.text_element(builder)),
        Segment::Emoji(emoji) => {
          only_text = false;
          elements.push(emoji_element(&emoji, INLINE_EMOJI));
        }
      }
    }

    if only_text && trailing.is_none() && elements.len() == 1 {
      return elements.pop().expect("one element");
    }

    // gpui measures a text's min-content width as its unwrapped width, so as a
    // flex item it could never shrink; `min_w_0` lets it take the row width
    // and wrap inside it.
    let items = elements.into_iter().map(|element| div().min_w_0().child(element));
    div().w_full().flex().flex_row().flex_wrap().items_center().children(items).children(trailing).into_any_element()
  }

  fn text_element(&mut self, builder: TextBuilder) -> Option<AnyElement> {
    if builder.text.is_empty() {
      return None;
    }
    let id = ("rich-text", self.text_ix);
    self.text_ix += 1;

    let (ranges, actions): (Vec<Range<usize>>, Vec<Action>) = builder.actions.into_iter().unzip();
    let view = self.view.clone();
    let text =
      InteractiveText::new(id, StyledText::new(builder.text).with_runs(builder.runs)).on_click(ranges, move |ix, _window, cx| match &actions[ix] {
        Action::Link(url) => cx.open_url(url),
        Action::Spoiler(index) => {
          let index = *index;
          view.update(cx, |this, cx| this.reveal_spoiler(index, cx)).ok();
        }
      });
    Some(text.into_any_element())
  }

  /// Flatten `inlines` into `segments`, carrying the accumulated `style`.
  /// `hidden` is true inside an unrevealed spoiler: colours are frozen to the
  /// spoiler pill and nothing inside is clickable or shown as an image.
  fn walk(&mut self, inlines: &[Inline], style: &TextStyle, hidden: bool, segments: &mut Vec<Segment>) {
    for inline in inlines {
      match inline {
        Inline::Text(text) => current(segments).push(text, style),
        Inline::LineBreak => current(segments).push("\n", style),

        Inline::Styled { style: md, content } => {
          let mut styled = style.clone();
          match md {
            MdStyle::Bold => styled.font_weight = FontWeight::BOLD,
            MdStyle::Italic => styled.font_style = FontStyle::Italic,
            // Decorations would show through the hidden pill.
            MdStyle::Underline if !hidden => {
              styled.underline = Some(UnderlineStyle {
                thickness: px(1.),
                color: None,
                wavy: false,
              })
            }
            MdStyle::Strikethrough if !hidden => {
              styled.strikethrough = Some(StrikethroughStyle {
                thickness: px(1.),
                color: None,
              })
            }
            MdStyle::Underline | MdStyle::Strikethrough => {}
          }
          self.walk(content, &styled, hidden, segments);
        }

        Inline::Spoiler(content) => {
          let index = self.spoiler_ix;
          self.spoiler_ix += 1;
          if hidden || self.revealed.contains(&index) {
            let styled = if hidden {
              style.clone()
            } else {
              TextStyle {
                background_color: Some(SPOILER_REVEALED),
                ..style.clone()
              }
            };
            self.walk(content, &styled, hidden, segments);
          } else {
            let styled = TextStyle {
              color: SPOILER_HIDDEN,
              background_color: Some(SPOILER_HIDDEN),
              underline: None,
              strikethrough: None,
              ..style.clone()
            };
            let mark = mark(segments);
            self.walk(content, &styled, true, segments);
            record(segments, mark, Action::Spoiler(index));
          }
        }

        Inline::Code(code) => {
          let mut styled = TextStyle {
            font_family: MONO_FONT.into(),
            ..style.clone()
          };
          if styled.font_weight != FontWeight::BOLD {
            styled.font_weight = FontWeight::NORMAL;
          }
          if !hidden {
            styled.color = tokens::TEXT;
            styled.background_color = Some(CODE_INLINE_BG);
          }
          current(segments).push(code, &styled);
        }

        Inline::Link { url, label } => {
          let styled = if hidden {
            style.clone()
          } else {
            TextStyle {
              color: tokens::TEXT_LINK,
              ..style.clone()
            }
          };
          let mark = mark(segments);
          match label {
            Some(label) => self.walk(label, &styled, hidden, segments),
            None => current(segments).push(url, &styled),
          }
          if !hidden {
            record(segments, mark, Action::Link(url.clone()));
          }
        }

        Inline::Mention(mention) => {
          let (text, color) = match mention {
            Mention::User { name, .. } => (format!("@{name}"), None),
            Mention::Channel { name, .. } => (format!("#{name}"), None),
            Mention::Role { name, color, .. } => (format!("@{name}"), color.filter(|c| *c != 0)),
            Mention::Command { name } => (format!("/{name}"), None),
            Mention::Everyone => ("@everyone".to_string(), None),
            Mention::Here => ("@here".to_string(), None),
          };
          let mut styled = TextStyle {
            font_weight: FontWeight::SEMIBOLD,
            ..style.clone()
          };
          if !hidden {
            let (fg, bg) = match color {
              Some(rgb) => (tokens::hex(rgb), tokens::hexa((rgb << 8) | ROLE_BG_ALPHA)),
              None => (tokens::TEXT, MENTION_BG),
            };
            styled.color = fg;
            styled.background_color = Some(bg);
          }
          current(segments).push(&text, &styled);
        }

        Inline::Emoji(emoji) => match emoji {
          // Colour emoji glyphs ignore the text colour, so blank them out.
          Emoji::Unicode(_) if hidden => current(segments).push("  ", style),
          Emoji::Unicode(text) => current(segments).push(text, style),
          Emoji::Custom { .. } if hidden => current(segments).push(&emoji.label(), style),
          Emoji::Custom { .. } => segments.push(Segment::Emoji(emoji.clone())),
        },

        Inline::Timestamp { unix, style: ts } => {
          let styled = if hidden {
            style.clone()
          } else {
            TextStyle {
              background_color: Some(tokens::BG_SURFACE),
              ..style.clone()
            }
          };
          current(segments).push(&format_timestamp(*unix, *ts), &styled);
        }
      }
    }
  }
}

/// Position in the current text builder before a clickable construct.
fn mark(segments: &mut Vec<Segment>) -> (usize, usize) {
  let offset = current(segments).text.len();
  (segments.len(), offset)
}

/// Make the text since `mark` clickable — provided it all landed in the same
/// builder (a label containing a custom emoji spans segments and is skipped).
fn record(segments: &mut [Segment], (count, start): (usize, usize), action: Action) {
  if segments.len() != count {
    return;
  }
  if let Some(Segment::Text(builder)) = segments.last_mut() {
    let end = builder.text.len();
    if end > start {
      builder.actions.push((start..end, action));
    }
  }
}

/// Nothing but emoji and whitespace, and at least one emoji.
fn is_emoji_only(inlines: &[Inline]) -> bool {
  let mut any = false;
  for inline in inlines {
    match inline {
      Inline::Emoji(_) => any = true,
      Inline::Text(text) if text.trim().is_empty() => {}
      _ => return false,
    }
  }
  any
}

fn emoji_element(emoji: &Emoji, size: f32) -> AnyElement {
  // The CDN wants a power-of-two size; fetch at 2x for crisp rendering.
  let cdn_size = (size * 2.) as u32;
  match emoji.image_url(cdn_size.next_power_of_two()) {
    Some(url) => img(url).size(px(size)).flex_shrink_0().object_fit(ObjectFit::Contain).into_any_element(),
    None => div().flex_shrink_0().text_size(px(size * 0.8)).line_height(px(size)).child(emoji.label()).into_any_element(),
  }
}

fn edited_marker(line_height: Pixels) -> AnyElement {
  div()
    .flex_shrink_0()
    .pl(px(4.))
    .text_size(tokens::TYPE_XS)
    .line_height(line_height)
    .font_weight(FontWeight::NORMAL)
    .text_color(tokens::TEXT_TERTIARY)
    .whitespace_nowrap()
    .child("(edited)")
    .into_any_element()
}

fn code_block(language: Option<&str>, code: &str, base: &TextStyle) -> AnyElement {
  let code = code.trim_end_matches(['\n', '\r']).replace('\t', "    ");
  let language = language.map(str::trim).filter(|l| !l.is_empty());
  // Keep the first line clear of the label (roughly 6.5px per character).
  let label_width = language.map_or(0., |l| l.chars().count() as f32 * 6.5 + 8.);

  div()
    .w_full()
    .relative()
    .bg(tokens::BG)
    .border_1()
    .border_color(tokens::BORDER)
    .rounded(tokens::RADIUS_100)
    .p(px(8.))
    .font_family(MONO_FONT)
    .font_weight(FontWeight::NORMAL)
    .text_size(CODE_TEXT)
    .line_height(CODE_LINE)
    .text_color(tokens::TEXT)
    .child(div().pr(px(label_width)).child(code))
    .children(language.map(|language| {
      div()
        .absolute()
        .top(px(4.))
        .right(px(8.))
        .font_family(base.font_family.clone())
        .text_size(tokens::TYPE_XS)
        .line_height(tokens::TYPE_XS_LINE)
        .text_color(tokens::TEXT_TERTIARY)
        .child(language.to_string())
    }))
    .into_any_element()
}

/// `<t:unix:style>` in the local time zone.
fn format_timestamp(unix: i64, style: TimestampStyle) -> String {
  let Some(utc) = DateTime::<Utc>::from_timestamp(unix, 0) else {
    return format!("<t:{unix}>");
  };
  let local = utc.with_timezone(&Local);
  let pattern = match style {
    TimestampStyle::ShortTime => "%-I:%M %p",
    TimestampStyle::LongTime => "%-I:%M:%S %p",
    TimestampStyle::ShortDate => "%d/%m/%Y",
    TimestampStyle::LongDate => "%-d %B %Y",
    TimestampStyle::ShortDateTime | TimestampStyle::Default => "%-d %B %Y %-I:%M %p",
    TimestampStyle::LongDateTime => "%A, %-d %B %Y %-I:%M %p",
    TimestampStyle::Relative => return relative((Utc::now() - utc).num_seconds()),
  };
  local.format(pattern).to_string()
}

/// "3 minutes ago" / "in 2 days", with moment.js-style rounding.
fn relative(delta_secs: i64) -> String {
  const MINUTE: u64 = 60;
  const HOUR: u64 = 60 * MINUTE;
  const DAY: u64 = 24 * HOUR;
  const MONTH: u64 = 30 * DAY;
  const YEAR: u64 = 365 * DAY;

  let secs = delta_secs.unsigned_abs();
  let round = |unit: u64| (secs + unit / 2) / unit;
  let body = match secs {
    s if s < 45 => "a few seconds".to_string(),
    s if s < 90 => "a minute".to_string(),
    s if s < 45 * MINUTE => format!("{} minutes", round(MINUTE)),
    s if s < 90 * MINUTE => "an hour".to_string(),
    s if s < 22 * HOUR => format!("{} hours", round(HOUR)),
    s if s < 36 * HOUR => "a day".to_string(),
    s if s < 26 * DAY => format!("{} days", round(DAY)),
    s if s < 46 * DAY => "a month".to_string(),
    s if s < 320 * DAY => format!("{} months", round(MONTH)),
    s if s < 548 * DAY => "a year".to_string(),
    _ => format!("{} years", round(YEAR)),
  };

  if delta_secs >= 0 { format!("{body} ago") } else { format!("in {body}") }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn relative_rounds_like_moment() {
    assert_eq!(relative(10), "a few seconds ago");
    assert_eq!(relative(-70), "in a minute");
    assert_eq!(relative(5 * 60), "5 minutes ago");
    assert_eq!(relative(-3 * 3600), "in 3 hours");
    assert_eq!(relative(3 * 86400), "3 days ago");
    assert_eq!(relative(400 * 86400), "a year ago");
  }

  #[test]
  fn emoji_only_detection() {
    let shrug = Inline::Emoji(Emoji::Unicode("🤷".into()));
    assert!(is_emoji_only(std::slice::from_ref(&shrug)));
    assert!(is_emoji_only(&[shrug.clone(), Inline::Text(" ".into()), shrug.clone()]));
    assert!(!is_emoji_only(&[shrug, Inline::Text("hi".into())]));
    assert!(!is_emoji_only(&[Inline::Text(" ".into())]));
  }

  #[test]
  fn runs_cover_text_and_merge() {
    let mut builder = TextBuilder::default();
    let style = TextStyle::default();
    builder.push("hello ", &style);
    builder.push("world", &style);
    builder.push(
      "!",
      &TextStyle {
        color: tokens::TEXT_LINK,
        ..style
      },
    );
    assert_eq!(builder.runs.len(), 2);
    assert_eq!(builder.runs.iter().map(|r| r.len).sum::<usize>(), builder.text.len());
  }
}
