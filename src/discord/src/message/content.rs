use gpui::{div, img, px, IntoElement, ParentElement, Render, RenderOnce, Styled, WindowContext};
use serenity::all::Attachment;

#[derive(Clone, IntoElement)]
pub struct DiscordMessageContent {
  pub content: String,
  pub is_pending: bool,
}

#[derive(Clone, IntoElement)]
pub struct DiscordImageContent {
  pub images: Vec<Attachment>,
  pub is_pending: bool,
}

impl RenderOnce for DiscordMessageContent {
  fn render(self, _: &mut WindowContext) -> impl IntoElement {
    div().opacity(if self.is_pending { 0.25 } else { 1.0 }).child(self.content.clone())
  }
}
impl RenderOnce for DiscordImageContent {
  fn render(self, _: &mut WindowContext) -> impl IntoElement {
    div()
      .opacity(if self.is_pending { 0.25 } else { 1.0 })
      .children(self.images.into_iter().map(|image| img(image.url).object_fit(gpui::ObjectFit::Contain).rounded_2xl().h(px(128.)).w(px(128.))))
  }
}
