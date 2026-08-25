//! Second sidebar (242px): server banner, channel search, categories and channels.
//!
//! Geometry follows the Figma "Channel Nav" component (242x940): a 136.6px
//! banner, a 32px search box, 22px category headers and 26px channel rows on a
//! 27px pitch. Collapsed categories and the search text are local UI state.

use std::{
  collections::HashSet,
  f32::consts::{FRAC_PI_2, PI},
};

use gpui::{
  Context, Div, Entity, Focusable as _, FontWeight, Hsla, IntoElement, ObjectFit, ParentElement, Render, SharedString, Styled, StyledImage, Window,
  div, img, linear_color_stop, linear_gradient, prelude::*, px, radians,
};
use gpui_component::{
  Icon, h_flex,
  input::{Input, InputEvent, InputState},
  v_flex,
};
use scope_chat::nav::{ChannelInfo, ChannelKind, GuildInfo, Id};

use crate::{
  icons::ScopeIcon,
  shell::{CHANNEL_NAV_WIDTH, tabs::tooltip},
  state::AppState,
  theme::tokens,
};

const BANNER_HEIGHT: f32 = 136.6;
const CATEGORY_HEIGHT: f32 = 22.;
const ROW_HEIGHT: f32 = 26.;
/// Vertical gap between rows (27px pitch).
const ROW_GAP: f32 = 1.;

/// Banner "…" menu colour; the design uses `#e2e5ed`, which has no token.
const BANNER_MENU: Hsla = tokens::hex(0xe2e5ed);
/// Banner member count + glyph: white at 53%.
const MEMBERS_TEXT: Hsla = tokens::hexa(0xffffff87);
/// Banner active count: white at 78%.
const ACTIVE_TEXT: Hsla = tokens::hexa(0xffffffc7);
/// Edge darkening that stands in for Figma's `inset 0 0 26px rgba(0,0,0,.5)`.
const BANNER_SHADE: Hsla = tokens::hexa(0x00000045);
const BANNER_SHADE_CLEAR: Hsla = tokens::hexa(0x00000000);

/// Banner shown in demo mode, where guilds have no banner URL.
const DEMO_BANNER: &str = "brand/placeholder-banner.png";
const MEMBERS_ICON: &str = "icons/scope/channelnav-members.svg";
const VOLUME_ICON: &str = "icons/scope/channelnav-volume.svg";

pub struct ChannelNav {
  state: Entity<AppState>,
  search: Entity<InputState>,
  /// Lower-cased search text; empty shows every channel.
  query: String,
  /// Categories the user has collapsed.
  collapsed: HashSet<Id>,
}

impl ChannelNav {
  pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
    cx.observe(&state, |_, _, cx| cx.notify()).detach();

    // The placeholder is painted by us (see `search_box`) so it can use
    // `TEXT_MUTED` rather than the theme's `muted_foreground`.
    let search = cx.new(|cx| InputState::new(window, cx));

    cx.subscribe(&search, |this, input, event: &InputEvent, cx| match event {
      InputEvent::Change => {
        this.query = input.read(cx).value().trim().to_lowercase();
        cx.notify();
      }
      // The search box border tracks focus.
      InputEvent::Focus | InputEvent::Blur => cx.notify(),
      InputEvent::PressEnter { .. } => {}
    })
    .detach();

    ChannelNav {
      state,
      search,
      query: String::new(),
      collapsed: HashSet::new(),
    }
  }

  fn toggle_category(&mut self, id: Id, cx: &mut Context<Self>) {
    if !self.collapsed.remove(&id) {
      self.collapsed.insert(id);
    }
    cx.notify();
  }

  fn rows<'a>(&self, channels: &'a [ChannelInfo]) -> Vec<Row<'a>> {
    ordered_rows(channels, &self.query, &self.collapsed)
  }

  fn search_box(&self, focused: bool, show_placeholder: bool) -> impl IntoElement {
    // gpui sizes are border-box: 1px border + 5px padding puts the icon at x=6.
    let input = Input::new(&self.search).appearance(false);
    let input =
      Styled::h(input, px(30.)).flex_1().px(px(0.)).py(px(0.)).text_size(tokens::TYPE_S).font_weight(FontWeight::MEDIUM).text_color(tokens::TEXT);

    h_flex()
      .relative()
      .flex_none()
      .mt(px(12.4))
      .ml(px(12.))
      .w(px(219.))
      .h(px(32.))
      .rounded(tokens::RADIUS_150)
      .bg(tokens::BG_SECONDARY)
      .border_1()
      .border_color(if focused { tokens::BORDER_BRAND } else { tokens::BORDER })
      .pl(px(5.))
      .gap(px(8.))
      .items_center()
      .child(Icon::new(ScopeIcon::Search).size(px(16.)).text_color(tokens::ICON_SECONDARY))
      .child(input)
      .when(show_placeholder, |this| {
        this.child(
          div()
            .absolute()
            .left(px(29.))
            .top_0()
            .bottom_0()
            .flex()
            .items_center()
            .text_size(tokens::TYPE_S)
            .font_weight(FontWeight::MEDIUM)
            .text_color(tokens::TEXT_MUTED)
            .whitespace_nowrap()
            .child("find channel in server"),
        )
      })
  }

  fn category_row(&self, category: &ChannelInfo, first: bool, cx: &Context<Self>) -> impl IntoElement {
    let id = category.id;
    let collapsed = self.collapsed.contains(&id);
    // The asset points up; π points it down (expanded), π/2 points it right (collapsed).
    let rotation = if collapsed { FRAC_PI_2 } else { PI };

    h_flex()
      .id(("category", id.0))
      .flex_none()
      .h(px(CATEGORY_HEIGHT))
      .when(!first, |this| this.mt(px(8. - ROW_GAP)))
      .pl(px(13.))
      .gap(px(6.))
      .items_center()
      .cursor_pointer()
      .text_size(tokens::TYPE_S)
      .line_height(px(18.))
      .font_weight(FontWeight::BOLD)
      .text_color(tokens::TEXT_TERTIARY)
      .hover(|this| this.text_color(tokens::TEXT_SECONDARY))
      .active(|this| this.opacity(0.85))
      .whitespace_nowrap()
      .child(Icon::new(ScopeIcon::TriangleUp).w(px(6.)).h(px(5.417)).text_color(tokens::ICON_SECONDARY).rotate(radians(rotation)))
      .child(category.name.to_lowercase())
      .on_click(cx.listener(move |this, _, _, cx| this.toggle_category(id, cx)))
  }

  fn channel_row(&self, channel: &ChannelInfo, selected: bool, cx: &Context<Self>) -> impl IntoElement {
    let id = channel.id;
    let voice = matches!(channel.kind, ChannelKind::Voice | ChannelKind::Stage);

    let label_color = if selected {
      tokens::BG_INVERSE
    } else if channel.muted {
      tokens::TEXT_DISABLED
    } else if channel.unread > 0 {
      tokens::TEXT_SECONDARY
    } else {
      tokens::TEXT_TERTIARY
    };

    let icon = if voice {
      Icon::empty().path(VOLUME_ICON)
    } else {
      Icon::new(ScopeIcon::Hash)
    };

    h_flex()
      .id(("channel", id.0))
      .flex_none()
      .h(px(ROW_HEIGHT))
      .pl(px(18.))
      // Border-box: the panel's 1px right border plus 10px lands the badge's right edge at x=230.
      .pr(px(10.))
      .gap(px(8.))
      .items_center()
      .when(selected, |this| this.bg(tokens::BG_SURFACE))
      .when(!voice, |this| {
        this
          .cursor_pointer()
          .when(!selected, |this| {
            this.hover(|this| this.bg(tokens::BG_SURFACE_SECONDARY)).active(|this| this.bg(tokens::BG_SURFACE))
          })
          .on_click(cx.listener(move |this, _, window, cx| this.state.update(cx, |s, cx| s.open_channel(id, window, cx))))
      })
      // Voice channels can't be joined yet: no pointer, no hover, just a hint.
      .when(voice, |this| this.tooltip(tooltip("voice channels aren't supported yet")))
      .child(icon.size(px(16.)).text_color(tokens::TEXT_TERTIARY))
      .child(
        div()
          .flex_1()
          .min_w_0()
          .overflow_hidden()
          .whitespace_nowrap()
          .text_ellipsis()
          .text_size(tokens::TYPE_M)
          .line_height(px(20.))
          .font_weight(FontWeight::BOLD)
          .text_color(label_color)
          .child(channel.name.clone()),
      )
      .when(channel.unread > 0, |this| {
        this.child(
          div()
            .flex_none()
            .text_size(tokens::TYPE_S)
            .line_height(px(18.))
            .font_weight(FontWeight::BOLD)
            .text_color(tokens::TEXT_TERTIARY)
            .child(channel.unread.to_string()),
        )
      })
  }
}

enum Row<'a> {
  Category(&'a ChannelInfo),
  Channel(&'a ChannelInfo),
}

/// Rows in display order: uncategorised channels first, then every category
/// (by position) followed by its children (by position).
///
/// `query` is a lower-cased substring filter; while it is non-empty, empty
/// categories are dropped and collapsed ones are expanded so matches show.
fn ordered_rows<'a>(channels: &'a [ChannelInfo], query: &str, collapsed: &HashSet<Id>) -> Vec<Row<'a>> {
  let mut sorted: Vec<&ChannelInfo> = channels.iter().filter(|c| c.kind != ChannelKind::Thread).collect();
  sorted.sort_by_key(|c| (c.position, c.id));

  let is_category = |c: &ChannelInfo| c.kind == ChannelKind::Category;
  let category_ids: HashSet<Id> = sorted.iter().filter(|c| is_category(c)).map(|c| c.id).collect();
  let matches = |c: &ChannelInfo| query.is_empty() || c.name.to_lowercase().contains(query);
  let filtering = !query.is_empty();

  let mut rows: Vec<Row<'a>> = sorted
    .iter()
    .copied()
    .filter(|c| !is_category(c) && !c.parent_id.is_some_and(|p| category_ids.contains(&p)) && matches(c))
    .map(Row::Channel)
    .collect();

  for category in sorted.iter().copied().filter(|c| is_category(c)) {
    let children: Vec<&ChannelInfo> = sorted.iter().copied().filter(|c| c.parent_id == Some(category.id) && matches(c)).collect();

    if filtering && children.is_empty() {
      continue;
    }

    rows.push(Row::Category(category));

    if filtering || !collapsed.contains(&category.id) {
      rows.extend(children.into_iter().map(Row::Channel));
    }
  }

  rows
}

/// One edge of the banner's fake inner shadow. `angle` follows CSS
/// `linear-gradient`: 0 = towards the top, increasing clockwise.
fn shade(angle: f32) -> Div {
  div().absolute().bg(linear_gradient(
    angle,
    linear_color_stop(BANNER_SHADE, 0.),
    linear_color_stop(BANNER_SHADE_CLEAR, 1.),
  ))
}

fn banner(guild: Option<&GuildInfo>, demo: bool) -> impl IntoElement {
  let image: Option<SharedString> =
    guild.and_then(|g| g.banner_url.clone()).map(SharedString::from).or_else(|| demo.then(|| SharedString::new_static(DEMO_BANNER)));
  let name = guild.map(|g| g.name.clone()).unwrap_or_default();
  let members = guild.and_then(|g| g.member_count).map(|n| format!("{n} members"));
  let active = guild.and_then(|g| g.online_count).map(|n| format!("{n} active"));

  div()
    .relative()
    .flex_none()
    .w_full()
    .h(px(BANNER_HEIGHT))
    .overflow_hidden()
    .bg(tokens::BG_SURFACE_SECONDARY)
    .children(image.map(|src| img(src).absolute().inset_0().size_full().object_fit(ObjectFit::Cover)))
    .child(shade(180.).top_0().left_0().right_0().h(px(26.)))
    .child(shade(0.).bottom_0().left_0().right_0().h(px(26.)))
    .child(shade(90.).top_0().bottom_0().left_0().w(px(26.)))
    .child(shade(270.).top_0().bottom_0().right_0().w(px(26.)))
    .child(
      div()
        .absolute()
        .left(px(14.))
        .top(px(9.))
        .text_size(px(18.))
        .line_height(px(27.))
        .font_weight(FontWeight::EXTRA_BOLD)
        .text_color(tokens::TEXT)
        .whitespace_nowrap()
        .child(name),
    )
    .child(
      div()
        .id("server-menu")
        .absolute()
        .right(px(9.))
        .top(px(12.))
        .size(px(20.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(tokens::RADIUS_100)
        .cursor_pointer()
        .text_color(BANNER_MENU)
        .hover(|this| this.text_color(tokens::ICON_SELECTED))
        .active(|this| this.opacity(0.85))
        .tooltip(tooltip("Coming soon"))
        .child(Icon::new(ScopeIcon::Ellipsis).size(px(14.))),
    )
    .child(
      h_flex()
        .absolute()
        .left(px(13.))
        .bottom(px(11.))
        .h(px(12.5))
        .gap(px(6.))
        .items_center()
        .text_size(tokens::TYPE_S)
        .line_height(px(16.))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .child(Icon::empty().path(MEMBERS_ICON).w(px(10.)).h(px(12.5)).text_color(MEMBERS_TEXT))
        .children(members.map(|text| div().text_color(MEMBERS_TEXT).child(text)))
        .children(active.map(|text| div().text_color(ACTIVE_TEXT).child(text))),
    )
}

impl Render for ChannelNav {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let (guild, channels, active, demo) = {
      let state = self.state.read(cx);
      (
        state.selected_guild_info().cloned(),
        state.selected_guild_channels().to_vec(),
        state.active_channel(),
        state.is_demo(),
      )
    };

    let show_placeholder = self.search.read(cx).value().is_empty();
    let search_focused = self.search.read(cx).focus_handle(cx).is_focused(window);

    let rows: Vec<_> = self
      .rows(&channels)
      .into_iter()
      .enumerate()
      .map(|(index, row)| match row {
        Row::Category(category) => self.category_row(category, index == 0, cx).into_any_element(),
        Row::Channel(channel) => self.channel_row(channel, active == Some(channel.id), cx).into_any_element(),
      })
      .collect();

    v_flex()
      .id("channel-nav")
      .w(px(CHANNEL_NAV_WIDTH))
      .h_full()
      .flex_shrink_0()
      .bg(tokens::BG)
      .border_r_1()
      .border_color(tokens::BORDER)
      .overflow_y_scroll()
      .child(banner(guild.as_ref(), demo))
      .child(self.search_box(search_focused, show_placeholder))
      .child(v_flex().flex_none().w_full().mt(px(12.)).pb(px(12.)).gap(px(ROW_GAP)).children(rows))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn channel(id: u64, name: &str, kind: ChannelKind, parent: Option<u64>, position: i64) -> ChannelInfo {
    ChannelInfo {
      id: Id(id),
      guild_id: Some(Id(1)),
      name: name.into(),
      kind,
      parent_id: parent.map(Id),
      position,
      unread: 0,
      muted: false,
      icon_url: None,
    }
  }

  fn names(rows: &[Row<'_>]) -> Vec<String> {
    rows
      .iter()
      .map(|row| match row {
        Row::Category(c) => format!("[{}]", c.name),
        Row::Channel(c) => c.name.clone(),
      })
      .collect()
  }

  fn fixture() -> Vec<ChannelInfo> {
    vec![
      channel(20, "voice", ChannelKind::Category, None, 1),
      channel(10, "info", ChannelKind::Category, None, 0),
      channel(12, "rules", ChannelKind::Text, Some(10), 1),
      channel(11, "welcome", ChannelKind::Text, Some(10), 0),
      channel(21, "lounge", ChannelKind::Voice, Some(20), 0),
      channel(2, "general", ChannelKind::Text, None, 5),
      channel(1, "lobby", ChannelKind::Text, None, 0),
      channel(99, "a-thread", ChannelKind::Thread, Some(12), 0),
    ]
  }

  #[test]
  fn uncategorised_first_then_categories_by_position() {
    let channels = fixture();
    let rows = ordered_rows(&channels, "", &HashSet::new());
    assert_eq!(names(&rows), ["lobby", "general", "[info]", "welcome", "rules", "[voice]", "lounge"]);
  }

  #[test]
  fn collapsed_category_hides_children() {
    let channels = fixture();
    let rows = ordered_rows(&channels, "", &HashSet::from([Id(10)]));
    assert_eq!(names(&rows), ["lobby", "general", "[info]", "[voice]", "lounge"]);
  }

  #[test]
  fn query_filters_rows_and_expands_collapsed_categories() {
    let channels = fixture();
    let rows = ordered_rows(&channels, "l", &HashSet::from([Id(10)]));
    assert_eq!(names(&rows), ["lobby", "general", "[info]", "welcome", "rules", "[voice]", "lounge"]);

    let rows = ordered_rows(&channels, "rul", &HashSet::from([Id(10)]));
    assert_eq!(names(&rows), ["[info]", "rules"]);

    let rows = ordered_rows(&channels, "zzz", &HashSet::new());
    assert!(rows.is_empty());
  }
}
