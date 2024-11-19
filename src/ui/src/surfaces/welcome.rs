use components::theme::ActiveTheme;
use gpui::{div, ParentElement, Render, Styled, ViewContext};

pub struct Welcome {
  ctx: &mut ViewContext<'_, Self>,
}

impl Welcome {
  pub fn new(ctx: &mut ViewContext<'_, Self>) -> Welcome {
    Welcome { ctx }
  }
}

impl Render for Welcome {
  fn render(&mut self, cx: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
    div().bg(cx.theme().background).w_full().h_full().flex().flex_col().child(div().child("cool"))
  }
}
