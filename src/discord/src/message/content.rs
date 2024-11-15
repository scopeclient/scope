use gpui::{IntoElement, Render};

pub struct DiscordMessageContent {
  content: String,
}

impl Render for DiscordMessageContent {
  fn render(&mut self, _: &mut gpui::ViewContext<Self>) -> impl IntoElement {
    self.content.clone()
  }
}