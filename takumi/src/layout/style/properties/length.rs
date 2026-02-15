use std::ops::Neg;

use cssparser::{Parser, Token, match_ignore_ascii_case};
use taffy::{CompactLength, Dimension, LengthPercentage, LengthPercentageAuto};

use crate::{
  layout::style::{
    AspectRatio, CssToken, FromCss, ParseResult,
    tw::{TW_VAR_SPACING, TailwindPropertyParser},
  },
  rendering::Sizing,
};

/// Represents a parsed `calc()` expression as a sum of percentage, em, rem, vh, vw, and px components.
///
/// Covers patterns like `calc(100% - 0.6em)`, `calc(100vh - 60px)`, etc.
/// Other absolute units (cm, mm, in, pt, pc) are converted to px at parse time.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CalcExpr {
  /// Percentage component (0-100 scale)
  pub percentage: f32,
  /// Em component (relative to current font-size)
  pub em: f32,
  /// Rem component (relative to root font-size)
  pub rem: f32,
  /// Vh component (relative to viewport height, 0-100 scale)
  pub vh: f32,
  /// Vw component (relative to viewport width, 0-100 scale)
  pub vw: f32,
  /// Absolute pixel component
  pub px: f32,
}

impl CalcExpr {
  const ONE_CM_IN_PX: f32 = 96.0 / 2.54;
  const ONE_MM_IN_PX: f32 = Self::ONE_CM_IN_PX / 10.0;
  const ONE_Q_IN_PX: f32 = Self::ONE_CM_IN_PX / 40.0;
  const ONE_IN_PX: f32 = 96.0;
  const ONE_PT_IN_PX: f32 = Self::ONE_IN_PX / 72.0;
  const ONE_PC_IN_PX: f32 = Self::ONE_IN_PX / 6.0;

  /// Add a Length term to this calc expression.
  fn add_term<const DEFAULT_AUTO: bool>(&mut self, length: Length<DEFAULT_AUTO>, negate: bool) {
    let sign = if negate { -1.0 } else { 1.0 };
    match length {
      Length::Percentage(v) => self.percentage += v * sign,
      Length::Em(v) => self.em += v * sign,
      Length::Rem(v) => self.rem += v * sign,
      Length::Px(v) => self.px += v * sign,
      Length::Vh(v) => self.vh += v * sign,
      Length::Vw(v) => self.vw += v * sign,
      Length::Calc(inner) => {
        self.percentage += inner.percentage * sign;
        self.em += inner.em * sign;
        self.rem += inner.rem * sign;
        self.vh += inner.vh * sign;
        self.vw += inner.vw * sign;
        self.px += inner.px * sign;
      }
      // Convert other absolute units to px at parse time
      Length::Cm(v) => self.px += v * Self::ONE_CM_IN_PX * sign,
      Length::Mm(v) => self.px += v * Self::ONE_MM_IN_PX * sign,
      Length::In(v) => self.px += v * Self::ONE_IN_PX * sign,
      Length::Q(v) => self.px += v * Self::ONE_Q_IN_PX * sign,
      Length::Pt(v) => self.px += v * Self::ONE_PT_IN_PX * sign,
      Length::Pc(v) => self.px += v * Self::ONE_PC_IN_PX * sign,
      // Auto can't be resolved in calc — treat as 0
      Length::Auto => {}
    }
  }

  /// Try to simplify to a simple Length value when only one component is non-zero.
  fn simplify<const DEFAULT_AUTO: bool>(self) -> Length<DEFAULT_AUTO> {
    let has_pct = self.percentage != 0.0;
    let has_em = self.em != 0.0;
    let has_rem = self.rem != 0.0;
    let has_vh = self.vh != 0.0;
    let has_vw = self.vw != 0.0;
    let has_px = self.px != 0.0;

    let count =
      has_pct as u8 + has_em as u8 + has_rem as u8 + has_vh as u8 + has_vw as u8 + has_px as u8;

    if count == 0 {
      return Length::Px(0.0);
    }
    if count == 1 {
      if has_pct {
        return Length::Percentage(self.percentage);
      }
      if has_em {
        return Length::Em(self.em);
      }
      if has_rem {
        return Length::Rem(self.rem);
      }
      if has_vh {
        return Length::Vh(self.vh);
      }
      if has_vw {
        return Length::Vw(self.vw);
      }
      return Length::Px(self.px);
    }

    Length::Calc(self)
  }
}

/// Represents a value that can be a specific length, percentage, or automatic.
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum Length<const DEFAULT_AUTO: bool = true> {
  /// Automatic sizing based on content
  Auto,
  /// Percentage value relative to parent container (0-100)
  Percentage(f32),
  /// Rem value relative to the root font size
  Rem(f32),
  /// Em value relative to the font size
  Em(f32),
  /// Vh value relative to the viewport height (0-100)
  Vh(f32),
  /// Vw value relative to the viewport width (0-100)
  Vw(f32),
  /// Centimeter value
  Cm(f32),
  /// Millimeter value
  Mm(f32),
  /// Inch value
  In(f32),
  /// Quarter value
  Q(f32),
  /// Point value
  Pt(f32),
  /// Picas value
  Pc(f32),
  /// Specific pixel value
  Px(f32),
  /// A calc() expression combining percentage, em, rem, and/or px components.
  Calc(CalcExpr),
}

impl<const DEFAULT_AUTO: bool> Default for Length<DEFAULT_AUTO> {
  fn default() -> Self {
    if DEFAULT_AUTO {
      Self::Auto
    } else {
      Self::Px(0.0)
    }
  }
}

impl<const DEFAULT_AUTO: bool> TailwindPropertyParser for Length<DEFAULT_AUTO> {
  fn parse_tw(token: &str) -> Option<Self> {
    if let Ok(value) = token.parse::<f32>() {
      return Some(Length::Rem(value * TW_VAR_SPACING));
    }

    match AspectRatio::from_str(token) {
      Ok(AspectRatio::Ratio(ratio)) => return Some(Length::Percentage(ratio * 100.0)),
      Ok(AspectRatio::Auto) => return Some(Length::Auto),
      _ => {}
    }

    match_ignore_ascii_case! {token,
      "auto" => Some(Length::Auto),
      "dvw" => Some(Length::Vw(100.0)),
      "dvh" => Some(Length::Vh(100.0)),
      "px" => Some(Length::Px(1.0)),
      "full" => Some(Length::Percentage(100.0)),
      "3xs" => Some(Length::Rem(16.0)),
      "2xs" => Some(Length::Rem(18.0)),
      "xs" => Some(Length::Rem(20.0)),
      "sm" => Some(Length::Rem(24.0)),
      "md" => Some(Length::Rem(28.0)),
      "lg" => Some(Length::Rem(32.0)),
      "xl" => Some(Length::Rem(36.0)),
      "2xl" => Some(Length::Rem(42.0)),
      "3xl" => Some(Length::Rem(48.0)),
      "4xl" => Some(Length::Rem(56.0)),
      "5xl" => Some(Length::Rem(64.0)),
      "6xl" => Some(Length::Rem(72.0)),
      "7xl" => Some(Length::Rem(80.0)),
      _ => None,
    }
  }
}

impl<const DEFAULT_AUTO: bool> Neg for Length<DEFAULT_AUTO> {
  type Output = Self;

  fn neg(self) -> Self::Output {
    self.negative()
  }
}

impl<const DEFAULT_AUTO: bool> Length<DEFAULT_AUTO> {
  /// Returns a zero pixel length unit.
  pub const fn zero() -> Self {
    Self::Px(0.0)
  }

  /// Returns a negative length unit.
  pub fn negative(self) -> Self {
    match self {
      Length::Auto => Length::Auto,
      Length::Percentage(v) => Length::Percentage(-v),
      Length::Rem(v) => Length::Rem(-v),
      Length::Em(v) => Length::Em(-v),
      Length::Vh(v) => Length::Vh(-v),
      Length::Vw(v) => Length::Vw(-v),
      Length::Cm(v) => Length::Cm(-v),
      Length::Mm(v) => Length::Mm(-v),
      Length::In(v) => Length::In(-v),
      Length::Q(v) => Length::Q(-v),
      Length::Pt(v) => Length::Pt(-v),
      Length::Pc(v) => Length::Pc(-v),
      Length::Px(v) => Length::Px(-v),
      Length::Calc(expr) => Length::Calc(CalcExpr {
        percentage: -expr.percentage,
        em: -expr.em,
        rem: -expr.rem,
        vh: -expr.vh,
        vw: -expr.vw,
        px: -expr.px,
      }),
    }
  }
}

impl<const DEFAULT_AUTO: bool> From<f32> for Length<DEFAULT_AUTO> {
  fn from(value: f32) -> Self {
    Self::Px(value)
  }
}

impl<'i, const DEFAULT_AUTO: bool> FromCss<'i> for Length<DEFAULT_AUTO> {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    // Try parsing calc() first
    if let Ok(result) = input.try_parse(|input| {
      input.expect_function_matching("calc")?;
      input.parse_nested_block(Self::parse_calc_inner)
    }) {
      return Ok(result);
    }

    let location = input.current_source_location();
    let token = input.next()?;

    match *token {
      Token::Ident(ref unit) => match_ignore_ascii_case! {&unit,
        "auto" => Ok(Self::Auto),
        _ => Err(Self::unexpected_token_error(location, token)),
      },
      Token::Dimension {
        value, ref unit, ..
      } => {
        match_ignore_ascii_case! {&unit,
          "px" => Ok(Self::Px(value)),
          "em" => Ok(Self::Em(value)),
          "rem" => Ok(Self::Rem(value)),
          "vw" => Ok(Self::Vw(value)),
          "vh" => Ok(Self::Vh(value)),
          "cm" => Ok(Self::Cm(value)),
          "mm" => Ok(Self::Mm(value)),
          "in" => Ok(Self::In(value)),
          "q" => Ok(Self::Q(value)),
          "pt" => Ok(Self::Pt(value)),
          "pc" => Ok(Self::Pc(value)),
          _ => Err(Self::unexpected_token_error(location, token)),
        }
      }
      Token::Percentage { unit_value, .. } => Ok(Self::Percentage(unit_value * 100.0)),
      Token::Number { value, .. } => Ok(Self::Px(value)),
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("length")]
  }
}

impl<const DEFAULT_AUTO: bool> Length<DEFAULT_AUTO> {
  /// Parse the inner content of a `calc()` function.
  ///
  /// Parses additive expressions like `100% - 0.6em` or `50% + 10px + 2em`.
  /// Terms are parsed as simple Length values, combined with `+` or `-` operators.
  fn parse_calc_inner<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut expr = CalcExpr::default();

    // Parse first term
    let first = Self::parse_calc_term(input)?;
    expr.add_term(first, false);

    // Parse remaining terms: operator (+ or -) followed by a term
    loop {
      let negate = if input.try_parse(|i| i.expect_delim('+')).is_ok() {
        false
      } else if input.try_parse(|i| i.expect_delim('-')).is_ok() {
        true
      } else {
        break;
      };

      let term = Self::parse_calc_term(input)?;
      expr.add_term(term, negate);
    }

    Ok(expr.simplify())
  }

  /// Parse a single term inside a calc() expression.
  /// Handles simple values (dimensions, percentages, numbers) and nested calc().
  fn parse_calc_term<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    // Try nested calc() or other functions
    if let Ok(result) = input.try_parse(|input| {
      input.expect_function_matching("calc")?;
      input.parse_nested_block(Self::parse_calc_inner)
    }) {
      return Ok(result);
    }

    // Parse simple value (dimension, percentage, number)
    let location = input.current_source_location();
    let token = input.next()?;

    match *token {
      Token::Dimension {
        value, ref unit, ..
      } => {
        match_ignore_ascii_case! {&unit,
          "px" => Ok(Self::Px(value)),
          "em" => Ok(Self::Em(value)),
          "rem" => Ok(Self::Rem(value)),
          "vw" => Ok(Self::Vw(value)),
          "vh" => Ok(Self::Vh(value)),
          "cm" => Ok(Self::Cm(value)),
          "mm" => Ok(Self::Mm(value)),
          "in" => Ok(Self::In(value)),
          "q" => Ok(Self::Q(value)),
          "pt" => Ok(Self::Pt(value)),
          "pc" => Ok(Self::Pc(value)),
          _ => Err(Self::unexpected_token_error(location, token)),
        }
      }
      Token::Percentage { unit_value, .. } => Ok(Self::Percentage(unit_value * 100.0)),
      Token::Number { value, .. } => Ok(Self::Px(value)),
      _ => Err(Self::unexpected_token_error(location, token)),
    }
  }

  /// Converts the length unit to a compact length representation.
  ///
  /// This method converts the length unit (either a percentage, pixel, rem, em, vh, vw, or auto)
  /// into a compact length format that can be used by the layout engine.
  pub(crate) fn to_compact_length(self, sizing: &Sizing) -> CompactLength {
    match self {
      Length::Auto => CompactLength::auto(),
      Length::Percentage(value) => CompactLength::percent(value / 100.0),
      Length::Rem(value) => CompactLength::length(
        value * sizing.viewport.font_size * sizing.viewport.device_pixel_ratio,
      ),
      Length::Em(value) => {
        // `device_pixel_ratio` should NOT be applied here since it's already taken into account by `sizing.font_size`
        CompactLength::length(value * sizing.font_size)
      }
      Length::Vh(value) => {
        CompactLength::length(sizing.viewport.height.unwrap_or_default() as f32 * value / 100.0)
      }
      Length::Vw(value) => {
        CompactLength::length(sizing.viewport.width.unwrap_or_default() as f32 * value / 100.0)
      }
      _ => {
        CompactLength::length(self.to_px(sizing, sizing.viewport.width.unwrap_or_default() as f32))
      }
    }
  }

  /// Resolves the length unit to a `LengthPercentage`.
  pub(crate) fn resolve_to_length_percentage(self, sizing: &Sizing) -> LengthPercentage {
    let compact_length = self.to_compact_length(sizing);

    if compact_length.is_auto() {
      return LengthPercentage::length(0.0);
    }

    // SAFETY: only length/percentage are allowed
    unsafe { LengthPercentage::from_raw(compact_length) }
  }

  /// Resolves the length unit to a pixel value.
  pub(crate) fn to_px(self, sizing: &Sizing, percentage_full_px: f32) -> f32 {
    const ONE_CM_IN_PX: f32 = 96.0 / 2.54;
    const ONE_MM_IN_PX: f32 = ONE_CM_IN_PX / 10.0;
    const ONE_Q_IN_PX: f32 = ONE_CM_IN_PX / 40.0;
    const ONE_IN_PX: f32 = 2.54 * ONE_CM_IN_PX;
    const ONE_PT_IN_PX: f32 = ONE_IN_PX / 72.0;
    const ONE_PC_IN_PX: f32 = ONE_IN_PX / 6.0;

    let value = match self {
      Length::Auto => 0.0,
      Length::Px(value) => value,
      Length::Percentage(value) => (value / 100.0) * percentage_full_px,
      Length::Rem(value) => value * sizing.viewport.font_size,
      Length::Em(value) => value * sizing.font_size,
      Length::Vh(value) => value * sizing.viewport.height.unwrap_or_default() as f32 / 100.0,
      Length::Vw(value) => value * sizing.viewport.width.unwrap_or_default() as f32 / 100.0,
      Length::Cm(value) => value * ONE_CM_IN_PX,
      Length::Mm(value) => value * ONE_MM_IN_PX,
      Length::In(value) => value * ONE_IN_PX,
      Length::Q(value) => value * ONE_Q_IN_PX,
      Length::Pt(value) => value * ONE_PT_IN_PX,
      Length::Pc(value) => value * ONE_PC_IN_PX,
      Length::Calc(expr) => {
        // Resolve each component independently with its own DPR handling
        let pct_px = (expr.percentage / 100.0) * percentage_full_px;
        let em_px = expr.em * sizing.font_size;
        let rem_px = expr.rem * sizing.viewport.font_size * sizing.viewport.device_pixel_ratio;
        let vh_px = expr.vh * sizing.viewport.height.unwrap_or_default() as f32 / 100.0;
        let vw_px = expr.vw * sizing.viewport.width.unwrap_or_default() as f32 / 100.0;
        let px_px = expr.px * sizing.viewport.device_pixel_ratio;
        return pct_px + em_px + rem_px + vh_px + vw_px + px_px;
      }
    };

    if matches!(
      self,
      Length::Auto | Length::Percentage(_) | Length::Vh(_) | Length::Vw(_) | Length::Em(_)
    ) {
      return value;
    }

    value * sizing.viewport.device_pixel_ratio
  }

  /// Resolves the length unit to a `LengthPercentageAuto`.
  pub(crate) fn resolve_to_length_percentage_auto(self, sizing: &Sizing) -> LengthPercentageAuto {
    // SAFETY: only length/percentage/auto are allowed
    unsafe { LengthPercentageAuto::from_raw(self.to_compact_length(sizing)) }
  }

  /// Resolves the length unit to a `Dimension`.
  pub(crate) fn resolve_to_dimension(self, sizing: &Sizing) -> Dimension {
    self.resolve_to_length_percentage_auto(sizing).into()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_calc_percentage_plus_em() {
    let result = Length::<true>::from_str("calc(100% - 0.6em)");
    assert_eq!(
      result,
      Ok(Length::Calc(CalcExpr {
        percentage: 100.0,
        em: -0.6,
        rem: 0.0,
        vh: 0.0,
        vw: 0.0,
        px: 0.0,
      }))
    );
  }

  #[test]
  fn test_parse_calc_zero_percentage_plus_em() {
    // calc(0% + 0.6em) should simplify to just Em(0.6)
    let result = Length::<true>::from_str("calc(0% + 0.6em)");
    assert_eq!(result, Ok(Length::Em(0.6)));
  }

  #[test]
  fn test_parse_calc_same_units_simplify() {
    // calc(50% + 25%) should simplify to Percentage(75.0)
    let result = Length::<true>::from_str("calc(50% + 25%)");
    assert_eq!(result, Ok(Length::Percentage(75.0)));
  }

  #[test]
  fn test_parse_calc_em_only_simplify() {
    // calc(1em + 2em) should simplify to Em(3.0)
    let result = Length::<true>::from_str("calc(1em + 2em)");
    assert_eq!(result, Ok(Length::Em(3.0)));
  }

  #[test]
  fn test_parse_calc_percentage_plus_px() {
    let result = Length::<true>::from_str("calc(100% - 10px)");
    assert_eq!(
      result,
      Ok(Length::Calc(CalcExpr {
        percentage: 100.0,
        em: 0.0,
        rem: 0.0,
        vh: 0.0,
        vw: 0.0,
        px: -10.0,
      }))
    );
  }

  #[test]
  fn test_parse_calc_three_terms() {
    let result = Length::<true>::from_str("calc(50% + 1em + 5px)");
    assert_eq!(
      result,
      Ok(Length::Calc(CalcExpr {
        percentage: 50.0,
        em: 1.0,
        rem: 0.0,
        vh: 0.0,
        vw: 0.0,
        px: 5.0,
      }))
    );
  }

  #[test]
  fn test_parse_calc_negative() {
    let result = Length::<true>::from_str("calc(100% - 0.6em)");
    assert!(result.is_ok());
    let negated = result.unwrap().negative();
    assert_eq!(
      negated,
      Length::Calc(CalcExpr {
        percentage: -100.0,
        em: 0.6,
        rem: 0.0,
        vh: 0.0,
        vw: 0.0,
        px: 0.0,
      })
    );
  }

  #[test]
  fn test_parse_calc_vh_minus_px() {
    let result = Length::<true>::from_str("calc(100vh - 60px)");
    assert_eq!(
      result,
      Ok(Length::Calc(CalcExpr {
        percentage: 0.0,
        em: 0.0,
        rem: 0.0,
        vh: 100.0,
        vw: 0.0,
        px: -60.0,
      }))
    );
  }

  #[test]
  fn test_parse_calc_vw_simplify() {
    // calc(50vw + 50vw) should simplify to Vw(100.0)
    let result = Length::<true>::from_str("calc(50vw + 50vw)");
    assert_eq!(result, Ok(Length::Vw(100.0)));
  }

  #[test]
  fn test_parse_non_calc_still_works() {
    assert_eq!(Length::<true>::from_str("10px"), Ok(Length::Px(10.0)));
    assert_eq!(
      Length::<true>::from_str("50%"),
      Ok(Length::Percentage(50.0))
    );
    assert_eq!(Length::<true>::from_str("2em"), Ok(Length::Em(2.0)));
    assert_eq!(Length::<true>::from_str("auto"), Ok(Length::Auto));
  }
}
