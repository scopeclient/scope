use gpui::IntoElement;

pub trait Message {
  fn get_author(&self) -> &impl MessageAuthor;
  fn get_content(&self) -> &impl gpui::Render;
  fn get_identifier(&self) -> String;
}

pub trait MessageAuthor {
  fn get_display_name(&self) -> &impl gpui::Render;
  fn get_icon(&self) -> String;
}