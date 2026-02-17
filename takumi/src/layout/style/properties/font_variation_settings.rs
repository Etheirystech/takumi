use cssparser::Parser;
use parley::FontVariation;

use crate::layout::style::{CssToken, FromCss, MakeComputed, ParseResult};

/// Controls variable font axis values via CSS font-variation-settings property.
///
/// This allows fine-grained control over variable font characteristics like weight,
/// width, slant, and other custom axes defined in the font.
pub type FontVariationSettings = Box<[FontVariation]>;

impl MakeComputed for FontVariationSettings {}

impl<'i> FromCss<'i> for FontVariationSettings {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Box::from_iter(FontVariation::parse_list(
      input.current_line(),
    )))
  }

  fn from_str(source: &'i str) -> ParseResult<'i, Self> {
    Ok(Box::from_iter(FontVariation::parse_list(source)))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Keyword("normal"), CssToken::Token("string")]
  }
}
