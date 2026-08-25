//! "zach is typing…" state for one channel.

use std::time::{Duration, Instant};

use gpui::{Context, SharedString};

/// How long a typing notice stays visible without a refresh (Discord resends every ~10s).
pub const TYPING_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Default)]
pub struct TypingIndicator {
  users: Vec<(String, Instant)>,
}

impl TypingIndicator {
  pub fn started(&mut self, user: String, cx: &mut Context<Self>) {
    self.users.retain(|(u, _)| *u != user);
    self.users.push((user, Instant::now()));
    self.prune();
    cx.notify();

    cx.spawn(async move |this, cx| {
      cx.background_executor().timer(TYPING_TIMEOUT + Duration::from_millis(50)).await;
      this
        .update(cx, |this, cx| {
          this.prune();
          cx.notify();
        })
        .ok();
    })
    .detach();
  }

  /// A message arrived in the channel: whoever was typing has (most likely) sent it.
  pub fn message_arrived(&mut self, cx: &mut Context<Self>) {
    if !self.users.is_empty() {
      self.users.clear();
      cx.notify();
    }
  }

  fn prune(&mut self) {
    let now = Instant::now();
    self.users.retain(|(_, at)| now.duration_since(*at) < TYPING_TIMEOUT);
  }

  pub fn text(&self) -> Option<SharedString> {
    let now = Instant::now();
    let names: Vec<&str> = self.users.iter().filter(|(_, at)| now.duration_since(*at) < TYPING_TIMEOUT).map(|(u, _)| u.as_str()).collect();

    let text = match names.as_slice() {
      [] => return None,
      [a] => format!("{a} is typing..."),
      [a, b] => format!("{a} and {b} are typing..."),
      [a, b, c] => format!("{a}, {b} and {c} are typing..."),
      _ => "several people are typing...".to_string(),
    };

    Some(text.into())
  }
}
