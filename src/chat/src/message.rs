use std::fmt::Debug;

use chrono::{DateTime, Utc};
use gpui::{IntoElement, Render, View, WindowContext};

use crate::async_list::AsyncListItem;

pub trait Message: Clone + AsyncListItem + Send {
  type Identifier: Sized + Copy + Clone + Debug + Eq + PartialEq;
  type Author: MessageAuthor<Identifier = <Self as Message>::Identifier>;
  type Content: Render;

  fn get_author(&self) -> Self::Author;
  fn get_content(&self, cx: &mut WindowContext) -> View<Self::Content>;
  fn get_identifier(&self) -> Option<<Self as Message>::Identifier>;
  fn get_nonce(&self) -> impl PartialEq;
  fn should_group(&self, previous: &Self) -> bool;
  fn get_timestamp(&self) -> Option<DateTime<Utc>>;
}

#[derive(Debug, Clone, Copy)]
pub struct IconRenderConfig {
  size: usize,
}

impl Default for IconRenderConfig {
  fn default() -> Self {
    IconRenderConfig { size: 1024 }
  }
}

impl IconRenderConfig {
  pub fn small() -> Self {
    IconRenderConfig { size: 32 }
  }

  pub fn with_size(mut self, size: usize) -> IconRenderConfig {
    self.size = size;
    self
  }

  pub fn size(&self) -> usize {
    self.size
  }
}

pub trait MessageAuthor: PartialEq + Eq {
  type Identifier: Sized + Copy + Clone + Debug + Eq + PartialEq;
  type DisplayName: IntoElement + Clone;
  type Icon: IntoElement + Clone;

  fn get_display_name(&self) -> Self::DisplayName;
  fn get_icon(&self, config: IconRenderConfig) -> Self::Icon;
  fn get_identifier(&self) -> Self::Identifier;
}
