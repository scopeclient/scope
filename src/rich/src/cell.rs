//! Per-message cache of the rendered content entity.

use std::sync::{Arc, OnceLock};

use gpui::{App, AppContext as _, Entity};

use crate::{RichContentView, RichMessage};

/// Lazily creates (once) the gpui entity that renders a message's content, so
/// spoiler/reveal state survives list rebuilds. Backends embed one per message.
#[derive(Clone, Default, Debug)]
pub struct ContentCell(Arc<OnceLock<Entity<RichContentView>>>);

impl ContentCell {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn get_or_create(&self, cx: &mut App, build: impl FnOnce() -> Arc<RichMessage>) -> Entity<RichContentView> {
    self.0.get_or_init(|| cx.new(|cx| RichContentView::new(build(), cx))).clone()
  }
}
