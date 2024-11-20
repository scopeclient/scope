use gpui::{div, Element, IntoElement, ParentElement, RenderOnce, Styled, WindowContext};
use scope_chat::message::MessageAuthor;

#[derive(Clone, Debug)]
pub struct DiscordMessageAuthor {
  pub display_name: DisplayName,
  pub icon: String,
  pub id: String,
}

impl PartialEq for DiscordMessageAuthor {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}
impl Eq for DiscordMessageAuthor {}

impl MessageAuthor for DiscordMessageAuthor {
  fn get_display_name(&self) -> impl Element {
    self.display_name.clone().into_element()
  }

  fn get_icon(&self) -> String {
    self.icon.clone()
  }

  fn get_small_icon(&self) -> String {
    let icon = match self.icon.strip_suffix("?size=1024") {
      Some(strip) => strip.to_owned(),
      None => self.icon.to_owned(),   
    };
    icon + "?size=32"
  }

  fn get_id(&self) -> String {
    self.id.clone()
  }
}

#[derive(Clone, IntoElement, Debug)]
pub struct DisplayName(pub String);

impl RenderOnce for DisplayName {
  fn render(self, _: &mut WindowContext) -> impl IntoElement {
    div().text_sm().child(self.0)
  }
}
