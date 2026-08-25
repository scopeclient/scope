//! gpui renderer for [`RichMessage`].
//!
//! `text.rs` renders the markdown body (blocks/inlines, spoilers, links,
//! mentions, emoji, code); `extras.rs` renders everything else (reply header,
//! attachments, embeds, stickers, polls, reactions, components, system notices).
//! This module only composes them.

pub mod extras;
pub mod selection;
pub mod text;

use std::{collections::HashSet, rc::Rc, sync::Arc};

use gpui::{App, ClipboardItem, Context, FocusHandle, Focusable, IntoElement, ParentElement, Styled, Window, div, prelude::*, px};
use scope_theme as tokens;

use crate::model::{Emoji, MessageKind, RichMessage};

/// Callback for clicks on a reaction pill (toggle that reaction).
pub type ReactionHandler = Rc<dyn Fn(Emoji, &mut Window, &mut App)>;

pub struct RichContentView {
  pub rich: Arc<RichMessage>,
  /// Indices (in document order) of spoilers the user has revealed.
  pub revealed_spoilers: HashSet<usize>,
  /// Attachment ids whose spoiler cover has been removed.
  pub revealed_attachments: HashSet<u64>,
  /// Set by the chat column so reaction pills can toggle reactions.
  pub on_reaction: Option<ReactionHandler>,
  /// Focus + selection for keyboard copy.
  focus: FocusHandle,
  selected: bool,
}

impl RichContentView {
  pub fn new(rich: Arc<RichMessage>, cx: &mut Context<Self>) -> Self {
    selection::install_keymap(cx);

    RichContentView {
      rich,
      revealed_spoilers: HashSet::new(),
      revealed_attachments: HashSet::new(),
      on_reaction: None,
      focus: cx.focus_handle(),
      selected: false,
    }
  }

  fn select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.selected = true;
    window.focus(&self.focus);
    cx.notify();
  }

  fn copy(&mut self, cx: &mut Context<Self>) {
    cx.write_to_clipboard(ClipboardItem::new_string(self.plain_text()));
  }

  /// The message flattened to plain text (reply snippet + body).
  fn plain_text(&self) -> String {
    let mut out = String::new();
    if let Some(reply) = &self.rich.reply {
      out.push_str(&format!("> {}: {}\n", reply.author_name, reply.snippet));
    }
    out.push_str(&crate::markdown::to_plain_text(&self.rich.blocks));
    out
  }

  pub fn reveal_spoiler(&mut self, index: usize, cx: &mut Context<Self>) {
    self.revealed_spoilers.insert(index);
    cx.notify();
  }

  pub fn reveal_attachment(&mut self, id: u64, cx: &mut Context<Self>) {
    self.revealed_attachments.insert(id);
    cx.notify();
  }
}

impl Focusable for RichContentView {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus.clone()
  }
}

impl Render for RichContentView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let rich = self.rich.clone();
    // A message stays "selected" only while it holds focus.
    let selected = self.selected && self.focus.is_focused(window);
    self.selected = selected;

    let body = if let MessageKind::System(kind) = &rich.kind {
      div().w_full().child(extras::render_system_notice(kind, &rich, window, cx))
    } else {
      div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(4.))
        .opacity(if rich.pending { 0.5 } else { 1.0 })
        .children(rich.reply.as_ref().map(|reply| extras::render_reply(reply, window, cx)))
        .children((!rich.blocks.is_empty()).then(|| text::render_blocks(&rich.blocks, &self.revealed_spoilers, rich.edited_at.is_some(), window, cx)))
        .children(extras::render_extras(
          &rich,
          &self.revealed_attachments,
          self.on_reaction.clone(),
          window,
          cx,
        ))
    };

    div()
      .id("message-content")
      .track_focus(&self.focus)
      .key_context(selection::MESSAGE_CONTEXT)
      .on_action(cx.listener(|this, _: &selection::CopyMessage, _window, cx| this.copy(cx)))
      .on_action(cx.listener(|this, _: &selection::SelectMessage, window, cx| this.select(window, cx)))
      .w_full()
      .rounded(tokens::RADIUS_100)
      .child(body)
  }
}
