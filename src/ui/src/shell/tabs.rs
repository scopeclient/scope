//! Top strip (44px): history arrows, open channel tabs, "+", window controls.
//! Doubles as the window title bar (client-side decorations): dragging an
//! empty area moves the window, double-clicking it zooms.

use std::f32::consts::PI;

use gpui::{
  div, img, prelude::*, px, radians, AnyView, App, Context, Div, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton, ObjectFit,
  ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, StyledImage, Window, WindowControlArea,
};
use gpui_component::{h_flex, tooltip::Tooltip, Icon, InteractiveElementExt as _};
use scope_chat::nav::Id;

use crate::{icons::ScopeIcon, shell::TABS_HEIGHT, state::AppState, theme::tokens};

/// Width of one tab (design: 204x44, edge to edge, no gap).
const TAB_WIDTH: Pixels = px(204.);
/// Size of the square hit box wrapped around the 14px history arrows and the 12px "+".
const SMALL_BUTTON: Pixels = px(20.);
/// Size of the server-logo tile used in tabs and the breadcrumb.
pub const SERVER_TILE: Pixels = px(18.);

const IS_MACOS: bool = cfg!(target_os = "macos");
const IS_WINDOWS: bool = cfg!(target_os = "windows");

pub struct TabsBar {
  state: Entity<AppState>,
  /// Left button is down on an empty part of the bar; the next mouse move starts a window drag.
  should_move: bool,
}

impl TabsBar {
  pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
    cx.observe(&state, |_, _, cx| cx.notify()).detach();
    TabsBar { state, should_move: false }
  }
}

/// Hover tooltip with a fixed label.
pub fn tooltip(text: &'static str) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
  move |window, cx| Tooltip::new(text).build(window, cx)
}

/// 18x18 rounded server-logo tile (radius 4). Shows the guild icon clipped to
/// the tile, or the first letter of `fallback` when there is no icon. Demo mode
/// uses the bundled placeholder logos so screenshots match the mockup.
pub fn server_tile(state: &AppState, guild: Option<Id>, icon_url: Option<&str>, fallback: &str) -> Div {
  let tile = div().size(SERVER_TILE).flex_shrink_0().relative().overflow_hidden().rounded(tokens::RADIUS_100).bg(tokens::BG_SURFACE);

  if state.is_demo() {
    // Placement mirrors the Figma "botlogos" component: the logo is drawn larger
    // than the tile and clipped, so it fills the tile the way the mockup does.
    match guild {
      Some(Id(10)) => {
        return tile
          .bg(tokens::hex(0x140c27))
          .child(img("brand/placeholder-server-a.png").absolute().left(px(-3.)).top(px(-1.)).size(px(24.)).object_fit(ObjectFit::Cover));
      }
      Some(Id(11)) => {
        return tile
          .bg(tokens::hex(0x0f121a))
          .child(img("brand/placeholder-server-b.png").absolute().left(px(2.)).top(px(4.)).w(px(15.)).h(px(11.)).object_fit(ObjectFit::Cover));
      }
      _ => {}
    }
  }

  match icon_url {
    Some(url) => tile.child(img(url.to_string()).size_full().object_fit(ObjectFit::Cover)),
    None => {
      let initial: String = fallback.trim_start_matches('#').chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
      tile.flex().items_center().justify_center().text_size(px(10.)).font_weight(FontWeight::BOLD).text_color(tokens::TEXT_SECONDARY).child(initial)
    }
  }
}

/// Square hit box around a small glyph that must not start a window drag.
fn glyph_button(id: &'static str, size: Pixels, color: Hsla, hover: Hsla, icon: Icon, tip: &'static str) -> impl IntoElement {
  div()
    .id(id)
    .size(size)
    .flex_shrink_0()
    .flex()
    .items_center()
    .justify_center()
    .rounded(tokens::RADIUS_100)
    .cursor_pointer()
    .text_color(color)
    .hover(move |style| style.bg(tokens::BG_FILL).text_color(hover))
    .active(|style| style.opacity(0.85))
    .tooltip(tooltip(tip))
    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    .child(icon)
}

/// Custom minimize / close control drawn on platforms without native traffic lights.
fn window_control(
  id: &'static str,
  width: Pixels,
  area: WindowControlArea,
  icon: Icon,
  tip: &'static str,
  action: fn(&mut Window),
) -> impl IntoElement {
  div()
    .id(id)
    .w(width)
    .h_full()
    .flex_shrink_0()
    .flex()
    .items_center()
    .justify_center()
    .cursor_pointer()
    .text_color(tokens::BORDER_TERTIARY)
    .hover(|style| style.bg(tokens::BG_FILL).text_color(tokens::ICON_SECONDARY))
    .active(|style| style.opacity(0.85))
    .tooltip(tooltip(tip))
    // Windows: the platform handles clicks in control areas itself.
    .when(IS_WINDOWS, |this| this.window_control_area(area))
    .when(!IS_WINDOWS, |this| {
      this
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
          window.prevent_default();
          cx.stop_propagation();
        })
        .on_click(move |_, window, cx| {
          cx.stop_propagation();
          action(window);
        })
    })
    .child(icon)
}

impl Render for TabsBar {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let state = self.state.read(cx);
    let active = state.active_tab;

    let tabs: Vec<_> = state
      .tabs
      .iter()
      .enumerate()
      .map(|(index, tab)| {
        let is_active = active == Some(index);
        let tile = server_tile(state, tab.guild, tab.icon_url.as_deref(), &tab.title);

        h_flex()
          .id(("tab", index))
          .h_full()
          .w(TAB_WIDTH)
          .flex_shrink_0()
          // Logo at x=11; the 16px close hit box puts its 8px glyph at right 13.
          .pl(px(11.))
          .pr(px(9.))
          .items_center()
          .cursor_pointer()
          .when(is_active, |this| this.bg(tokens::BG_SECONDARY))
          .when(!is_active, |this| {
            this
              .hover(|style| {
                style.bg(Hsla {
                  a: 0.5,
                  ..tokens::BG_SURFACE_TERTIARY
                })
              })
              .active(|style| style.bg(tokens::BG_SURFACE_TERTIARY))
          })
          .child(tile)
          .child(
            div()
              .ml(px(12.))
              .flex_1()
              .min_w_0()
              .truncate()
              .text_size(tokens::TYPE_M)
              .font_weight(FontWeight::BOLD)
              .text_color(tokens::TEXT)
              .child(tab.title.clone()),
          )
          .child(
            div()
              .id(("tab-close", index))
              .ml(px(8.))
              .size(px(16.))
              .flex_shrink_0()
              .flex()
              .items_center()
              .justify_center()
              .rounded(tokens::RADIUS_100)
              .cursor_pointer()
              .text_color(tokens::ICON_SECONDARY)
              .hover(|style| style.bg(tokens::BG_FILL).text_color(tokens::ICON_HOVER))
              .active(|style| style.opacity(0.85))
              .tooltip(tooltip("Close tab"))
              .child(Icon::new(ScopeIcon::Close).size(px(8.)))
              .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
              .on_click(cx.listener(move |this, _, _, cx| this.state.update(cx, |s, cx| s.close_tab(index, cx)))),
          )
          // Keep the bar's drag / double-click-to-zoom from triggering on tabs.
          .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
          .on_mouse_down(
            MouseButton::Middle,
            cx.listener(move |this, _, _, cx| this.state.update(cx, |s, cx| s.close_tab(index, cx))),
          )
          .on_click(cx.listener(move |this, _, _, cx| this.state.update(cx, |s, cx| s.activate_tab(index, cx))))
      })
      .collect();

    let back = Icon::new(ScopeIcon::ArrowRight).size(px(14.)).rotate(radians(PI));
    let forward = Icon::new(ScopeIcon::ArrowRight).size(px(14.));

    // Arrows: 14px glyphs 6px apart, inside 20px hit boxes (3px slack each side).
    let history = h_flex()
      .flex_shrink_0()
      .h_full()
      .items_center()
      .mr(px(11.))
      .child(glyph_button(
        "nav-back",
        SMALL_BUTTON,
        tokens::ICON_SECONDARY,
        tokens::ICON_SECONDARY_HOVER,
        back,
        "Back",
      ))
      .child(glyph_button(
        "nav-forward",
        SMALL_BUTTON,
        tokens::ICON_SECONDARY,
        tokens::ICON_SECONDARY_HOVER,
        forward,
        "Forward",
      ));

    // "+" sits 10px after the last tab (hit box is 4px wider than the glyph per side).
    let new_tab = div().ml(px(6.)).child(glyph_button(
      "new-tab",
      SMALL_BUTTON,
      tokens::BORDER_TERTIARY,
      tokens::ICON_SECONDARY,
      Icon::new(ScopeIcon::Plus).size(px(12.)),
      "New tab",
    ));

    let strip = h_flex().h_full().flex_1().min_w_0().overflow_hidden().items_center().children(tabs).child(new_tab);

    // Non-macOS: 12x3 minimize glyph at right 40 and 11x10 close glyph at right 16.
    let controls = h_flex()
      .flex_shrink_0()
      .h_full()
      .items_center()
      .pr(px(10.))
      .gap(px(1.))
      .child(window_control(
        "window-minimize",
        px(24.),
        WindowControlArea::Min,
        Icon::new(ScopeIcon::WindowMinimize).w(px(12.)).h(px(3.)),
        "Minimize",
        |window| window.minimize_window(),
      ))
      .child(window_control(
        "window-close",
        px(23.),
        WindowControlArea::Close,
        Icon::new(ScopeIcon::WindowClose).w(px(11.)).h(px(10.)),
        "Close",
        |window| window.remove_window(),
      ));

    // Everything except the window controls doubles as the title bar: press on an
    // empty spot and move to drag the window (Linux; macOS drags natively,
    // Windows via the `Drag` control area), double-click to zoom. Tabs and
    // buttons stop mouse-down propagation so they never start a drag. The
    // controls sit outside this region because gpui resolves control areas by
    // the first hitbox under the cursor, so a parent `Drag` would shadow them.
    let drag_region = h_flex()
      .id("tabs-drag")
      .flex_1()
      .min_w_0()
      .h_full()
      .items_center()
      // macOS: leave room for the traffic lights `main.rs` places at (12, 14).
      .pl(if IS_MACOS { px(75.) } else { px(11.) })
      .window_control_area(WindowControlArea::Drag)
      .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, _| this.should_move = true))
      .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.should_move = false))
      .on_mouse_down_out(cx.listener(|this, _, _, _| this.should_move = false))
      .on_mouse_move(cx.listener(|this, _, window, _| {
        if this.should_move {
          this.should_move = false;
          window.start_window_move();
        }
      }))
      // Windows toggles maximize on caption double-clicks itself.
      .when(!IS_WINDOWS, |this| {
        this.on_double_click(|_, window, _| {
          if IS_MACOS {
            window.titlebar_double_click();
          } else {
            window.zoom_window();
          }
        })
      })
      .child(history)
      .child(strip);

    h_flex()
      .id("tabs-bar")
      .w_full()
      .h(px(TABS_HEIGHT))
      .flex_shrink_0()
      .items_center()
      .bg(tokens::BG)
      .child(drag_region)
      .when(!IS_MACOS, |this| this.child(controls))
  }
}
