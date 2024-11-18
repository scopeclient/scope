use gpui::{div, IntoElement, Render};

pub struct Marker {
  pub name: &'static str,
}

impl Render for Marker {
  fn render(&mut self, cx: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
    println!("Marker rendered: {}", self.name);

    div()
  }
}
