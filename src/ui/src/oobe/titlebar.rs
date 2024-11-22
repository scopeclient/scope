use components::{h_flex, theme::ActiveTheme, Icon, IconName, Sizable as _};
use gpui::{
  div, prelude::FluentBuilder as _, px, relative, rgb, AnyElement, Div, Element, Hsla, InteractiveElement as _, IntoElement, ParentElement, Pixels,
  RenderOnce, Stateful, StatefulInteractiveElement as _, Style, Styled, WindowContext,
};

pub const TITLE_BAR_HEIGHT: Pixels = px(35.);
#[cfg(target_os = "macos")]
const TITLE_BAR_LEFT_PADDING: Pixels = px(80.);
#[cfg(not(target_os = "macos"))]
const TITLE_BAR_LEFT_PADDING: Pixels = px(12.);

/// TitleBar used to customize the appearance of the title bar.
///
/// We can put some elements inside the title bar.
#[derive(IntoElement)]
pub struct OobeTitleBar {
  base: Stateful<Div>,
  children: Vec<AnyElement>,
}

impl OobeTitleBar {
  pub fn new() -> Self {
    Self {
      base: div().id("title-bar").pl(TITLE_BAR_LEFT_PADDING),
      children: Vec::new(),
    }
  }
}

// The Windows control buttons have a fixed width of 35px.
//
// We don't need implementation the click event for the control buttons.
// If user clicked in the bounds, the window event will be triggered.
#[derive(IntoElement, Clone)]
enum ControlIcon {
  Close,
}

impl ControlIcon {
  fn close() -> Self {
    Self::Close
  }

  fn id(&self) -> &'static str {
    match self {
      Self::Close => "close",
    }
  }

  fn icon(&self) -> IconName {
    match self {
      Self::Close => IconName::WindowClose,
    }
  }

  fn is_close(&self) -> bool {
    matches!(self, Self::Close)
  }

  fn fg(&self, cx: &WindowContext) -> Hsla {
    if cx.theme().mode.is_dark() {
      components::white()
    } else {
      components::black()
    }
  }

  fn hover_fg(&self, cx: &WindowContext) -> Hsla {
    if self.is_close() || cx.theme().mode.is_dark() {
      components::white()
    } else {
      components::black()
    }
  }

  fn hover_bg(&self, cx: &WindowContext) -> Hsla {
    if self.is_close() {
      if cx.theme().mode.is_dark() {
        components::red_800()
      } else {
        components::red_600()
      }
    } else if cx.theme().mode.is_dark() {
      components::stone_700()
    } else {
      components::stone_200()
    }
  }
}

impl RenderOnce for ControlIcon {
  fn render(self, cx: &mut WindowContext) -> impl IntoElement {
    let fg = self.fg(cx);
    let hover_fg = self.hover_fg(cx);
    let hover_bg = self.hover_bg(cx);
    let icon = self.clone();
    let is_linux = cfg!(target_os = "linux");

    div()
      .id(self.id())
      .flex()
      .cursor_pointer()
      .w(TITLE_BAR_HEIGHT)
      .h_full()
      .justify_center()
      .content_center()
      .items_center()
      .text_color(fg)
      .when(is_linux, |this| {
        this.on_click(move |_, cx| match icon {
          Self::Close { .. } => {
            cx.remove_window();
          }
        })
      })
      .hover(|style| style.bg(hover_bg).text_color(hover_fg))
      .active(|style| style.bg(hover_bg.opacity(0.7)))
      .child(Icon::new(self.icon()).small())
  }
}

#[derive(IntoElement)]
struct WindowControls {}

impl RenderOnce for WindowControls {
  fn render(self, _: &mut WindowContext) -> impl IntoElement {
    if cfg!(target_os = "macos") {
      return div().id("window-controls");
    }

    h_flex().id("window-controls").items_center().flex_shrink_0().h_full().child(ControlIcon::close())
  }
}

impl Styled for OobeTitleBar {
  fn style(&mut self) -> &mut gpui::StyleRefinement {
    self.base.style()
  }
}

impl ParentElement for OobeTitleBar {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for OobeTitleBar {
  fn render(self, cx: &mut WindowContext) -> impl IntoElement {
    let is_linux = cfg!(target_os = "linux");

    const HEIGHT: Pixels = px(34.);

    div()
      .flex_shrink_0()
      .w_full()
      .child(
        self
          .base
          .flex()
          .w_full()
          .flex_row()
          .items_center()
          .justify_end()
          .h(HEIGHT)
          .when(cx.is_fullscreen(), |this| this.pl(px(12.)))
          .child(WindowControls {}),
      )
      .when(is_linux, |this| {
        this.child(div().top_0().left_0().absolute().size_full().h_full().child(TitleBarElement {}))
      })
  }
}

/// A TitleBar Element that can be move the window.
pub struct TitleBarElement {}

impl IntoElement for TitleBarElement {
  type Element = Self;

  fn into_element(self) -> Self::Element {
    self
  }
}

impl Element for TitleBarElement {
  type RequestLayoutState = ();

  type PrepaintState = ();

  fn id(&self) -> Option<gpui::ElementId> {
    None
  }

  fn request_layout(&mut self, _: Option<&gpui::GlobalElementId>, cx: &mut WindowContext) -> (gpui::LayoutId, Self::RequestLayoutState) {
    let mut style = Style::default();
    style.flex_grow = 1.0;
    style.flex_shrink = 1.0;
    style.size.width = relative(1.).into();
    style.size.height = relative(1.).into();

    let id = cx.request_layout(style, []);
    (id, ())
  }

  fn prepaint(
    &mut self,
    _: Option<&gpui::GlobalElementId>,
    _: gpui::Bounds<Pixels>,
    _: &mut Self::RequestLayoutState,
    _: &mut WindowContext,
  ) -> Self::PrepaintState {
  }

  #[allow(unused_variables)]
  fn paint(
    &mut self,
    _: Option<&gpui::GlobalElementId>,
    bounds: gpui::Bounds<Pixels>,
    _: &mut Self::RequestLayoutState,
    _: &mut Self::PrepaintState,
    cx: &mut WindowContext,
  ) {
    use gpui::{MouseButton, MouseMoveEvent, MouseUpEvent};
    cx.on_mouse_event(move |ev: &MouseMoveEvent, _, cx: &mut WindowContext| {
      if bounds.contains(&ev.position) && ev.pressed_button == Some(MouseButton::Left) {
        cx.start_window_move();
      }
    });

    cx.on_mouse_event(move |ev: &MouseUpEvent, _, cx: &mut WindowContext| {
      if ev.button == MouseButton::Left {
        cx.show_window_menu(ev.position);
      }
    });
  }
}
