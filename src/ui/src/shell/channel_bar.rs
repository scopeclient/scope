//! Strip under the tabs (40px): sidebar toggle, centred breadcrumb, channel actions.

use std::f32::consts::{FRAC_PI_2, PI};

use gpui::{
  Context, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, Window, div,
  prelude::*, px, radians,
};
use gpui_component::{Icon, h_flex};

use crate::{
  icons::ScopeIcon,
  shell::{
    CHANNEL_BAR_HEIGHT,
    tabs::{server_tile, tooltip},
  },
  state::AppState,
  theme::tokens,
};

/// Size of the action icons on the right.
const ACTION_ICON: Pixels = px(18.);

pub struct ChannelBar {
  state: Entity<AppState>,
}

impl ChannelBar {
  pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
    cx.observe(&state, |_, _, cx| cx.notify()).detach();
    ChannelBar { state }
  }
}

/// 18x18 icon button: `ICON_SECONDARY`, `ICON_HOVER` on hover (or `color` when
/// given: it marks a toggled-on control, so hovering keeps it at `ICON_SELECTED`).
fn action_button(id: &'static str, icon: Icon, tip: &'static str, color: Option<Hsla>) -> gpui::Stateful<gpui::Div> {
  let hover = if color.is_some() { tokens::ICON_SELECTED } else { tokens::ICON_HOVER };

  div()
    .id(id)
    .relative()
    .size(ACTION_ICON)
    .flex_shrink_0()
    .flex()
    .items_center()
    .justify_center()
    .cursor_pointer()
    .text_color(color.unwrap_or(tokens::ICON_SECONDARY))
    .hover(move |style| style.text_color(hover))
    .active(|style| style.opacity(0.8))
    .tooltip(tooltip(tip))
    .child(icon)
}

impl Render for ChannelBar {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let state = self.state.read(cx);
    let tab = state.active_tab();
    let guild_name = tab.and_then(|t| t.guild).and_then(|g| state.guild(g)).map(|g| g.name.clone());

    // Left: collapse triangle (points left while the channel nav is open) + view-list glyph.
    let triangle = if state.show_channel_nav { -FRAC_PI_2 } else { FRAC_PI_2 };
    let toggle = h_flex()
      .id("toggle-channel-nav")
      .ml(px(10.))
      .h_full()
      .flex_shrink_0()
      .items_center()
      .gap(px(2.))
      .cursor_pointer()
      .text_color(tokens::ICON_SECONDARY)
      .hover(|style| style.text_color(tokens::ICON_HOVER))
      .active(|style| style.opacity(0.85))
      .tooltip(tooltip(if state.show_channel_nav { "hide channels" } else { "show channels" }))
      .child(
        div()
          .size(px(6.))
          .flex()
          .items_center()
          .justify_center()
          .child(Icon::new(ScopeIcon::TriangleUp).w(px(6.)).h(px(5.417)).rotate(radians(triangle))),
      )
      .child(Icon::new(ScopeIcon::PanelToggle).size(px(18.)))
      .on_click(cx.listener(|this, _, _, cx| this.state.update(cx, |s, cx| s.toggle_channel_nav(cx))));

    // Centre: [logo 18][8][guild][6][/][6][#channel][10][chevron 6], centred on the whole bar.
    let breadcrumb = tab.map(|tab| {
      h_flex()
        .items_center()
        .text_size(tokens::TYPE_M)
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .child(server_tile(
          state,
          tab.guild,
          tab.icon_url.as_deref(),
          guild_name.as_deref().unwrap_or(&tab.title),
        ))
        .when_some(guild_name.clone(), |this, name| {
          this
            .child(div().ml(px(8.)).text_color(tokens::TEXT_TERTIARY).child(name))
            .child(div().ml(px(6.)).text_color(tokens::TEXT_TERTIARY).child("/"))
        })
        .child(div().ml(if guild_name.is_some() { px(6.) } else { px(8.) }).text_color(tokens::TEXT).child(tab.title.clone()))
        .child(
          div()
            .id("channel-switcher")
            .ml(px(5.))
            .mr(px(-5.))
            .size(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(tokens::RADIUS_100)
            .cursor_pointer()
            .text_color(tokens::ICON_SECONDARY)
            .hover(|style| style.text_color(tokens::ICON_HOVER))
            .active(|style| style.opacity(0.85))
            .tooltip(tooltip("Coming soon"))
            .child(Icon::new(ScopeIcon::TriangleUp).w(px(6.)).h(px(5.)).rotate(radians(PI))),
        )
    });

    let center = h_flex().absolute().inset_0().items_center().justify_center().children(breadcrumb);

    // Right: four 18px icons, 12px apart, 11px from the edge; unread dot on the pin.
    let pin = action_button("pinned-messages", Icon::new(ScopeIcon::Pin).size(ACTION_ICON), "pinned messages", None)
      .child(div().absolute().top_0().right_0().size(px(4.)).rounded_full().bg(tokens::BORDER_BRAND));

    let members = action_button(
      "toggle-member-list",
      Icon::new(ScopeIcon::Member).w(px(10.)).h(px(12.5)),
      if state.show_member_list {
        "hide member list"
      } else {
        "show member list"
      },
      state.show_member_list.then_some(tokens::ICON_SELECTED),
    )
    .on_click(cx.listener(|this, _, _, cx| this.state.update(cx, |s, cx| s.toggle_member_list(cx))));

    let actions = h_flex()
      .mr(px(11.))
      .h_full()
      .flex_shrink_0()
      .items_center()
      .gap(px(12.))
      .child(action_button(
        "open-popout",
        Icon::new(ScopeIcon::ArrowOut).size(ACTION_ICON),
        "open in popout",
        None,
      ))
      .child(pin)
      .child(action_button(
        "search",
        Icon::new(ScopeIcon::SearchBold).size(ACTION_ICON),
        "search",
        None,
      ))
      .child(members);

    h_flex()
      .id("channel-bar")
      .relative()
      .w_full()
      .h(px(CHANNEL_BAR_HEIGHT))
      .flex_shrink_0()
      .items_center()
      .justify_between()
      .bg(tokens::BG_SECONDARY)
      .border_b_1()
      .border_color(tokens::BORDER)
      .child(center)
      .child(toggle)
      .child(actions)
  }
}
