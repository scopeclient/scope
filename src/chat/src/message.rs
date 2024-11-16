use gpui::Element;

pub trait Message: Clone {
  fn get_author(&self) -> &impl MessageAuthor;
  fn get_content(&self) -> impl Element;
  fn get_identifier(&self) -> String;
  fn get_nonce(&self) -> Option<&String>;
}

pub trait MessageAuthor {
  fn get_display_name(&self) -> impl Element;
  fn get_icon(&self) -> String;
}
