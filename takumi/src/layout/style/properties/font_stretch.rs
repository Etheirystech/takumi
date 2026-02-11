use cssparser::{Parser, Token, match_ignore_ascii_case};
use parley::FontWidth;

use crate::layout::style::{CssToken, FromCss, ParseResult};

/// Controls the width/stretch of text rendering.
///
/// Maps to the CSS `font-stretch` property and wraps parley's `FontWidth`.
/// Supports both keyword values (e.g., `condensed`, `expanded`) and percentage values.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct FontStretch(FontWidth);

impl<'i> FromCss<'i> for FontStretch {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();

    // Try parsing as a percentage first (e.g., `75%`, `112.5%`)
    if let Ok(value) = input.try_parse(|input| input.expect_percentage()) {
      return Ok(Self(FontWidth::from_percentage(value * 100.0)));
    }

    // Parse as keyword
    let ident = input.expect_ident()?;
    match_ignore_ascii_case! { ident,
      "normal" => Ok(Self(FontWidth::NORMAL)),
      "ultra-condensed" => Ok(Self(FontWidth::ULTRA_CONDENSED)),
      "extra-condensed" => Ok(Self(FontWidth::EXTRA_CONDENSED)),
      "condensed" => Ok(Self(FontWidth::CONDENSED)),
      "semi-condensed" => Ok(Self(FontWidth::SEMI_CONDENSED)),
      "semi-expanded" => Ok(Self(FontWidth::SEMI_EXPANDED)),
      "expanded" => Ok(Self(FontWidth::EXPANDED)),
      "extra-expanded" => Ok(Self(FontWidth::EXTRA_EXPANDED)),
      "ultra-expanded" => Ok(Self(FontWidth::ULTRA_EXPANDED)),
      _ => Err(Self::unexpected_token_error(location, &Token::Ident(ident.to_owned()))),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("normal"),
      CssToken::Keyword("ultra-condensed"),
      CssToken::Keyword("extra-condensed"),
      CssToken::Keyword("condensed"),
      CssToken::Keyword("semi-condensed"),
      CssToken::Keyword("semi-expanded"),
      CssToken::Keyword("expanded"),
      CssToken::Keyword("extra-expanded"),
      CssToken::Keyword("ultra-expanded"),
      CssToken::Token("percentage"),
    ]
  }
}

impl From<FontStretch> for FontWidth {
  fn from(value: FontStretch) -> Self {
    value.0
  }
}
