//! Far-left sidebar (267px): user header, quick nav, channel folders, server
//! list and the voice-call card. Mirrors the Figma `Main-Nav Component`
//! (440:9155): 30px rows, tree rows on a 32px pitch, voice card pinned to the
//! bottom.

use std::f32::consts::PI;

use gpui::{
  div, img, prelude::*, px, radians, AnyElement, Context, Div, Entity, FontWeight, Hsla, IntoElement, ObjectFit, ParentElement, Render, Stateful,
  Styled, Window,
};
use gpui_component::{h_flex, v_flex, Icon};
use scope_chat::nav::{GuildInfo, Presence, UserInfo};

use crate::{
  icons::ScopeIcon,
  shell::{tabs::tooltip, MAIN_NAV_WIDTH},
  state::AppState,
  theme::tokens,
};

/// Height of every nav / tree row.
const ROW_HEIGHT: f32 = 30.;
/// Gap between tree rows (sections, folders, server-channels) -> 32px pitch.
const ROW_GAP: f32 = 2.;
/// Horizontal inset of the server-channel selection pill (x=12..255).
const PILL_INSET: f32 = 12.;

/// Voice-card border and divider: `#313342` has no design token (spec §3.8.3).
const CARD_BORDER: Hsla = tokens::hex(0x313342);

// Scope-specific glyphs exported for this panel (`assets/icons/scope/mainnav-*`).
const ICON_TRAY: &str = "icons/scope/mainnav-tray.svg";
const ICON_BELL: &str = "icons/scope/mainnav-bell.svg";
const ICON_COLUMNS: &str = "icons/scope/mainnav-columns.svg";
const ICON_COMPASS: &str = "icons/scope/mainnav-compass.svg";
const ICON_COMMAND: &str = "icons/scope/mainnav-command.svg";
const ICON_CHEVRONS: &str = "icons/scope/mainnav-chevrons.svg";
const ICON_STAR: &str = "icons/scope/mainnav-star.svg";
const ICON_PLUS: &str = "icons/scope/mainnav-plus.svg";
const ICON_HAMBURGER: &str = "icons/scope/mainnav-hamburger.svg";
const ICON_FOLDER: &str = "icons/scope/mainnav-folder.svg";
const ICON_VOLUME: &str = "icons/scope/mainnav-volume.svg";
const ICON_SIGNAL: &str = "icons/scope/mainnav-signal.svg";
const ICON_MIC: &str = "icons/scope/mainnav-mic.svg";
const ICON_HEADPHONES: &str = "icons/scope/mainnav-headphones.svg";
const ICON_PHONE_BLOCK: &str = "icons/scope/mainnav-phone-block.svg";

pub struct MainNav {
  state: Entity<AppState>,
}

impl MainNav {
  pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
    cx.observe(&state, |_, _, cx| cx.notify()).detach();
    MainNav { state }
  }
}

impl Render for MainNav {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let state = self.state.read(cx);
    let demo = state.is_demo();
    let selected = state.selected_guild;

    // Messages badge = DMs with unread messages. There is no notification
    // model yet; demo mode shows the mockup's sample counts instead.
    let unread_dms = state.dms.iter().filter(|dm| dm.unread > 0).count() as u32;
    let (messages_badge, notifications_badge) = if demo { (5, 8) } else { (unread_dms, 0) };

    let nav = v_flex()
      .mt(px(12.))
      .w_full()
      .child(nav_row(0, Icon::new(ScopeIcon::SearchBold).size(px(18.)), "Find", 0, "F"))
      .child(nav_row(1, icon(ICON_TRAY).size(px(14.)), "Messages", messages_badge, "I"))
      .child(nav_row(2, icon(ICON_BELL).size(px(16.)), "Notifications", notifications_badge, "N"))
      .child(nav_row(3, icon(ICON_COLUMNS).size(px(16.)), "Columns", 0, "C"))
      .child(nav_row(4, icon(ICON_COMPASS).size(px(16.)), "Explore", 0, "E"));

    // Favourites are not modelled yet: demo mode reproduces the mockup's
    // sample folder ("member importants" -> three `#member-important` rows).
    let mut favourites = v_flex().mt(px(12.)).w_full().gap(px(ROW_GAP)).child(section_row(0, icon(ICON_STAR), "Channel Folders"));
    if demo {
      favourites =
        favourites.child(folder_row(0, "member importants")).children([0usize, 1, 4].into_iter().enumerate().map(|(index, guild_index)| {
          let guild = state.guilds.get(guild_index);
          let tile = icon_tile(guild.and_then(|g| g.icon_url.as_deref()), guild.map_or("#", |g| g.name.as_str()));
          channel_row(("favourite", index), tile, "#member-important".into(), 5, index == 0)
        }));
    }

    let mut servers = v_flex().mt(px(16.)).w_full().gap(px(ROW_GAP)).child(section_row(1, icon(ICON_HAMBURGER), "All servers"));
    if demo {
      servers = servers.child(folder_row(1, "bots"));
    }
    servers = servers.children(state.guilds.iter().enumerate().map(|(index, guild)| {
      let id = guild.id;
      let tile = icon_tile(guild.icon_url.as_deref(), &guild.name);

      channel_row(("guild", index), tile, guild.name.clone(), guild.unread, selected == Some(id))
        .on_click(cx.listener(move |this, _, _, cx| this.state.update(cx, |s, cx| s.select_guild(id, cx))))
    }));

    let tree = v_flex()
      .id("main-nav-scroll")
      .flex_1()
      .min_h_0()
      .w_full()
      .overflow_y_scroll()
      .children(state.user.as_ref().map(header))
      .child(nav)
      .child(favourites)
      .child(servers)
      .child(div().h(px(12.)).flex_shrink_0());

    v_flex()
      .id("main-nav")
      .w(px(MAIN_NAV_WIDTH))
      .h_full()
      .flex_shrink_0()
      .bg(tokens::BG)
      .border_r_1()
      .border_color(tokens::BORDER)
      .child(tree)
      // Voice is not supported yet; the card is demo-only mockup content.
      .when(demo, |this| this.child(voice_card(state.guilds.first())))
  }
}

fn icon(path: &'static str) -> Icon {
  Icon::empty().path(path)
}

fn text(size: gpui::Pixels, weight: FontWeight, color: Hsla) -> Div {
  div().text_size(size).font_weight(weight).text_color(color).whitespace_nowrap()
}

/// Single-line ellipsis truncation. Unlike `truncate()`, white-space stays
/// `normal`: gpui caches the first (intrinsic, untruncated) text layout when
/// `nowrap` is set and never re-truncates for the final width.
fn ellipsis(element: Div) -> Div {
  element.whitespace_normal().overflow_hidden().text_ellipsis().line_clamp(1)
}

fn initial(name: &str) -> String {
  name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().to_string()).unwrap_or_default()
}

fn presence_color(presence: Presence) -> Hsla {
  match presence {
    Presence::Online => tokens::ICON_SUCCESS,
    Presence::Idle => tokens::ICON_WARNING,
    Presence::DoNotDisturb => tokens::ICON_DANGER,
    Presence::Offline => tokens::TEXT_MUTED,
  }
}

fn presence_label(presence: Presence) -> &'static str {
  match presence {
    Presence::Online => "Online",
    Presence::Idle => "Idle",
    Presence::DoNotDisturb => "Do Not Disturb",
    Presence::Offline => "Offline",
  }
}

/// 36px avatar ring (`bg-secondary`) around a 33px circle-cropped picture,
/// falling back to the user's initial.
fn avatar(user: &UserInfo) -> Div {
  let ring = div().size(px(36.)).flex_shrink_0().rounded_full().bg(tokens::BG_SECONDARY).flex().items_center().justify_center();

  match &user.avatar_url {
    Some(url) => ring.child(img(url.clone()).size(px(33.)).rounded_full().object_fit(ObjectFit::Cover)),
    None => ring.child(text(tokens::TYPE_M, FontWeight::BOLD, tokens::TEXT_SECONDARY).child(initial(&user.display_name))),
  }
}

/// User header at (13, 11): avatar, `user#0001` + up/down chevron, presence dot + status.
fn header(user: &UserInfo) -> impl IntoElement {
  let status = user.status_text.clone().unwrap_or_else(|| presence_label(user.presence).into());

  h_flex().ml(px(13.)).mt(px(11.)).h(px(36.)).items_start().child(avatar(user)).child(
    v_flex()
      .ml(px(8.))
      .child(
        h_flex()
          .h(px(21.))
          .items_center()
          .gap(px(8.))
          .child(text(tokens::TYPE_M, FontWeight::BOLD, tokens::TEXT).child(user.tag.clone()))
          .child(icon(ICON_CHEVRONS).w(px(7.)).h(px(10.)).text_color(tokens::ICON_SECONDARY)),
      )
      .child(
        h_flex()
          .mt(px(-3.))
          .h(px(18.))
          .items_center()
          .gap(px(6.))
          .child(div().size(px(4.)).rounded_full().bg(presence_color(user.presence)))
          .child(text(tokens::TYPE_S, FontWeight::MEDIUM, tokens::TEXT_SECONDARY).child(status)),
      ),
  )
}

/// Quick-nav row: icon centred at x=26, label at x=48, optional count 8px
/// after the label, `⌘<key>` hint left-anchored at x=220.
fn nav_row(index: usize, glyph: Icon, label: &'static str, badge: u32, key: &'static str) -> impl IntoElement {
  h_flex()
    .id(("nav", index))
    .relative()
    .w_full()
    .h(px(ROW_HEIGHT))
    .items_center()
    .cursor_pointer()
    .hover(|this| this.bg(tokens::BG_SURFACE_SECONDARY))
    .active(|this| this.bg(tokens::BG_SURFACE))
    .tooltip(tooltip("Coming soon"))
    .child(div().ml(px(17.)).size(px(18.)).flex_shrink_0().flex().items_center().justify_center().child(glyph.text_color(tokens::ICON)))
    .child(text(tokens::TYPE_M, FontWeight::MEDIUM, tokens::TEXT_SECONDARY).ml(px(13.)).child(label))
    .when(badge > 0, |this| {
      this.child(text(tokens::TYPE_S, FontWeight::MEDIUM, tokens::TEXT_TERTIARY).ml(px(8.)).child(badge.to_string()))
    })
    .child(
      h_flex()
        .absolute()
        .left(px(220.))
        .top(px(0.))
        .h_full()
        .items_center()
        .gap(px(2.))
        .child(icon(ICON_COMMAND).size(px(16.)).text_color(tokens::ICON_SECONDARY))
        .child(text(tokens::TYPE_M, FontWeight::MEDIUM, tokens::ICON_SECONDARY).child(key)),
    )
}

/// Small disclosure triangle pointing down (expanded).
fn disclosure() -> Icon {
  Icon::new(ScopeIcon::TriangleUp).size(px(6.)).rotate(radians(PI)).text_color(tokens::ICON_SECONDARY)
}

/// Section header ("Channel Folders", "All servers"): chevron x=20, icon x=32,
/// label x=56, "+" right-aligned so its glyph keeps a 17px margin.
fn section_row(index: usize, glyph: Icon, label: &'static str) -> impl IntoElement {
  h_flex()
    .w_full()
    .h(px(ROW_HEIGHT))
    .items_center()
    .child(div().ml(px(20.)).w(px(6.)).flex_shrink_0().flex().items_center().child(disclosure()))
    .child(div().ml(px(6.)).size(px(16.)).flex_shrink_0().flex().items_center().justify_center().child(glyph.text_color(tokens::ICON_SECONDARY)))
    .child(text(tokens::TYPE_M, FontWeight::MEDIUM, tokens::TEXT_TERTIARY).ml(px(8.)).child(label))
    .child(div().flex_1())
    .child(
      div()
        .id(("section-add", index))
        .mr(px(15.))
        .size(px(20.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(tokens::RADIUS_100)
        .cursor_pointer()
        .text_color(tokens::ICON_SECONDARY)
        .hover(|this| this.bg(tokens::BG_FILL).text_color(tokens::ICON_HOVER))
        .active(|this| this.opacity(0.85))
        .tooltip(tooltip("Coming soon"))
        .child(icon(ICON_PLUS).size(px(16.))),
    )
}

/// Folder row: chevron x=28, folder icon x=42, label x=64, hover pill x=12..255.
fn folder_row(index: usize, label: &'static str) -> impl IntoElement {
  h_flex()
    .id(("folder", index))
    .mx(px(PILL_INSET))
    .w(px(MAIN_NAV_WIDTH - 2. * PILL_INSET))
    .h(px(ROW_HEIGHT))
    .pl(px(28. - PILL_INSET))
    .items_center()
    .rounded(tokens::RADIUS_150)
    .cursor_pointer()
    .hover(|this| this.bg(tokens::BG_SURFACE_SECONDARY))
    .active(|this| this.bg(tokens::BG_SURFACE))
    .child(div().w(px(6.)).flex_shrink_0().flex().items_center().child(disclosure()))
    .child(icon(ICON_FOLDER).size(px(14.)).ml(px(8.)).text_color(tokens::ICON_SECONDARY))
    .child(text(tokens::TYPE_M, FontWeight::MEDIUM, tokens::TEXT_SECONDARY).ml(px(8.)).child(label))
}

/// 18px rounded server icon; falls back to a surface tile with the initial.
fn icon_tile(url: Option<&str>, name: &str) -> AnyElement {
  let tile = div().size(px(18.)).flex_shrink_0().rounded(tokens::RADIUS_100).overflow_hidden();

  match url {
    Some(url) => tile.child(img(url.to_owned()).size_full().rounded(tokens::RADIUS_100).object_fit(ObjectFit::Cover)).into_any_element(),
    None => tile
      .bg(tokens::BG_SURFACE)
      .flex()
      .items_center()
      .justify_center()
      .child(text(tokens::TYPE_XS, FontWeight::BOLD, tokens::TEXT_SECONDARY).child(initial(name)))
      .into_any_element(),
  }
}

/// Server-channel row: selection pill x=12..255 r6, icon x=47, label x=75,
/// badge right-aligned to x=243.
fn channel_row(id: impl Into<gpui::ElementId>, tile: AnyElement, label: String, badge: u32, selected: bool) -> Stateful<Div> {
  let (label_weight, label_color, badge_color) = if selected {
    (FontWeight::BOLD, tokens::TEXT, tokens::ICON_TERTIARY)
  } else {
    (FontWeight::MEDIUM, tokens::TEXT_SECONDARY, tokens::TEXT_TERTIARY)
  };

  h_flex()
    .id(id)
    .mx(px(PILL_INSET))
    .w(px(MAIN_NAV_WIDTH - 2. * PILL_INSET))
    .h(px(ROW_HEIGHT))
    .pl(px(47. - PILL_INSET))
    .pr(px(PILL_INSET))
    .items_center()
    .rounded(tokens::RADIUS_150)
    .cursor_pointer()
    .when(selected, |this| this.bg(tokens::BG_SURFACE))
    .when(!selected, |this| {
      this.hover(|this| this.bg(tokens::BG_SURFACE_SECONDARY)).active(|this| this.bg(tokens::BG_SURFACE))
    })
    .child(tile)
    .child(ellipsis(text(tokens::TYPE_M, label_weight, label_color).ml(px(10.)).flex_1().min_w_0()).child(label))
    .when(badge > 0, |this| {
      this.child(text(tokens::TYPE_S, FontWeight::MEDIUM, badge_color).ml(px(8.)).child(badge.to_string()))
    })
}

/// 28x28 voice-card icon button around a 22px glyph. The glyph inherits the
/// wrapper's colour so the hover colour reaches it.
fn voice_button(id: &'static str, glyph: Icon, color: Hsla, hover: Hsla, tip: &'static str) -> Stateful<Div> {
  div()
    .id(id)
    .size(px(28.))
    .flex_shrink_0()
    .flex()
    .items_center()
    .justify_center()
    .rounded(tokens::RADIUS_100)
    .cursor_pointer()
    .text_color(color)
    .hover(move |this| this.bg(tokens::BG_FILL).text_color(hover))
    .active(|this| this.opacity(0.85))
    .tooltip(tooltip(tip))
    .child(glyph.size(px(22.)))
}

/// Voice call card (245x87, 11px margins) pinned to the bottom. Static mockup
/// content until voice is supported.
fn voice_card(guild: Option<&GuildInfo>) -> impl IntoElement {
  let tile = icon_tile(guild.and_then(|g| g.icon_url.as_deref()), guild.map_or("?", |g| g.name.as_str()));

  let upper = h_flex()
    .h(px(40.))
    .w_full()
    .flex_shrink_0()
    .items_center()
    .bg(tokens::BG_SURFACE)
    .border_b_1()
    .border_color(CARD_BORDER)
    .child(div().ml(px(11.)).flex_shrink_0().child(tile))
    .child(icon(ICON_VOLUME).size(px(16.)).ml(px(4.)).text_color(tokens::ICON_SECONDARY))
    .child(ellipsis(text(tokens::TYPE_M, FontWeight::BOLD, tokens::TEXT).ml(px(4.)).w(px(114.))).child("important-friends"))
    .child(icon(ICON_SIGNAL).size(px(16.)).ml(px(12.)).text_color(tokens::ICON_SUCCESS))
    .child(text(tokens::TYPE_M, FontWeight::MEDIUM, tokens::TEXT_SUCCESS).ml(px(4.)).mr(px(11.)).child("12ms"));

  // 28px hit boxes centred on the 22px glyphs keep the icons at x=11/14/13.
  let lower = h_flex()
    .flex_1()
    .w_full()
    .items_center()
    .bg(tokens::BG_SECONDARY)
    .child(voice_button("voice-mic", icon(ICON_MIC), tokens::ICON, tokens::ICON_HOVER, "Mute").ml(px(8.)))
    .child(voice_button("voice-deafen", icon(ICON_HEADPHONES), tokens::ICON, tokens::ICON_HOVER, "Deafen").ml(px(8.)))
    .child(div().flex_1())
    .child(text(tokens::TYPE_M, FontWeight::MEDIUM, tokens::TEXT_TERTIARY).child("2:40"))
    .child(
      voice_button(
        "voice-hang-up",
        icon(ICON_PHONE_BLOCK),
        tokens::ICON_DANGER,
        tokens::ICON_DANGER,
        "Disconnect",
      )
      .ml(px(10.))
      .mr(px(11.)),
    );

  v_flex()
    .mx(px(11.))
    .mb(px(11.))
    .w(px(MAIN_NAV_WIDTH - 22.))
    .h(px(87.))
    .flex_shrink_0()
    .rounded(tokens::RADIUS_200)
    .border_1()
    .border_color(CARD_BORDER)
    .overflow_hidden()
    .child(upper)
    .child(lower)
}
