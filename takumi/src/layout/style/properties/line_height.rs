use cssparser::{Parser, match_ignore_ascii_case};

use crate::{
  layout::style::{
    CssToken, FromCss, Length, MakeComputed, ParseResult,
    tw::{TW_VAR_SPACING, TailwindPropertyParser},
  },
  rendering::Sizing,
};

/// Represents a line height value, number value is parsed as em.
#[derive(Debug, Clone, PartialEq, Copy, Default)]
pub enum LineHeight {
  /// Normal line height.
  #[default]
  Normal,
  /// A unitless line height which is relative to the font size.
  Unitless(f32),
  /// A specific line height.
  Length(Length),
}

impl From<Length> for LineHeight {
  fn from(value: Length) -> Self {
    Self::Length(value)
  }
}

impl TailwindPropertyParser for LineHeight {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {&token,
      "none" => Some(LineHeight::Unitless(1.0)),
      "tight" => Some(LineHeight::Unitless(1.25)),
      "snug" => Some(LineHeight::Unitless(1.375)),
      "normal" => Some(LineHeight::Unitless(1.5)),
      "relaxed" => Some(LineHeight::Unitless(1.625)),
      "loose" => Some(LineHeight::Unitless(2.0)),
      _ => {
        let Ok(value) = token.parse::<f32>() else {
          return None;
        };

        Some(LineHeight::Unitless(value * TW_VAR_SPACING))
      }
    }
  }
}

impl<'i> FromCss<'i> for LineHeight {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("normal"))
      .is_ok()
    {
      return Ok(Self::Normal);
    }

    let Ok(number) = input.try_parse(Parser::expect_number) else {
      return Length::from_css(input).map(Into::into);
    };

    Ok(LineHeight::Unitless(number))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("number"), CssToken::Token("length")]
  }
}

impl LineHeight {
  pub(crate) fn into_parley(self, sizing: &Sizing) -> parley::LineHeight {
    match self {
      Self::Normal => parley::LineHeight::MetricsRelative(1.0),
      Self::Length(length) => parley::LineHeight::Absolute(length.to_px(sizing, sizing.font_size)),
      Self::Unitless(value) => parley::LineHeight::FontSizeRelative(value),
    }
  }
}

impl MakeComputed for LineHeight {
  fn make_computed(&mut self, sizing: &Sizing) {
    if let Self::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}
