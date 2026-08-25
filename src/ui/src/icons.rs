//! Scope-specific icons exported from the Figma file (`assets/icons/scope/*.svg`).
//!
//! gpui renders SVGs as masks tinted with the current text colour, so the
//! fills baked into the files do not matter. For icons not in this set, fall
//! back to gpui-component's bundled Lucide set via `gpui_component::IconName`.

use gpui::SharedString;
use gpui_component::IconNamed;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScopeIcon {
  /// `#` channel glyph (16x16).
  Hash,
  /// Small disclosure triangle pointing up (6x5.4). Rotate for other directions.
  TriangleUp,
  /// Search glyph used in the channel search box (16x16).
  Search,
  /// Heavier search glyph used in the channel bar (18x18).
  SearchBold,
  /// Horizontal three-dot menu (14x14).
  Ellipsis,
  /// History "forward" arrow (14x14). Flip horizontally for "back".
  ArrowRight,
  /// "+" glyph used for new tab / add section (12x12).
  Plus,
  WindowMinimize,
  WindowClose,
  /// Small "x" used to close a tab (8x8).
  Close,
  /// Sidebar collapse toggle (18x18).
  PanelToggle,
  Pin,
  /// Single member silhouette used to toggle the member list (10x12.5).
  Member,
  Emoji,
  Upload,
  Heart,
  ChatText,
  /// Arrow leaving a box ("open in popout") (18x18).
  ArrowOut,
  MembersBody,
  MembersHead,
}

impl IconNamed for ScopeIcon {
  fn path(self) -> SharedString {
    match self {
      Self::Hash => "icons/scope/hash.svg",
      Self::TriangleUp => "icons/scope/triangle-up.svg",
      Self::Search => "icons/scope/search.svg",
      Self::SearchBold => "icons/scope/search-bold.svg",
      Self::Ellipsis => "icons/scope/ellipsis.svg",
      Self::ArrowRight => "icons/scope/arrow-right.svg",
      Self::Plus => "icons/scope/plus.svg",
      Self::WindowMinimize => "icons/scope/window-minimize.svg",
      Self::WindowClose => "icons/scope/window-close.svg",
      Self::Close => "icons/scope/close.svg",
      Self::PanelToggle => "icons/scope/panel-toggle.svg",
      Self::Pin => "icons/scope/pin.svg",
      Self::Member => "icons/scope/member.svg",
      Self::Emoji => "icons/scope/emoji.svg",
      Self::Upload => "icons/scope/upload.svg",
      Self::Heart => "icons/scope/heart.svg",
      Self::ChatText => "icons/scope/chat-text.svg",
      Self::ArrowOut => "icons/scope/arrow-out.svg",
      Self::MembersBody => "icons/scope/members-body.svg",
      Self::MembersHead => "icons/scope/members-head.svg",
    }
    .into()
  }
}
