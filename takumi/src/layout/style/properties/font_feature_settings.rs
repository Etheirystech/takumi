use cssparser::Parser;
use parley::FontFeature;

use crate::layout::style::{CssToken, FromCss, MakeComputed, ParseResult};

/// Controls OpenType font features via CSS font-feature-settings property.
///
/// This allows enabling/disabling specific typographic features in OpenType fonts
/// such as ligatures, kerning, small caps, and other advanced typography features.
pub type FontFeatureSettings = Box<[FontFeature]>;

impl MakeComputed for FontFeatureSettings {}

impl<'i> FromCss<'i> for FontFeatureSettings {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(Box::from_iter(FontFeature::parse_list(
      input.current_line(),
    )))
  }

  fn from_str(source: &'i str) -> ParseResult<'i, Self> {
    Ok(Box::from_iter(FontFeature::parse_list(source)))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Keyword("normal"), CssToken::Token("string")]
  }
}
