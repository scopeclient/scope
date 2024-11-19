use components::theme::ActiveTheme;
use gpui::{div, img, IntoElement, ParentElement, RenderOnce, Styled, WindowContext};
use scope_chat::channel::ChannelMetadata;
use std::sync::Arc;

#[derive(IntoElement)]
pub struct ChannelEntry {
  meta: Arc<dyn ChannelMetadata>,
}

impl ChannelEntry {
  pub fn new(meta: Arc<dyn ChannelMetadata>) -> Self {
    Self { meta }
  }
}

impl RenderOnce for ChannelEntry {
  fn render(self, cx: &mut WindowContext) -> impl IntoElement {
    div()
      .px_2()
      .py_1()
      .my_1()
      .flex()
      .items_center()
      .gap_2()
      .bg(cx.theme().list)
      .child(img(self.meta.get_icon().unwrap()).flex_shrink_0().object_fit(gpui::ObjectFit::Fill).rounded_full().w_4().h_4())
      .child(self.meta.get_name())
  }
}
