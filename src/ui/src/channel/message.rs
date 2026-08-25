//! A run of consecutive messages from one author, and the row that renders it.
//!
//! Row metrics follow the Figma "message" component: 36px avatar at (17, 9),
//! text column at x=72, a 21px header line (name + timestamp) and 21px body
//! lines, so a row is `38 + 21 * lines` tall.

use chrono::{DateTime, Local, Utc};
use gpui::{App, FontWeight, Hsla, IntoElement, ParentElement, Styled, Window, div, prelude::*, px};
use scope_chat::message::{IconRenderConfig, Message, MessageAuthor};

use crate::theme::tokens;

pub const ROW_PAD_TOP: f32 = 8.;
pub const ROW_PAD_BOTTOM: f32 = 9.;
pub const ROW_PAD_LEFT: f32 = 17.;
pub const ROW_PAD_RIGHT: f32 = 16.;
pub const AVATAR_SIZE: f32 = 36.;
/// Avatar right edge (53) to the text column (72).
pub const AVATAR_GAP: f32 = 19.;
/// Line box for 14px text in the design (1.5x).
pub const LINE_HEIGHT: f32 = 21.;
/// Gap between the author name and the timestamp.
pub const TIMESTAMP_GAP: f32 = 6.;

/// Message body colour. The design uses `#c4c8d4`, which sits between `TEXT`
/// and `TEXT_SECONDARY`; no token matches.
pub const BODY_TEXT: Hsla = tokens::hex(0xc4c8d4);
/// Hovered row background. Between `BG` and `BG_SECONDARY`; no token matches.
pub const ROW_HOVER_BG: Hsla = tokens::hex(0x131418);

#[derive(Clone)]
pub struct MessageGroup<M: Message> {
  contents: Vec<M>,
}

impl<M: Message> MessageGroup<M> {
  pub fn new(message: M) -> MessageGroup<M> {
    MessageGroup { contents: vec![message] }
  }

  pub fn get_author(&self) -> M::Author {
    self.contents.first().unwrap().get_author()
  }

  pub fn add(&mut self, message: M) {
    // FIXME: This is scuffed, should be using PartialEq trait.
    if self.get_author().get_identifier() != message.get_author().get_identifier() {
      panic!("Authors must match in a message group")
    }

    self.contents.push(message);
  }

  pub fn size(&self) -> usize {
    self.contents.len()
  }

  pub fn remove(&mut self, index: usize) {
    if self.size() == 1 {
      panic!("Cannot remove such that it would leave the group empty.");
    }

    self.contents.remove(index);
  }

  pub fn first(&self) -> &M {
    self.contents.first().unwrap()
  }

  pub fn last(&self) -> &M {
    self.contents.last().unwrap()
  }

  pub fn messages(&self) -> &[M] {
    &self.contents
  }
}

/// "Today at 1:52 PM", "Yesterday at 9:03 AM", otherwise "24/08/2026 13:52" (local time).
pub fn format_timestamp(timestamp: DateTime<Utc>) -> String {
  let local = timestamp.with_timezone(&Local);
  let today = Local::now().date_naive();
  let date = local.date_naive();
  let time = local.format("%-I:%M %P");

  if date == today {
    format!("today at {time}")
  } else if date.succ_opt() == Some(today) {
    format!("yesterday at {time}")
  } else {
    local.format("%d/%m/%Y %H:%M").to_string()
  }
}

/// The full-width row shell shared by real and placeholder rows.
fn row() -> gpui::Div {
  div()
    .w_full()
    .flex()
    .flex_row()
    .items_start()
    .pt(px(ROW_PAD_TOP))
    .pb(px(ROW_PAD_BOTTOM))
    .pl(px(ROW_PAD_LEFT))
    .pr(px(ROW_PAD_RIGHT))
    .gap(px(AVATAR_GAP))
}

/// 36px circle the avatar image is clipped to. The backdrop matches the list
/// background so a transparent or still-loading avatar reads as empty space.
fn avatar_frame(fill: Hsla) -> gpui::Div {
  div().flex_shrink_0().mt(px(1.)).size(px(AVATAR_SIZE)).rounded_full().bg(fill).overflow_hidden()
}

/// One author header followed by every message body in the group, each body
/// on its own 21px line(s).
pub fn message_group<M: Message>(
  group: MessageGroup<M>,
  _actions: super::actions::MessageActions<M>,
  window: &mut Window,
  cx: &mut App,
) -> impl IntoElement {
  let author = group.get_author();
  let timestamp = group.first().get_timestamp().map(format_timestamp);
  let bodies: Vec<_> = group.messages().iter().map(|m| m.get_content(window, cx)).collect();

  row()
    .hover(|style| style.bg(ROW_HOVER_BG))
    .child(avatar_frame(tokens::BG_SECONDARY).child(author.get_icon(IconRenderConfig::small().with_size(128))))
    .child(
      div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(
          div()
            .h(px(LINE_HEIGHT))
            .min_w_0()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(TIMESTAMP_GAP))
            .child(
              div()
                .min_w_0()
                .text_size(tokens::TYPE_M)
                .line_height(px(LINE_HEIGHT))
                .font_weight(FontWeight::BOLD)
                .text_color(tokens::TEXT)
                .child(author.get_display_name()),
            )
            .children(timestamp.map(|timestamp| {
              div()
                .flex_shrink_0()
                .text_size(tokens::TYPE_XS)
                .line_height(px(15.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tokens::TEXT_TERTIARY)
                .whitespace_nowrap()
                .child(timestamp)
            })),
        )
        .child(div().text_size(tokens::TYPE_M).line_height(px(LINE_HEIGHT)).font_weight(FontWeight::MEDIUM).text_color(BODY_TEXT).children(bodies)),
    )
}

/// Placeholder for a row whose message is still being fetched: a faded avatar
/// circle plus name and body bars, at single-line row height (59px).
pub fn loading_row() -> impl IntoElement {
  let bar = |width: f32| div().h(px(10.)).w(px(width)).rounded_full().bg(tokens::BG_SURFACE);
  let line = |content: gpui::Div| div().h(px(LINE_HEIGHT)).flex().items_center().child(content);

  row()
    .opacity(0.5)
    .child(avatar_frame(tokens::BG_SURFACE))
    .child(div().flex_1().min_w_0().flex().flex_col().child(line(bar(72.))).child(line(bar(220.))))
}
