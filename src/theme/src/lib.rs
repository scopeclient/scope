//! Design tokens exported from the Scope Figma file (Tokens sections).
//!
//! Colours are `Hsla` so they can be passed straight to gpui style builders.
//! Names mirror the Figma variable names: `color/bg/surface` -> `BG_SURFACE`.

use gpui::{Hsla, Pixels, px};

/// Build an opaque colour from an `0xRRGGBB` literal at compile time.
pub const fn hex(rgb: u32) -> Hsla {
  hexa((rgb << 8) | 0xff)
}

/// Build a colour from an `0xRRGGBBAA` literal at compile time.
pub const fn hexa(rgba: u32) -> Hsla {
  let r = ((rgba >> 24) & 0xff) as f32 / 255.;
  let g = ((rgba >> 16) & 0xff) as f32 / 255.;
  let b = ((rgba >> 8) & 0xff) as f32 / 255.;
  let a = (rgba & 0xff) as f32 / 255.;

  let max = if r > g {
    if r > b { r } else { b }
  } else if g > b {
    g
  } else {
    b
  };
  let min = if r < g {
    if r < b { r } else { b }
  } else if g < b {
    g
  } else {
    b
  };

  let l = (max + min) / 2.;
  let delta = max - min;

  if delta == 0. {
    return Hsla { h: 0., s: 0., l, a };
  }

  let s = if l > 0.5 { delta / (2. - max - min) } else { delta / (max + min) };
  let h = if max == r {
    ((g - b) / delta + if g < b { 6. } else { 0. }) / 6.
  } else if max == g {
    ((b - r) / delta + 2.) / 6.
  } else {
    ((r - g) / delta + 4.) / 6.
  };

  Hsla { h, s, l, a }
}

// ---- color/bg -------------------------------------------------------------
pub const BG: Hsla = hex(0x111215);
pub const BG_SECONDARY: Hsla = hex(0x16171c);
pub const BG_INVERSE: Hsla = hex(0xffffff);
pub const BG_SURFACE: Hsla = hex(0x292b33);
pub const BG_SURFACE_SECONDARY: Hsla = hex(0x1e2028);
pub const BG_SURFACE_TERTIARY: Hsla = hex(0x16171c);
pub const BG_FILL: Hsla = hex(0x40444f);
pub const BG_FILL_SECONDARY: Hsla = hex(0x292b33);
pub const BG_FILL_TERTIARY: Hsla = hex(0x1e2028);
pub const BG_FILL_BRAND: Hsla = hexa(0xf24890d6);
pub const BG_FILL_BRAND_SECONDARY: Hsla = hexa(0xcf407d61);

// ---- color/text -----------------------------------------------------------
pub const TEXT: Hsla = hex(0xf1f2f4);
pub const TEXT_SECONDARY: Hsla = hex(0xa8acb8);
pub const TEXT_TERTIARY: Hsla = hex(0x65687a);
pub const TEXT_MUTED: Hsla = hex(0x575c6b);
pub const TEXT_DISABLED: Hsla = hex(0x2b2e39);
pub const TEXT_SUCCESS: Hsla = hex(0x3ac79d);
pub const TEXT_WARNING: Hsla = hex(0xffcc00);
pub const TEXT_DANGER: Hsla = hex(0xcf2658);
pub const TEXT_LINK: Hsla = hex(0x3b65fc);
pub const TEXT_LINK_HOVER: Hsla = hex(0x4d77ff);
pub const TEXT_LINK_ACTIVE: Hsla = hex(0x2f50d8);
pub const TEXT_LINK_DISABLED: Hsla = hex(0x292b33);

// ---- color/icon -----------------------------------------------------------
pub const ICON: Hsla = hex(0x8b91a1);
pub const ICON_HOVER: Hsla = hex(0xa8acb8);
pub const ICON_ACTIVE: Hsla = hex(0x65687a);
pub const ICON_SELECTED: Hsla = hex(0xf1f2f4);
pub const ICON_SECONDARY: Hsla = hex(0x65687a);
pub const ICON_SECONDARY_HOVER: Hsla = hex(0x8b91a1);
pub const ICON_SECONDARY_ACTIVE: Hsla = hex(0x575c6b);
pub const ICON_TERTIARY: Hsla = hex(0xffffff);
pub const ICON_BRAND: Hsla = hex(0xfc4f97);
pub const ICON_DISABLED: Hsla = hex(0x292b33);
pub const ICON_SUCCESS: Hsla = hex(0x3ac79d);
pub const ICON_WARNING: Hsla = hex(0xffcc00);
pub const ICON_DANGER: Hsla = hex(0xcf2658);
pub const ICON_INVERSE: Hsla = hex(0x111215);
pub const ICON_INVERSE_SECONDARY: Hsla = hex(0x292b33);

// ---- color/border ---------------------------------------------------------
pub const BORDER: Hsla = hex(0x22252f);
pub const BORDER_SECONDARY: Hsla = hex(0x2b2e39);
pub const BORDER_TERTIARY: Hsla = hex(0x40444f);
pub const BORDER_BRAND: Hsla = hex(0xfc3b8c);
pub const BORDER_DISABLED: Hsla = hex(0x1e2028);
pub const BORDER_SUCCESS: Hsla = hex(0x3ac79d);
pub const BORDER_WARNING: Hsla = hex(0xffcc00);
pub const BORDER_DANGER: Hsla = hex(0xcf2658);
pub const BORDER_INVERSE: Hsla = hex(0x8b91a1);

// ---- brand ----------------------------------------------------------------
pub const PINK_900: Hsla = hex(0xfc3b8c);
pub const PINK_950: Hsla = hex(0x9c0242);
pub const BRAND: Hsla = PINK_900;
pub const BRAND_HOVER: Hsla = hex(0xfd5b9d);
pub const BRAND_ACTIVE: Hsla = hex(0xe0287a);

// ---- border/radius --------------------------------------------------------
pub const RADIUS_050: Pixels = px(2.);
pub const RADIUS_100: Pixels = px(4.);
pub const RADIUS_150: Pixels = px(6.);
pub const RADIUS_200: Pixels = px(8.);
pub const RADIUS_250: Pixels = px(10.);
pub const RADIUS_300: Pixels = px(12.);
pub const RADIUS_400: Pixels = px(24.);

// ---- space ----------------------------------------------------------------
pub const SPACE_050: Pixels = px(2.);
pub const SPACE_100: Pixels = px(4.);
pub const SPACE_200: Pixels = px(8.);
pub const SPACE_300: Pixels = px(12.);
pub const SPACE_400: Pixels = px(16.);
pub const SPACE_500: Pixels = px(20.);
pub const SPACE_600: Pixels = px(24.);
pub const SPACE_700: Pixels = px(28.);
pub const SPACE_800: Pixels = px(32.);
pub const SPACE_900: Pixels = px(36.);
pub const SPACE_1000: Pixels = px(40.);
pub const SPACE_1100: Pixels = px(44.);
pub const SPACE_1200: Pixels = px(48.);
pub const SPACE_1600: Pixels = px(64.);
pub const SPACE_2000: Pixels = px(80.);

// ---- typeface (size / line-height) ----------------------------------------
pub const TYPE_XS: Pixels = px(10.);
pub const TYPE_XS_LINE: Pixels = px(12.);
pub const TYPE_S: Pixels = px(12.);
pub const TYPE_S_LINE: Pixels = px(16.);
pub const TYPE_M: Pixels = px(14.);
pub const TYPE_M_LINE: Pixels = px(20.);
pub const TYPE_L: Pixels = px(16.);
pub const TYPE_L_LINE: Pixels = px(24.);
pub const HEADING_S: Pixels = px(20.);
pub const HEADING_S_LINE: Pixels = px(32.);
pub const HEADING_M: Pixels = px(24.);
pub const HEADING_M_LINE: Pixels = px(36.);
pub const HEADING_L: Pixels = px(32.);
pub const HEADING_L_LINE: Pixels = px(40.);
pub const HEADING_XL: Pixels = px(36.);
pub const HEADING_XL_LINE: Pixels = px(48.);

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn hex_roundtrips_through_gpui() {
    for value in [0x111215u32, 0xf1f2f4, 0xfc3b8c, 0x3ac79d, 0x000000, 0xffffff] {
      let ours = hex(value);
      let theirs: Hsla = gpui::rgb(value).into();
      assert!((ours.h - theirs.h).abs() < 1e-4, "{value:#x} h {} vs {}", ours.h, theirs.h);
      assert!((ours.s - theirs.s).abs() < 1e-4, "{value:#x} s {} vs {}", ours.s, theirs.s);
      assert!((ours.l - theirs.l).abs() < 1e-4, "{value:#x} l {} vs {}", ours.l, theirs.l);
      assert_eq!(ours.a, 1.);
    }
  }
}
