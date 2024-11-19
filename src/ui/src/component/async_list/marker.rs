use gpui::{div, IntoElement, ParentElement, Render};

pub struct Marker {
  pub name: &'static str,
}

impl Render for Marker {
  fn render(&mut self, cx: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
    div().child(self.name.to_owned())
  }
}
