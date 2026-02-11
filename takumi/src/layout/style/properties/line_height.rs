use cssparser::{Parser, match_ignore_ascii_case};

use crate::{
  layout::style::{
    CssToken, FromCss, Length, ParseResult,
    tw::{TW_VAR_SPACING, TailwindPropertyParser},
  },
  rendering::Sizing,
};

/// Represents a line height value.
///
/// `None` means "normal" — use the font's built-in metrics (ascent + descent + leading).
/// `Some(length)` means an explicit value that resolves to absolute pixels.
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct LineHeight(pub Option<Length>);

impl From<Length> for LineHeight {
  fn from(value: Length) -> Self {
    Self(Some(value))
  }
}

impl Default for LineHeight {
  fn default() -> Self {
    // Default to 1.2em to match the previous behavior and keep existing
    // component positioning stable. Use `lineHeight: "normal"` in CSS/JSX
    // to opt into font-metrics-based line height (MetricsRelative).
    Self(Some(Length::Em(1.2)))
  }
}

impl TailwindPropertyParser for LineHeight {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {&token,
      "none" => Some(Length::Em(1.0).into()),
      "tight" => Some(Length::Em(1.25).into()),
      "snug" => Some(Length::Em(1.375).into()),
      "normal" => Some(Length::Em(1.5).into()),
      "relaxed" => Some(Length::Em(1.625).into()),
      "loose" => Some(Length::Em(2.0).into()),
      _ => {
        let Ok(value) = token.parse::<f32>() else {
          return None;
        };

        Some(Length::Em(value * TW_VAR_SPACING).into())
      }
    }
  }
}

impl<'i> FromCss<'i> for LineHeight {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    // Handle "normal" keyword
    if input
      .try_parse(|input| input.expect_ident_matching("normal"))
      .is_ok()
    {
      return Ok(LineHeight(None));
    }

    let Ok(number) = input.try_parse(Parser::expect_number) else {
      return Length::from_css(input).map(|l| LineHeight(Some(l)));
    };

    Ok(LineHeight(Some(Length::Em(number))))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("number"), CssToken::Token("length")]
  }
}

impl LineHeight {
  pub(crate) fn into_parley(self, sizing: &Sizing) -> parley::LineHeight {
    match self.0 {
      Some(length) => parley::LineHeight::Absolute(length.to_px(sizing, sizing.font_size)),
      // "normal" — let parley use the font's natural line height metrics
      None => parley::LineHeight::MetricsRelative(1.0),
    }
  }
}
