//! Scope's theme: maps the Figma design tokens onto gpui-component's theme.

/// Design tokens (colours, spacing, type scale) — shared with other crates.
pub use scope_theme as tokens;

use gpui::App;
use gpui_component::theme::{Theme, ThemeMode};
use tokens::*;

pub fn init(cx: &mut App) {
  Theme::change(ThemeMode::Dark, None, cx);

  let theme = Theme::global_mut(cx);
  theme.font_family = "Inter".into();
  theme.font_size = TYPE_M;
  theme.radius = RADIUS_150;
  theme.radius_lg = RADIUS_200;
  theme.shadow = true;

  let c = &mut theme.colors;

  c.background = BG;
  c.foreground = TEXT;
  c.border = BORDER;

  c.primary = BRAND;
  c.primary_hover = BRAND_HOVER;
  c.primary_active = BRAND_ACTIVE;
  c.primary_foreground = BG_INVERSE;

  c.secondary = BG_SURFACE;
  c.secondary_hover = BG_FILL;
  c.secondary_active = BG_SURFACE_SECONDARY;
  c.secondary_foreground = TEXT;

  c.accent = BG_SURFACE;
  c.accent_foreground = TEXT;

  c.muted = BG_SURFACE_SECONDARY;
  c.muted_foreground = TEXT_TERTIARY;

  c.input = BORDER_SECONDARY;
  c.ring = BORDER_BRAND;
  c.caret = TEXT;
  c.selection = BG_FILL_BRAND_SECONDARY;

  c.popover = BG_SURFACE_SECONDARY;
  c.popover_foreground = TEXT;

  c.list = gpui::transparent_black();
  c.list_even = gpui::transparent_black();
  c.list_hover = BG_SURFACE_SECONDARY;
  c.list_active = BG_SURFACE;
  c.list_active_border = BG_SURFACE;
  c.list_head = BG_SECONDARY;

  c.sidebar = BG_SECONDARY;
  c.sidebar_foreground = TEXT_SECONDARY;
  c.sidebar_border = BORDER;
  c.sidebar_accent = BG_SURFACE_SECONDARY;
  c.sidebar_accent_foreground = TEXT;
  c.sidebar_primary = BRAND;
  c.sidebar_primary_foreground = BG_INVERSE;

  c.tab_bar = BG_SECONDARY;
  c.tab_bar_segmented = BG_SECONDARY;
  c.tab = gpui::transparent_black();
  c.tab_foreground = TEXT_SECONDARY;
  c.tab_active = BG;
  c.tab_active_foreground = TEXT;

  c.title_bar = BG_SECONDARY;
  c.title_bar_border = BORDER;
  c.window_border = BORDER;

  c.scrollbar = gpui::transparent_black();
  c.scrollbar_thumb = BG_FILL;
  c.scrollbar_thumb_hover = TEXT_MUTED;

  c.skeleton = BG_SURFACE;
  c.progress_bar = BRAND;
  c.slider_bar = BRAND;
  c.slider_thumb = BG_INVERSE;
  c.switch = BG_FILL;
  c.switch_thumb = BG_INVERSE;

  c.link = TEXT_LINK;
  c.link_hover = TEXT_LINK_HOVER;
  c.link_active = TEXT_LINK_ACTIVE;

  c.danger = TEXT_DANGER;
  c.danger_hover = hex(0xdb3a6a);
  c.danger_active = hex(0xb21f4b);
  c.danger_foreground = BG_INVERSE;
  c.success = TEXT_SUCCESS;
  c.success_hover = hex(0x52d3ab);
  c.success_active = hex(0x2fb38c);
  c.success_foreground = ICON_INVERSE;
  c.warning = TEXT_WARNING;
  c.warning_hover = hex(0xffd633);
  c.warning_active = hex(0xe6b800);
  c.warning_foreground = ICON_INVERSE;
  c.info = TEXT_LINK;
  c.info_hover = TEXT_LINK_HOVER;
  c.info_active = TEXT_LINK_ACTIVE;
  c.info_foreground = BG_INVERSE;

  c.red = TEXT_DANGER;
  c.green = TEXT_SUCCESS;
  c.yellow = TEXT_WARNING;
  c.blue = TEXT_LINK;
  c.magenta = BRAND;
}
