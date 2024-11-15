use gpui::Render;
use scope_chat::message::MessageAuthor;

pub struct DiscordMessageAuthor {
  display_name: DisplayName,
  icon: String,
}

impl MessageAuthor for DiscordMessageAuthor {
  fn get_display_name(&self) -> &impl gpui::Render {
    &self.display_name
  }

  fn get_icon(&self) -> String {
    self.icon.clone()
  }
}

struct DisplayName(String);

impl Render for DisplayName {
  fn render(&mut self, cx: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
    self.0.clone()
  }
}