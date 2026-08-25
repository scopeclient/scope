//! The chat column: a bottom-anchored message list with the composer under it.

pub mod actions;
pub mod message;
pub mod message_list;
pub mod typing;

use std::{rc::Rc, sync::Arc};

use gpui::{
  App, Context, Entity, FontWeight, Global, IntoElement, KeyBinding, ParentElement, Render, SharedString, Stateful, Styled, Window, actions, div,
  img, prelude::*, px,
};
use gpui_component::{
  Icon, h_flex,
  input::{Enter, Input, InputEvent, InputState},
  tooltip::Tooltip,
  v_flex,
};
use message_list::MessageListComponent;
use scope_chat::channel::{Channel, ChannelEvent};

use crate::channel::actions::{MessageAction, MessageActions};
use tokio::sync::broadcast::error::RecvError;

use crate::{icons::ScopeIcon, theme::tokens};

actions!(composer, [InsertNewline]);

/// Key context of the composer wrapper; bindings below apply while the
/// message field (a descendant) is focused.
const COMPOSER_CONTEXT: &str = "Composer";

// ---- composer metrics (Figma "message-bar") ------------------------------
const COMPOSER_PAD_LEFT: f32 = 17.;
const COMPOSER_PAD_RIGHT: f32 = 16.;
const COMPOSER_PAD_BOTTOM: f32 = 13.;
const TYPING_HEIGHT: f32 = 18.;
const TYPING_GAP: f32 = 4.;
const COMPOSER_ROW_HEIGHT: f32 = 32.;
const COMPOSER_AVATAR: f32 = 32.;
const COMPOSER_AVATAR_INSET: f32 = 2.;
const FIELD_GAP: f32 = 6.;
const BUTTONS_GAP: f32 = 9.;
/// Field text line box; with 5px vertical padding and a 1px border the field is 32px tall.
const FIELD_LINE_HEIGHT: f32 = 20.;
const FIELD_PAD_X: f32 = 10.;
const FIELD_PAD_Y: f32 = 5.;
const FIELD_MIN_ROWS: usize = 1;
const FIELD_MAX_ROWS: usize = 6;

/// Presentation details the chat column cannot derive from the channel itself.
#[derive(Clone, Debug, Default)]
pub struct ChannelViewOptions {
  /// Display title of the channel as shown on its tab, e.g. `#announcements`
  /// or a DM partner's name. Used for the composer placeholder.
  pub title: String,
  /// Avatar of the signed-in user, drawn next to the composer.
  pub composer_avatar: Option<String>,
  /// Who is typing in this channel (shared with `AppState`).
  pub typing: Option<Entity<typing::TypingIndicator>>,
}

/// Marker so the composer key bindings are installed once per app.
struct ComposerKeymapInstalled;

impl Global for ComposerKeymapInstalled {}

fn install_composer_keymap(cx: &mut App) {
  if cx.has_global::<ComposerKeymapInstalled>() {
    return;
  }

  cx.bind_keys([KeyBinding::new("shift-enter", InsertNewline, Some(COMPOSER_CONTEXT))]);
  cx.set_global(ComposerKeymapInstalled);
}

pub struct ChannelView<C: Channel + 'static> {
  channel: Arc<C>,
  list_view: Entity<MessageListComponent<Arc<C>>>,
  message_input: Entity<InputState>,
  options: ChannelViewOptions,
}

impl<C: Channel + 'static> ChannelView<C> {
  pub fn create(window: &mut Window, cx: &mut Context<Self>, channel: Arc<C>, options: ChannelViewOptions) -> Self {
    install_composer_keymap(cx);

    // Row-level actions; TODO(chat-actions): wire to the channel + reply/edit state.
    let actions: MessageActions<C::Message> = Rc::new(
      move |action: MessageAction<C::Message>, _window: &mut Window, _cx: &mut App| match action {
        MessageAction::React { .. } => log::info!("react"),
        MessageAction::Reply(_) => log::info!("reply"),
        MessageAction::Edit(_) => log::info!("edit"),
        MessageAction::Delete(_) => log::info!("delete"),
        MessageAction::CopyText(_) => log::info!("copy"),
      },
    );

    let list_view = cx.new(|cx| MessageListComponent::create(cx, channel.clone(), px(30.), actions));

    if let Some(typing) = &options.typing {
      cx.observe(typing, |_, _, cx| cx.notify()).detach();
    }

    // Pump live messages from the backend (tokio) onto the gpui foreground thread.
    let receiver = channel.get_receiver();
    let list = list_view.clone();
    let typing = options.typing.clone();
    cx.spawn(async move |_this, cx| {
      let mut receiver = receiver;

      loop {
        let (tx, rx) = catty::oneshot();

        tokio::spawn(async move {
          let result = receiver.recv().await;
          let _ = tx.send((result, receiver));
        });

        let Ok((result, returned)) = rx.await else { break };
        receiver = returned;

        match result {
          Ok(event) => {
            let is_new = matches!(event, ChannelEvent::New(_));

            let alive = list.update(cx, |list, cx| {
              match event {
                ChannelEvent::New(message) => list.append_message(cx, message),
                ChannelEvent::Updated(message) => list.update_message(cx, message),
                ChannelEvent::Deleted(id) => list.remove_message(cx, id),
              }
              cx.notify();
            });

            if alive.is_err() {
              break;
            }

            if is_new && let Some(typing) = &typing {
              typing.update(cx, |typing, cx| typing.message_arrived(cx)).unwrap_or_else(|_| log::debug!("view dropped before this update applied"));
            }
          }
          Err(RecvError::Lagged(n)) => log::warn!("dropped {n} live messages from channel stream"),
          Err(RecvError::Closed) => break,
        }
      }
    })
    .detach();

    let placeholder = format!("Message {}", options.title);
    let message_input = cx.new(|cx| InputState::new(window, cx).auto_grow(FIELD_MIN_ROWS, FIELD_MAX_ROWS).placeholder(placeholder));

    // In multi-line mode the input emits `PressEnter` only for the secondary
    // (cmd/ctrl) chord; plain Enter is intercepted in `render` before the
    // input can turn it into a newline. Treat the secondary chord as send too.
    cx.subscribe_in(&message_input, window, |this, _, event: &InputEvent, window, cx| {
      if let InputEvent::PressEnter { secondary: true } = event {
        this.send(window, cx);
      }
    })
    .detach();

    ChannelView {
      channel,
      list_view,
      message_input,
      options,
    }
  }

  /// Send whatever is in the field: optimistic append, then clear.
  fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let content = self.message_input.read(cx).value().to_string();
    let content = content.trim_end_matches('\n').to_string();

    if content.trim().is_empty() {
      return;
    }

    self.message_input.update(cx, |state, cx| state.set_value("", window, cx));

    let nonce = random_string::generate(20, random_string::charsets::ALPHANUMERIC);
    let pending = self.channel.send_message(content, nonce);

    self.list_view.update(cx, |list, cx| {
      list.append_message(cx, pending);
      cx.notify();
    });
  }

  fn insert_newline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.message_input.update(cx, |state, cx| state.insert("\n", window, cx));
  }

  fn render_composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let field =
      div().flex_1().min_w_0().bg(tokens::BG_FILL_TERTIARY).border_1().border_color(tokens::BORDER_SECONDARY).rounded(tokens::RADIUS_150).child(
        Input::new(&self.message_input)
          .appearance(false)
          .px(px(FIELD_PAD_X))
          .py(px(FIELD_PAD_Y))
          .text_size(tokens::TYPE_M)
          .line_height(px(FIELD_LINE_HEIGHT))
          .font_weight(FontWeight::MEDIUM)
          .text_color(tokens::TEXT),
      );

    v_flex()
      .key_context(COMPOSER_CONTEXT)
      .capture_action(cx.listener(|this, action: &Enter, window, cx| {
        if action.secondary {
          return;
        }

        cx.stop_propagation();
        this.send(window, cx);
      }))
      .on_action(cx.listener(|this, _: &InsertNewline, window, cx| this.insert_newline(window, cx)))
      .w_full()
      .flex_shrink_0()
      .pl(px(COMPOSER_PAD_LEFT))
      .pr(px(COMPOSER_PAD_RIGHT))
      .pb(px(COMPOSER_PAD_BOTTOM))
      .gap(px(TYPING_GAP))
      .child(typing_indicator(self.options.typing.as_ref().and_then(|t| t.read(cx).text())))
      .child(
        h_flex()
          .w_full()
          .items_end()
          .gap(px(BUTTONS_GAP))
          .child(h_flex().flex_1().min_w_0().items_end().gap(px(FIELD_GAP)).child(composer_avatar(self.options.composer_avatar.clone())).child(field))
          .child(composer_buttons()),
      )
  }
}

impl<C: Channel + 'static> Render for ChannelView<C> {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex()
      .size_full()
      .bg(tokens::BG_SECONDARY)
      .child(div().flex_1().min_h_0().w_full().child(self.list_view.clone()))
      .child(self.render_composer(cx))
  }
}

/// "zach is typing..." line above the field. The 18px is reserved even when
/// idle so the list does not jump.
fn typing_indicator(text: Option<SharedString>) -> impl IntoElement {
  div()
    .w_full()
    .h(px(TYPING_HEIGHT))
    .text_size(tokens::TYPE_S)
    .line_height(px(TYPING_HEIGHT))
    .font_weight(FontWeight::MEDIUM)
    .text_color(tokens::TEXT_TERTIARY)
    .whitespace_nowrap()
    .overflow_hidden()
    .children(text)
}

/// 32px avatar of the signed-in user, inset 2px so its centre lines up with the
/// 36px row avatars above it.
fn composer_avatar(url: Option<String>) -> impl IntoElement {
  let has_image = url.is_some();

  div()
    .flex_shrink_0()
    .ml(px(COMPOSER_AVATAR_INSET))
    .size(px(COMPOSER_AVATAR))
    .rounded_full()
    .bg(if has_image { tokens::BG_SECONDARY } else { tokens::BG_SURFACE_SECONDARY })
    .overflow_hidden()
    .children(url.map(|url| img(url).size_full().rounded_full()))
}

/// Segmented control on the right of the field: emoji | upload | GIF | sticker.
/// Widths 46/47/47/46 with 2px gaps of page background between them (192 total).
fn composer_buttons() -> impl IntoElement {
  let radius = tokens::RADIUS_150;

  h_flex()
    .flex_shrink_0()
    .h(px(COMPOSER_ROW_HEIGHT))
    .gap(px(2.))
    .child(
      composer_button(
        "composer-emoji",
        46.,
        "Emoji",
        Icon::new(ScopeIcon::Emoji).size(px(18.)).text_color(tokens::ICON),
      )
      .rounded_l(radius),
    )
    .child(composer_button(
      "composer-upload",
      47.,
      "Upload a file",
      Icon::new(ScopeIcon::Upload).size(px(18.)).text_color(tokens::TEXT_TERTIARY),
    ))
    .child(composer_button(
      "composer-gif",
      47.,
      "GIFs",
      div().text_size(tokens::TYPE_M).font_weight(FontWeight::EXTRA_BOLD).text_color(tokens::TEXT_TERTIARY).child("GIF"),
    ))
    .child(
      composer_button(
        "composer-sticker",
        46.,
        "Stickers",
        Icon::new(ScopeIcon::Heart).size(px(16.)).text_color(tokens::TEXT_TERTIARY),
      )
      .rounded_r(radius),
    )
}

fn composer_button(id: &'static str, width: f32, label: &'static str, content: impl IntoElement) -> Stateful<gpui::Div> {
  div()
    .id(id)
    .w(px(width))
    .h_full()
    .flex_shrink_0()
    .flex()
    .items_center()
    .justify_center()
    .bg(tokens::BG_SURFACE)
    .border_1()
    .border_color(tokens::BORDER_SECONDARY)
    .cursor_pointer()
    .hover(|style| style.bg(tokens::BG_FILL))
    .tooltip(move |window, cx| Tooltip::new(label).build(window, cx))
    .child(content)
}
