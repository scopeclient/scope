//! Emoji rendering: Twemoji images for unicode emoji, CDN/backend images for
//! custom emoji. The full Twemoji set ships in `assets/emoji/twemoji/`.

use gpui::{AnyElement, IntoElement, ObjectFit, ParentElement, Styled, StyledImage as _, div, img, px};

use crate::{emoji_bundled, model::Emoji};

/// Twemoji file name for an emoji string, following twemoji.js: code points
/// joined by `-`, with U+FE0F dropped unless the sequence contains a ZWJ.
pub fn twemoji_code(emoji: &str) -> String {
  let keep_fe0f = emoji.contains('\u{200D}');
  emoji
    .chars()
    .filter(|c| keep_fe0f || *c != '\u{FE0F}')
    .map(|c| format!("{:x}", c as u32))
    .collect::<Vec<_>>()
    .join("-")
}

/// Asset path (bundled) or CDN URL (anything newer than the bundle) for a unicode emoji.
pub fn twemoji_url(emoji: &str) -> String {
  let code = twemoji_code(emoji);

  if emoji_bundled::BUNDLED.binary_search(&code.as_str()).is_ok() {
    format!("emoji/twemoji/{code}.png")
  } else {
    format!("https://cdn.jsdelivr.net/gh/jdecked/twemoji@{}/assets/72x72/{code}.png", emoji_bundled::TWEMOJI_TAG)
  }
}

/// True if `s` consists only of emoji (and whitespace) — used for jumbo rendering.
pub fn is_emoji_only(s: &str) -> bool {
  let trimmed = s.trim();
  !trimmed.is_empty() && trimmed.split_whitespace().all(|token| token.chars().all(is_emoji_char))
}

/// Rough per-character emoji test: pictographs, symbols, modifiers and joiners.
pub fn is_emoji_char(c: char) -> bool {
  matches!(c as u32,
    0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2300..=0x23FF | 0x2B00..=0x2BFF
      | 0xFE0F | 0x200D | 0x20E3 | 0xE0020..=0xE007F | 0x2190..=0x21FF | 0x2460..=0x24FF | 0x25A0..=0x25FF | 0x2900..=0x297F | 0x3030 | 0x303D | 0x3297 | 0x3299 | 0x00A9 | 0x00AE | 0x203C | 0x2049 | 0x2122 | 0x2139
  )
}

/// Image source for any emoji.
pub fn emoji_url(emoji: &Emoji, size_hint: u32) -> String {
  match emoji {
    Emoji::Unicode(s) => twemoji_url(s),
    Emoji::Custom { .. } => emoji.image_url(size_hint).unwrap_or_default(),
  }
}

/// A square emoji image of `size` logical pixels.
pub fn render_emoji(emoji: &Emoji, size: f32) -> AnyElement {
  let url = emoji_url(emoji, if size > 32. { 96 } else { 64 });
  let alt = emoji.label();

  div()
    .flex_shrink_0()
    .size(px(size))
    .flex()
    .items_center()
    .justify_center()
    .child(img(url).size(px(size)).object_fit(ObjectFit::Contain).with_fallback(move || div().text_size(px(size * 0.8)).child(alt.clone()).into_any_element()))
    .into_any_element()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn codes_follow_twemoji_rules() {
    assert_eq!(twemoji_code("🔥"), "1f525");
    assert_eq!(twemoji_code("❤️"), "2764"); // FE0F dropped
    assert_eq!(twemoji_code("❤️‍🔥"), "2764-fe0f-200d-1f525"); // kept with ZWJ
    assert_eq!(twemoji_code("#️⃣"), "23-20e3");
    assert_eq!(twemoji_code("🇬🇧"), "1f1ec-1f1e7");
  }

  #[test]
  fn bundled_set_is_sorted_and_complete_for_common_emoji() {
    assert!(emoji_bundled::BUNDLED.windows(2).all(|w| w[0] < w[1]));
    for e in ["🔥", "👀", "🚀", "🎉", "👍", "❤️", "😂", "🫡"] {
      assert!(twemoji_url(e).starts_with("emoji/twemoji/"), "{e} should be bundled");
    }
  }

  #[test]
  fn emoji_only_detection() {
    assert!(is_emoji_only("🔥🔥🔥"));
    assert!(is_emoji_only("🎉 🔥"));
    assert!(!is_emoji_only("fire 🔥"));
    assert!(!is_emoji_only(""));
  }
}
