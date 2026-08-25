use std::fmt::Debug;

use chrono::{DateTime, Utc};
use gpui::{App, Entity, IntoElement, Window};
use scope_rich::RichContentView;

use crate::async_list::AsyncListItem;

pub trait Message: Clone + AsyncListItem + Send {
  type Identifier: Sized + Copy + Clone + Debug + Eq + PartialEq;
  type Author: MessageAuthor<Identifier = <Self as Message>::Identifier>;

  fn get_author(&self) -> Self::Author;
  /// Sent by the signed-in user (enables edit/delete).
  fn is_own(&self) -> bool;
  /// The rendered body (text, attachments, embeds, …), cached per message.
  fn get_content(&self, window: &mut Window, cx: &mut App) -> Entity<RichContentView>;
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
