use cssparser::match_ignore_ascii_case;

use crate::layout::style::{declare_enum_from_css_impl, tw::TailwindPropertyParser};

/// How children overflowing their container should affect layout
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Overflow {
  /// The automatic minimum size of this node as a flexbox/grid item should be based on the size of its content.
  /// Content that overflows this node *should* contribute to the scroll region of its parent.
  #[default]
  Visible,
  /// The automatic minimum size of this node as a flexbox/grid item should be `0`.
  /// Content that overflows this node should *not* contribute to the scroll region of its parent.
  Hidden,
  /// Content that overflows this node is clipped (like Hidden), but the automatic minimum size
  /// is still based on content (like Visible). This matches CSS `overflow: clip` behavior where
  /// the element clips visually but doesn't affect flex/grid auto minimum sizing.
  Clip,
}

declare_enum_from_css_impl!(
  Overflow,
  "visible" => Overflow::Visible,
  "hidden" => Overflow::Hidden,
  "clip" => Overflow::Clip,
);

impl TailwindPropertyParser for Overflow {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "visible" => Some(Overflow::Visible),
      "hidden" => Some(Overflow::Hidden),
      "clip" => Some(Overflow::Clip),
      _ => None,
    }
  }
}

impl From<Overflow> for taffy::Overflow {
  fn from(val: Overflow) -> Self {
    match val {
      Overflow::Visible => taffy::Overflow::Visible,
      Overflow::Hidden => taffy::Overflow::Hidden,
      // Clip uses Visible for layout (preserving content-based auto min-size)
      // but clips visually during rendering.
      Overflow::Clip => taffy::Overflow::Visible,
    }
  }
}
