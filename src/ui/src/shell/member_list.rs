//! Right sidebar (242px): members of the selected server, grouped by hoisted
//! role, each with avatar, presence dot, display name and status line.

use std::{
  collections::HashSet,
  f32::consts::{FRAC_PI_2, PI},
};

use gpui::{
  App, ClickEvent, Context, Entity, FontWeight, Hsla, IntoElement, ObjectFit, ParentElement, Render, Styled, Window, div, img, prelude::*, px,
  radians,
};
use gpui_component::{Icon, h_flex, v_flex};
use scope_chat::nav::{MemberInfo, Presence};

use crate::{icons::ScopeIcon, shell::MEMBER_LIST_WIDTH, state::AppState, theme::tokens};

/// Group label for members without a hoisted role.
const DEFAULT_GROUP: &str = "MEMBERS";

const ROW_HEIGHT: f32 = 38.;
const HEADER_HEIGHT: f32 = 22.;
const AVATAR_SIZE: f32 = 28.;
const DOT_SIZE: f32 = 10.;
/// Content starts 13px from the panel's outer edge; 1px of that is the divider.
const EDGE_PADDING: f32 = 13.;
const LEFT_PADDING: f32 = EDGE_PADDING - 1.;

pub struct MemberList {
  state: Entity<AppState>,
  /// Group labels the user has collapsed. Purely local UI state.
  collapsed: HashSet<String>,
}

impl MemberList {
  pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
    cx.observe(&state, |_, _, cx| cx.notify()).detach();
    MemberList {
      state,
      collapsed: HashSet::new(),
    }
  }

  fn toggle_group(&mut self, group: &str, cx: &mut Context<Self>) {
    if !self.collapsed.remove(group) {
      self.collapsed.insert(group.to_owned());
    }
    cx.notify();
  }
}

pub fn presence_color(presence: Presence) -> Hsla {
  match presence {
    Presence::Online => tokens::ICON_SUCCESS,
    Presence::Idle => tokens::ICON_WARNING,
    Presence::DoNotDisturb => tokens::ICON_DANGER,
    Presence::Offline => tokens::TEXT_MUTED,
  }
}

fn group_of(member: &MemberInfo) -> &str {
  member.role_group.as_deref().unwrap_or(DEFAULT_GROUP)
}

/// Bucket members by role group, keeping groups in order of first appearance
/// (the backend already sorts members by hoisted role, then name).
fn grouped(members: &[MemberInfo]) -> Vec<(&str, Vec<&MemberInfo>)> {
  let mut groups: Vec<(&str, Vec<&MemberInfo>)> = Vec::new();

  for member in members {
    let name = group_of(member);

    match groups.iter_mut().find(|(group, _)| *group == name) {
      Some((_, list)) => list.push(member),
      None => groups.push((name, vec![member])),
    }
  }

  groups
}

impl Render for MemberList {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let groups = grouped(self.state.read(cx).selected_guild_members());

    let mut list = v_flex()
      .id("member-list")
      .w(px(MEMBER_LIST_WIDTH))
      .h_full()
      .flex_shrink_0()
      .bg(tokens::BG)
      .border_l_1()
      .border_color(tokens::BORDER)
      .overflow_y_scroll()
      .pt(px(7.));

    let mut row_index = 0;

    for (group_index, (name, members)) in groups.into_iter().enumerate() {
      let collapsed = self.collapsed.contains(name);
      let group = name.to_owned();

      list = list.child(group_header(
        group_index,
        name,
        members.len(),
        collapsed,
        cx.listener(move |this, _, _, cx| this.toggle_group(&group, cx)),
      ));

      if collapsed {
        continue;
      }

      for member in members {
        list = list.child(member_row(row_index, member));
        row_index += 1;
      }
    }

    list
  }
}

/// 242x22 category row: chevron at x=13, uppercase label at x=27, count 8px after.
fn group_header(
  index: usize,
  name: &str,
  count: usize,
  collapsed: bool,
  on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
  // The asset points up; rotate to point down when open, right when collapsed.
  let angle = if collapsed { FRAC_PI_2 } else { PI };

  h_flex()
    .id(("member-group", index))
    .h(px(HEADER_HEIGHT))
    .w_full()
    .pl(px(LEFT_PADDING))
    .items_center()
    .cursor_pointer()
    .text_size(tokens::TYPE_S)
    .line_height(px(18.))
    .font_weight(FontWeight::BOLD)
    // The label inherits the row colour so it brightens on hover; the count
    // keeps its explicit `TEXT_SECONDARY`.
    .text_color(tokens::TEXT_TERTIARY)
    .hover(|this| this.text_color(tokens::TEXT_SECONDARY))
    .active(|this| this.opacity(0.85))
    .on_click(on_click)
    .child(
      div()
        .size(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(ScopeIcon::TriangleUp).size(px(6.)).text_color(tokens::ICON_SECONDARY).rotate(radians(angle))),
    )
    .child(div().ml(px(8.)).child(name.to_uppercase()))
    .child(div().ml(px(8.)).text_color(tokens::TEXT_SECONDARY).child(count.to_string()))
}

/// 242x38 member row: avatar at x=13, text column at x=53.
fn member_row(index: usize, member: &MemberInfo) -> impl IntoElement {
  let offline = matches!(member.presence, Presence::Offline);
  let status = member.status_text.clone();

  let name = div()
    .text_size(tokens::TYPE_M)
    .line_height(tokens::TYPE_M_LINE)
    .font_weight(FontWeight::BOLD)
    .text_color(tokens::TEXT)
    .truncate()
    .child(member.display_name.clone());

  // Name box at y=3 and subtitle at y=21 put the baselines where Figma has them.
  let text = v_flex()
    .flex_1()
    .min_w_0()
    .h_full()
    .when(status.is_some(), |this| this.pt(px(3.)))
    .when(status.is_none(), |this| this.justify_center())
    .child(name)
    .children(status.map(|status| {
      div()
        .mt(px(-2.))
        .text_size(tokens::TYPE_S)
        .line_height(tokens::TYPE_S_LINE)
        .font_weight(FontWeight::MEDIUM)
        .text_color(tokens::TEXT_SECONDARY)
        .truncate()
        .child(status)
    }));

  h_flex()
    .id(("member", index))
    .h(px(ROW_HEIGHT))
    .w_full()
    .pl(px(LEFT_PADDING))
    .pr(px(EDGE_PADDING))
    .gap(px(12.))
    .items_center()
    .cursor_pointer()
    .hover(|this| this.bg(tokens::BG_SURFACE_SECONDARY))
    .active(|this| this.bg(tokens::BG_SURFACE))
    .when(offline, |this| this.opacity(0.5))
    .child(avatar(member))
    .child(text)
}

/// 28px avatar with the presence dot hanging off its bottom-right.
fn avatar(member: &MemberInfo) -> impl IntoElement {
  let initial: String = member.display_name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();

  let picture = match &member.avatar_url {
    Some(url) => img(url.clone()).size_full().rounded_full().object_fit(ObjectFit::Cover).into_any_element(),
    None => div().child(initial).into_any_element(),
  };

  // 6px dot + 2px `BG` ring = 10px, centred 11px right of and below the avatar centre.
  let dot_offset = AVATAR_SIZE / 2. + 11. - DOT_SIZE / 2.;

  div()
    .relative()
    .size(px(AVATAR_SIZE))
    .flex_shrink_0()
    .child(
      div()
        .size(px(AVATAR_SIZE))
        .rounded_full()
        .bg(tokens::BG_SECONDARY)
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center()
        .text_size(tokens::TYPE_S)
        .font_weight(FontWeight::BOLD)
        .text_color(tokens::TEXT_SECONDARY)
        .child(picture),
    )
    .child(
      div()
        .absolute()
        .left(px(dot_offset))
        .top(px(dot_offset))
        .size(px(DOT_SIZE))
        .rounded_full()
        .bg(tokens::BG)
        .p(px(2.))
        .child(div().size_full().rounded_full().bg(presence_color(member.presence))),
    )
}
