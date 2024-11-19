use gpui::{div, IntoElement, ParentElement, RenderOnce, Styled, WindowContext};

#[derive(Clone, IntoElement, Debug)]
pub struct DiscordMessageContent {
  pub content: String,
  pub is_pending: bool,
}

impl RenderOnce for DiscordMessageContent {
  fn render(self, _: &mut WindowContext) -> impl IntoElement {
    div().opacity(if self.is_pending { 0.25 } else { 1.0 }).child(self.content.clone())
  }
}
