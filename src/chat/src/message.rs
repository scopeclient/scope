use chrono::{DateTime, Utc};
use gpui::Element;

use crate::async_list::AsyncListItem;
use crate::reaction::MessageReaction;

pub trait Message: Clone + AsyncListItem + Send {
  fn get_author(&self) -> &impl MessageAuthor;
  fn get_content(&self) -> impl Element;
  fn get_identifier(&self) -> String;
  fn get_nonce(&self) -> Option<&String>;
  fn should_group(&self, previous: &Self) -> bool;
  fn get_timestamp(&self) -> Option<DateTime<Utc>>;
  fn get_reactions(&self) -> Vec<impl MessageReaction>;
}

pub trait MessageAuthor: PartialEq + Eq {
  fn get_display_name(&self) -> impl Element;
  fn get_icon(&self) -> String;
  fn get_small_icon(&self) -> String;
  fn get_id(&self) -> String;
}
