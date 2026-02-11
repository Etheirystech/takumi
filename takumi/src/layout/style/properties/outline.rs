use cssparser::Parser;

use crate::layout::style::{
  ColorInput, CssToken, FromCss, ParseResult,
  properties::{BorderStyle, Length},
};

/// Parsed `outline` shorthand value.
///
/// CSS outline is similar to border but does not take up space and is drawn
/// outside the border edge. Supports: `outline: <width> <style> <color>`.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Outline {
  /// Outline width.
  pub width: Option<Length>,
  /// Outline style (currently only solid is supported).
  pub style: Option<BorderStyle>,
  /// Optional outline color.
  pub color: Option<ColorInput>,
}

impl<'i> FromCss<'i> for Outline {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut width = None;
    let mut style = None;
    let mut color = None;

    loop {
      if input.is_exhausted() {
        break;
      }

      if let Ok(value) = input.try_parse(Length::from_css) {
        width = Some(value);
        continue;
      }

      if let Ok(value) = input.try_parse(BorderStyle::from_css) {
        style = Some(value);
        continue;
      }

      if let Ok(value) = input.try_parse(ColorInput::from_css) {
        color = Some(value);
        continue;
      }

      return Err(Self::unexpected_token_error(
        input.current_source_location(),
        input.next()?,
      ));
    }

    Ok(Outline {
      width,
      style,
      color,
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Token("length"),
      CssToken::Token("outline-style"),
      CssToken::Token("color"),
    ]
  }
}

#[cfg(test)]
mod tests {
  use crate::layout::style::{Color, ColorInput};

  use super::*;

  #[test]
  fn test_parse_outline_width_style_color() {
    assert_eq!(
      Outline::from_str("0.123em solid white"),
      Ok(Outline {
        width: Some(Length::Em(0.123)),
        style: Some(BorderStyle::Solid),
        color: Some(ColorInput::Value(Color([255, 255, 255, 255]))),
      })
    );
  }

  #[test]
  fn test_parse_outline_width_only() {
    assert_eq!(
      Outline::from_str("2px"),
      Ok(Outline {
        width: Some(Length::Px(2.0)),
        style: None,
        color: None,
      })
    );
  }

  #[test]
  fn test_parse_outline_empty() {
    assert_eq!(Outline::from_str(""), Ok(Outline::default()));
  }

  #[test]
  fn test_parse_outline_color_first() {
    assert_eq!(
      Outline::from_str("red solid 3px"),
      Ok(Outline {
        width: Some(Length::Px(3.0)),
        style: Some(BorderStyle::Solid),
        color: Some(ColorInput::Value(Color([255, 0, 0, 255]))),
      })
    );
  }
}
