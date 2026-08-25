//! First-run screen: paste a Discord token (bot by default, or user) to connect.
//!
//! Mirrors the Figma "Scope Login" window (458x503): a dark card with a pink
//! glow along the top, the Scope mark, one masked token field and a
//! "Sign In" button. Coordinates below are the Figma frame coordinates.

use gpui::{
  App, BoxShadow, ClickEvent, Context, Entity, Focusable as _, FontWeight, Hsla, IntoElement, ParentElement, Render, SharedString, Styled, Window,
  div, linear_color_stop, linear_gradient, point, prelude::*, px,
};
use gpui_component::{
  ActiveTheme as _, Icon, h_flex,
  input::{Input, InputEvent, InputState},
  v_flex,
};

use scope_backend_discord::client::TokenKind;

use crate::{
  state::{AppState, Connection},
  theme::tokens,
};

const CARD_WIDTH: f32 = 458.;
const CARD_HEIGHT: f32 = 503.;
/// Left edge of every piece of content inside the card.
const CONTENT_X: f32 = 47.;
const CONTENT_WIDTH: f32 = 363.;

// The login surfaces are two steps darker than the `BG` token; Figma uses
// these literals and no token matches them.
const CARD_BG: Hsla = tokens::hex(0x0f1013);
const FIELD_BG: Hsla = tokens::hex(0x111319);
// Tints of the two blurred lobes in the design's background.
const VIOLET: Hsla = tokens::hex(0x7830dc);
const MAGENTA: Hsla = tokens::hex(0xb43cc8);
const WHITE: Hsla = tokens::BG_INVERSE;

pub struct Login {
  state: Entity<AppState>,
  token: Entity<InputState>,
  kind: TokenKind,
}

impl Login {
  pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
    cx.observe(&state, |_, _, cx| cx.notify()).detach();

    let token = cx.new(|cx| InputState::new(window, cx).placeholder("Paste your token").masked(true));

    cx.subscribe(&token, |this, _, event: &InputEvent, cx| match event {
      InputEvent::PressEnter { .. } => this.submit(cx),
      // The field label and border track focus.
      InputEvent::Focus | InputEvent::Blur => cx.notify(),
      InputEvent::Change => {}
    })
    .detach();

    // Land in the field straight away, like the focused state in the design.
    token.read(cx).focus_handle(cx).focus(window);

    Login {
      state,
      token,
      kind: TokenKind::Bot,
    }
  }

  fn submit(&mut self, cx: &mut Context<Self>) {
    let token = self.token.read(cx).value().trim().to_string();

    if token.is_empty() {
      return;
    }

    let kind = self.kind;
    self.state.update(cx, |state, cx| state.connect(token, kind, true, cx));
  }
}

impl Render for Login {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let (connecting, error) = match &self.state.read(cx).connection {
      Connection::Connecting => (true, None),
      Connection::Failed(error) => (false, Some(error.clone())),
      _ => (false, None),
    };
    let focused = self.token.read(cx).focus_handle(cx).is_focused(window);

    let card = div()
      .relative()
      .w(px(CARD_WIDTH))
      .h(px(CARD_HEIGHT))
      .flex_shrink_0()
      .overflow_hidden()
      .rounded(tokens::RADIUS_200)
      .bg(CARD_BG)
      .border_1()
      .border_color(tokens::BORDER)
      .shadow(vec![BoxShadow {
        color: gpui::black().opacity(0.45),
        offset: point(px(0.), px(8.)),
        blur_radius: px(20.),
        spread_radius: px(0.),
      }])
      .child(glow())
      .child(logo_row())
      .child(heading())
      .child(self.form(focused, error, cx))
      .child(footer(
        connecting,
        match self.state.read(cx).connecting_as.as_deref() {
          Some(name) => format!("Connecting as {name}…").into(),
          None => SharedString::new_static("Connecting…"),
        },
        cx.listener(|this, _, _, cx| this.submit(cx)),
      ));

    div().size_full().flex().items_center().justify_center().bg(cx.theme().background).child(card)
  }
}

impl Login {
  /// Label + 363x36 token field (+ error line) at (47,185).
  fn form(&self, focused: bool, error: Option<String>, cx: &mut Context<Self>) -> impl IntoElement {
    let field = div()
      .mt(px(4.))
      .w_full()
      .h(px(36.))
      .flex()
      .items_center()
      .rounded(tokens::RADIUS_150)
      .bg(FIELD_BG)
      .border_1()
      .border_color(if focused { tokens::BORDER_BRAND } else { tokens::BORDER })
      .when(focused, |this| {
        this.shadow(vec![BoxShadow {
          color: tokens::BORDER_BRAND.opacity(0.25),
          offset: point(px(0.), px(0.)),
          blur_radius: px(0.),
          spread_radius: px(3.),
        }])
      })
      .text_size(tokens::TYPE_M)
      .font_weight(FontWeight::MEDIUM)
      .text_color(tokens::TEXT)
      .child(Input::new(&self.token).appearance(false));

    v_flex()
      .absolute()
      .left(px(CONTENT_X))
      .top(px(185.))
      .w(px(CONTENT_WIDTH))
      .items_start()
      .child(
        div()
          .h(px(21.))
          .line_height(px(21.))
          .text_size(tokens::TYPE_M)
          .font_weight(FontWeight::MEDIUM)
          .text_color(if focused { tokens::TEXT_SECONDARY } else { tokens::TEXT_TERTIARY })
          .child("Your Discord token"),
      )
      .child(field)
      .child(self.kind_toggle(cx))
      .children(
        error.map(|error| div().mt(px(8.)).text_size(tokens::TYPE_S).line_height(tokens::TYPE_S_LINE).text_color(tokens::TEXT_DANGER).child(error)),
      )
  }

  /// Bot / user token switch under the field, with a one-line hint.
  fn kind_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let hint = match self.kind {
      TokenKind::Bot => "Bot tokens need the Presence, Server Members and Message Content intents enabled in the Developer Portal.",
      TokenKind::User => "User tokens use the unofficial client path. Use a throwaway account.",
    };

    v_flex()
      .mt(px(12.))
      .w_full()
      .gap(px(6.))
      .child(
        h_flex()
          .p(px(2.))
          .gap(px(2.))
          .rounded(tokens::RADIUS_200)
          .bg(FIELD_BG)
          .border_1()
          .border_color(tokens::BORDER)
          .child(kind_pill(
            "kind-bot",
            "Bot token",
            self.kind == TokenKind::Bot,
            cx.listener(|this, _, _, cx| this.set_kind(TokenKind::Bot, cx)),
          ))
          .child(kind_pill(
            "kind-user",
            "User token",
            self.kind == TokenKind::User,
            cx.listener(|this, _, _, cx| this.set_kind(TokenKind::User, cx)),
          )),
      )
      .child(div().text_size(tokens::TYPE_S).line_height(tokens::TYPE_S_LINE).text_color(tokens::TEXT_TERTIARY).child(hint))
  }

  fn set_kind(&mut self, kind: TokenKind, cx: &mut Context<Self>) {
    self.kind = kind;
    cx.notify();
  }
}

fn kind_pill(
  id: &'static str,
  label: &'static str,
  selected: bool,
  on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
  div()
    .id(id)
    .h(px(24.))
    .px(px(10.))
    .flex()
    .items_center()
    .rounded(tokens::RADIUS_150)
    .cursor_pointer()
    .text_size(tokens::TYPE_S)
    .font_weight(FontWeight::MEDIUM)
    .when(selected, |this| this.bg(tokens::BG_SURFACE).text_color(tokens::TEXT))
    .when(!selected, |this| {
      this
        .text_color(tokens::TEXT_TERTIARY)
        .hover(|this| this.bg(tokens::BG_SURFACE_SECONDARY).text_color(tokens::TEXT_SECONDARY))
        .active(|this| this.bg(tokens::BG_SURFACE).text_color(tokens::TEXT))
    })
    .on_click(on_click)
    .child(label)
}

/// Ambient glow behind the card content, approximating the design's gradient
/// rectangle plus blurred shape (sampled from the mockup): a pink band fading
/// out over the top 150px, and two soft violet lobes (top-left, right).
fn glow() -> impl IntoElement {
  div()
    .absolute()
    .inset_0()
    .child(div().absolute().top_0().left_0().w_full().h(px(150.)).bg(linear_gradient(
      180.,
      linear_color_stop(tokens::BRAND.opacity(0.11), 0.),
      linear_color_stop(tokens::BRAND.opacity(0.), 1.),
    )))
    .child(glow_lobe(point(px(30.), px(50.)), 280., VIOLET.opacity(0.08)))
    .child(glow_lobe(point(px(345.), px(100.)), 240., MAGENTA.opacity(0.10)))
}

/// Radial falloff (`login-glow.svg` is an alpha mask) centred on `center`.
fn glow_lobe(center: gpui::Point<gpui::Pixels>, radius: f32, color: Hsla) -> impl IntoElement {
  Icon::empty()
    .path("icons/scope/login-glow.svg")
    .absolute()
    .left(center.x - px(radius))
    .top(center.y - px(radius))
    .size(px(radius * 2.))
    .text_color(color)
}

/// 16px crosshair mark + "SCOPE S1" at (47,45).
fn logo_row() -> impl IntoElement {
  h_flex()
    .absolute()
    .left(px(CONTENT_X))
    .top(px(45.))
    .h(px(19.))
    .items_center()
    .gap(px(4.))
    .child(Icon::empty().path("icons/scope/login-mark.svg").size(px(16.)).text_color(tokens::BRAND))
    .child(div().text_size(tokens::TYPE_S).font_weight(FontWeight::BOLD).text_color(tokens::TEXT_TERTIARY).child("SCOPE S1"))
}

/// "Login to Scope", 36 Bold, glyphs spanning y 90..122.
fn heading() -> impl IntoElement {
  div()
    .absolute()
    .left(px(CONTENT_X))
    .top(px(77.))
    .text_size(tokens::HEADING_XL)
    .line_height(tokens::HEADING_XL_LINE)
    .font_weight(FontWeight::BOLD)
    .text_color(tokens::TEXT)
    .whitespace_nowrap()
    .child("Login to Scope")
}

/// "Not a member? Get Scope" left, "Sign In ->" button right, at (47,424) 363x36.
fn footer(connecting: bool, connecting_label: SharedString, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> impl IntoElement {
  h_flex()
    .absolute()
    .left(px(CONTENT_X))
    .top(px(424.))
    .w(px(CONTENT_WIDTH))
    .h(px(36.))
    .items_center()
    .justify_between()
    .child(
      h_flex()
        .gap(px(4.))
        .text_size(tokens::TYPE_M)
        .font_weight(FontWeight::MEDIUM)
        .child(div().text_color(tokens::TEXT_TERTIARY).child("Not a member?"))
        .child(
          div()
            .id("get-scope")
            .cursor_pointer()
            .text_color(tokens::BRAND)
            .hover(|this| this.text_color(tokens::BRAND_HOVER).underline())
            .active(|this| this.text_color(tokens::BRAND_ACTIVE))
            .on_click(|_, _, cx| cx.open_url("https://www.scopeclient.com/"))
            .child("Get Scope"),
        ),
    )
    .child(sign_in_button(connecting, connecting_label, on_click))
}

/// 89x36 brand button: `BG_FILL_BRAND` fill, 1px `BORDER_BRAND` rim, arrow glyph.
fn sign_in_button(
  connecting: bool,
  connecting_label: SharedString,
  on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
  h_flex()
    .id("sign-in")
    .h(px(36.))
    .pl(px(11.))
    .pr(px(13.))
    .gap(px(11.))
    .items_center()
    .rounded(tokens::RADIUS_150)
    .bg(tokens::BG_FILL_BRAND)
    .border_1()
    .border_color(tokens::BORDER_BRAND)
    .text_size(tokens::TYPE_M)
    .font_weight(FontWeight::BOLD)
    .text_color(WHITE)
    .when(connecting, |this| this.opacity(0.7).child(connecting_label))
    .when(!connecting, |this| {
      this
        .cursor_pointer()
        .hover(|this| this.bg(tokens::BRAND))
        .active(|this| this.bg(tokens::PINK_950))
        .on_click(on_click)
        .child("Sign In")
        .child(Icon::empty().path("icons/scope/login-arrow.svg").w(px(11.)).h(px(10.)).text_color(WHITE))
    })
}
