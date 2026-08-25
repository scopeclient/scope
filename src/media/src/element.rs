//! Small gpui helpers shared by every place that renders playback controls
//! (attachment cards in `scope-rich`, the media bar in `scope`).

use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{Div, InteractiveElement, MouseButton, ParentElement, Pixels, Styled, canvas, px};

use crate::MediaPlayer;

/// Makes `content` a click-to-seek surface: a left click seeks the global
/// player to the clicked fraction of the element's width. The click does not
/// propagate, so a surrounding play/pause click target is unaffected.
///
/// The element's painted bounds are captured with an invisible overlay canvas
/// each frame, so the handler can turn a window-space mouse position into a
/// fraction without hard-coding widths.
pub fn seekable(content: Div, enabled: bool) -> Div {
  if !enabled {
    return content;
  }

  // (origin_x, width) of the surface, in window pixels.
  let bounds: Rc<Cell<(Pixels, Pixels)>> = Rc::new(Cell::new((px(0.), px(0.))));
  let captured = bounds.clone();

  content
    .relative()
    .child(canvas(move |b, _, _| captured.set((b.origin.x, b.size.width)), |_, _, _, _| {}).absolute().size_full())
    .on_mouse_down(MouseButton::Left, move |event, _, cx| {
      cx.stop_propagation();
      let (origin, width) = bounds.get();
      if width > px(0.) {
        MediaPlayer::seek_fraction(((event.position.x - origin) / width).clamp(0., 1.), cx);
      }
    })
    .cursor_pointer()
}

/// `m:ss`, rounding to whole seconds.
pub fn format_duration(duration: Duration) -> String {
  let secs = duration.as_secs_f32().round().max(0.) as u64;
  format!("{}:{:02}", secs / 60, secs % 60)
}

/// `m:ss / m:ss` (elapsed / total).
pub fn format_progress(position: Duration, duration: Option<Duration>) -> String {
  match duration {
    Some(duration) => format!("{} / {}", format_duration(position.min(duration)), format_duration(duration)),
    None => format_duration(position),
  }
}

/// A 4px seekable progress bar: `track` colored rail, `fill` up to `fraction`.
/// `height` is the clickable band around the rail.
pub fn progress_bar(fraction: f32, height: f32, track: gpui::Hsla, fill: gpui::Hsla, enabled: bool) -> Div {
  use gpui::relative;

  let rail = gpui::div()
    .w_full()
    .h(px(4.))
    .rounded_full()
    .bg(track)
    .overflow_hidden()
    .child(gpui::div().h_full().w(relative(fraction.clamp(0., 1.))).rounded_full().bg(fill));

  seekable(gpui::div().flex_1().min_w_0().h(px(height)).flex().items_center().child(rail), enabled)
}
