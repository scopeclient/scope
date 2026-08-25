//! The main window layout: tab strip on top, then
//! `[main nav | channel nav | channel bar + chat | member list]`.

pub mod channel_bar;
pub mod channel_nav;
pub mod main_nav;
pub mod media_bar;
pub mod member_list;
pub mod tabs;

use gpui::{AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{h_flex, v_flex};

use crate::{
  shell::{channel_bar::ChannelBar, channel_nav::ChannelNav, main_nav::MainNav, media_bar::MediaBar, member_list::MemberList, tabs::TabsBar},
  state::AppState,
  theme::tokens,
};

pub const MAIN_NAV_WIDTH: f32 = 267.;
pub const CHANNEL_NAV_WIDTH: f32 = 242.;
pub const MEMBER_LIST_WIDTH: f32 = 242.;
pub const TABS_HEIGHT: f32 = 44.;
pub const CHANNEL_BAR_HEIGHT: f32 = 40.;

pub struct Shell {
  state: Entity<AppState>,
  tabs: Entity<TabsBar>,
  main_nav: Entity<MainNav>,
  channel_nav: Entity<ChannelNav>,
  channel_bar: Entity<ChannelBar>,
  member_list: Entity<MemberList>,
  media_bar: Entity<MediaBar>,
}

impl Shell {
  pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
    cx.observe(&state, |_, _, cx| cx.notify()).detach();

    Shell {
      tabs: cx.new(|cx| TabsBar::new(state.clone(), cx)),
      main_nav: cx.new(|cx| MainNav::new(state.clone(), cx)),
      channel_nav: cx.new(|cx| ChannelNav::new(state.clone(), window, cx)),
      channel_bar: cx.new(|cx| ChannelBar::new(state.clone(), cx)),
      member_list: cx.new(|cx| MemberList::new(state.clone(), cx)),
      media_bar: cx.new(|cx| MediaBar::new(state.clone(), cx)),
      state,
    }
  }
}

impl Render for Shell {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let state = self.state.read(cx);

    let chat = match state.active_channel_view() {
      Some(view) => div().size_full().child(view).into_any_element(),
      None => empty_state(state.active_channel().is_some()).into_any_element(),
    };

    let show_channel_nav = state.show_channel_nav && state.selected_guild.is_some();
    let show_member_list = state.show_member_list && state.selected_guild.is_some();

    // Mockup layout: the tab strip spans the window; the channel bar spans
    // everything right of the main nav; below it sit channel nav | chat | members.
    let notice = state.notice.clone().map(|text| {
      h_flex()
        .w_full()
        .px(px(16.))
        .py(px(6.))
        .gap(px(12.))
        .items_center()
        .bg(tokens::BG_SURFACE)
        .border_b_1()
        .border_color(tokens::BORDER)
        .text_size(tokens::TYPE_S)
        .text_color(tokens::TEXT_WARNING)
        .child(div().flex_1().min_w_0().child(text))
        .child(
          div().id("dismiss-notice").cursor_pointer().text_color(tokens::TEXT_TERTIARY).child("dismiss").on_click(cx.listener(|this, _, _, cx| {
            this.state.update(cx, |s, cx| {
              s.notice = None;
              cx.notify();
            })
          })),
        )
    });

    let right = v_flex().flex_1().min_w_0().h_full().child(self.channel_bar.clone()).children(notice).child(self.media_bar.clone()).child(
      h_flex()
        .flex_1()
        .min_h_0()
        .w_full()
        .when(show_channel_nav, |this| this.child(self.channel_nav.clone()))
        .child(div().flex_1().min_w_0().h_full().bg(tokens::BG_SECONDARY).child(chat))
        .when(show_member_list, |this| this.child(self.member_list.clone())),
    );

    v_flex()
      .size_full()
      .bg(tokens::BG)
      .text_color(tokens::TEXT)
      .child(self.tabs.clone())
      .child(h_flex().flex_1().min_h_0().w_full().child(self.main_nav.clone()).child(right))
  }
}

fn empty_state(loading: bool) -> impl IntoElement {
  v_flex().size_full().items_center().justify_center().gap_2().text_color(tokens::TEXT_TERTIARY).child(if loading {
    "loading channel…"
  } else {
    "pick a channel to start reading"
  })
}
