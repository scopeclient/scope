use gpui::{IntoElement, Render, RenderOnce, WindowContext};

#[derive(Clone, IntoElement)]
pub struct DiscordMessageContent {
  pub content: String,
}

impl RenderOnce for DiscordMessageContent {
  fn render(self, _: &mut WindowContext) -> impl IntoElement {
    self.content.clone()
  }
}