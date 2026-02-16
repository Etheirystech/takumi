use std::f32::consts::TAU;

use cssparser::Parser;
use image::{GenericImageView, Rgba};

use super::gradient_utils::{adaptive_lut_size, build_color_lut, resolve_stops_along_axis};
use crate::{
  layout::style::{
    Angle, BackgroundPosition, CssToken, FromCss, GradientStop, GradientStops, ParseResult,
  },
  rendering::RenderContext,
};

/// Represents a CSS conic-gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct ConicGradient {
  /// The starting angle of the gradient (default 0deg = from top).
  pub from_angle: Angle,
  /// Center position (default 50% 50%).
  pub center: BackgroundPosition,
  /// Gradient color stops.
  pub stops: Box<[GradientStop]>,
}

/// Precomputed data for repeated sampling of a `ConicGradient`.
#[derive(Debug, Clone)]
pub(crate) struct ConicGradientTile {
  /// Target width in pixels.
  pub width: u32,
  /// Target height in pixels.
  pub height: u32,
  /// Center X coordinate in pixels.
  pub cx: f32,
  /// Center Y coordinate in pixels.
  pub cy: f32,
  /// Starting angle in radians (CSS 0deg = from top, clockwise).
  pub start_rad: f32,
  /// Pre-computed color lookup table for fast gradient sampling.
  /// Maps normalized angle [0.0, 1.0] (fraction of full turn) to color.
  pub color_lut: Box<[Rgba<u8>]>,
}

impl GenericImageView for ConicGradientTile {
  type Pixel = Rgba<u8>;

  fn dimensions(&self) -> (u32, u32) {
    (self.width, self.height)
  }

  fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
    // Fast path for empty or single-color gradients
    if self.color_lut.is_empty() {
      return Rgba([0, 0, 0, 0]);
    }
    if self.color_lut.len() == 1 {
      return self.color_lut[0];
    }

    let dx = x as f32 - self.cx;
    let dy = y as f32 - self.cy;

    // atan2 gives angle from positive X axis, counter-clockwise.
    // CSS conic gradients start from top (negative Y axis) and go clockwise.
    // Convert: css_angle = atan2(dx, -dy) (measured from top, clockwise)
    let angle_from_top = dx.atan2(-dy); // range [-π, π]

    // Subtract start angle and normalize to [0, 2π)
    let adjusted = (angle_from_top - self.start_rad).rem_euclid(std::f32::consts::TAU);

    // Normalize to [0.0, 1.0)
    let normalized = (adjusted / std::f32::consts::TAU).clamp(0.0, 1.0);

    // Map to LUT index
    let lut_idx = (normalized * (self.color_lut.len() - 1) as f32).round() as usize;

    self.color_lut[lut_idx]
  }
}

impl ConicGradientTile {
  /// Builds a drawing context from a conic gradient and a target viewport.
  pub fn new(gradient: &ConicGradient, width: u32, height: u32, context: &RenderContext) -> Self {
    use crate::layout::style::Length;

    let cx = Length::from(gradient.center.0.x).to_px(&context.sizing, width as f32);
    let cy = Length::from(gradient.center.0.y).to_px(&context.sizing, height as f32);

    let start_rad = gradient.from_angle.to_radians();
    let axis_degrees = 360.0;

    // Resolve stop percentages against one full turn (360deg).
    let resolved_stops = resolve_stops_along_axis(&gradient.stops, axis_degrees, context);

    // Match angular resolution to the largest visible ring in this tile.
    let dx_left = cx;
    let dx_right = width as f32 - cx;
    let dy_top = cy;
    let dy_bottom = height as f32 - cy;
    let max_radius = [
      (dx_left * dx_left + dy_top * dy_top).sqrt(),
      (dx_left * dx_left + dy_bottom * dy_bottom).sqrt(),
      (dx_right * dx_right + dy_top * dy_top).sqrt(),
      (dx_right * dx_right + dy_bottom * dy_bottom).sqrt(),
    ]
    .into_iter()
    .fold(0.0_f32, f32::max);

    let lut_size = adaptive_lut_size((TAU * max_radius).max(1.0));
    let color_lut = build_color_lut(&resolved_stops, axis_degrees, lut_size);

    ConicGradientTile {
      width,
      height,
      cx,
      cy,
      start_rad,
      color_lut,
    }
  }
}

impl<'i> FromCss<'i> for ConicGradient {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, ConicGradient> {
    input.expect_function_matching("conic-gradient")?;

    input.parse_nested_block(|input| {
      let mut from_angle: Option<Angle> = None;
      let mut center: Option<BackgroundPosition> = None;

      // Parse optional "from <angle>" and/or "at <position>" before the comma
      loop {
        // Try "from <angle>"
        if input.try_parse(|i| i.expect_ident_matching("from")).is_ok() {
          from_angle = Some(Angle::from_css(input)?);
          continue;
        }

        // Try "at <position>"
        if input.try_parse(|i| i.expect_ident_matching("at")).is_ok() {
          center = Some(BackgroundPosition::from_css(input)?);
          continue;
        }

        // Consume the comma separator if present
        input.try_parse(Parser::expect_comma).ok();
        break;
      }

      let stops = GradientStops::from_css(input)?;

      Ok(ConicGradient {
        from_angle: from_angle.unwrap_or(Angle::zero()),
        center: center.unwrap_or_default(),
        stops: stops.into_boxed_slice(),
      })
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("conic-gradient()")]
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::style::{Color, Length, StopPosition};
  use crate::{GlobalContext, rendering::RenderContext};

  #[test]
  fn test_parse_conic_gradient_basic() {
    let gradient = ConicGradient::from_str("conic-gradient(#ff0000, #0000ff)");

    assert_eq!(
      gradient,
      Ok(ConicGradient {
        from_angle: Angle::zero(),
        center: BackgroundPosition::default(),
        stops: [
          GradientStop::ColorHint {
            color: Color([255, 0, 0, 255]).into(),
            hint: None,
          },
          GradientStop::ColorHint {
            color: Color([0, 0, 255, 255]).into(),
            hint: None,
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_parse_conic_gradient_with_stops() {
    assert_eq!(
      ConicGradient::from_str("conic-gradient(#ff0000 0%, #00ff00 50%, #0000ff 100%)"),
      Ok(ConicGradient {
        from_angle: Angle::zero(),
        center: BackgroundPosition::default(),
        stops: [
          GradientStop::ColorHint {
            color: Color([255, 0, 0, 255]).into(),
            hint: Some(StopPosition(Length::Percentage(0.0))),
          },
          GradientStop::ColorHint {
            color: Color([0, 255, 0, 255]).into(),
            hint: Some(StopPosition(Length::Percentage(50.0))),
          },
          GradientStop::ColorHint {
            color: Color([0, 0, 255, 255]).into(),
            hint: Some(StopPosition(Length::Percentage(100.0))),
          },
        ]
        .into(),
      })
    );
  }

  #[test]
  fn test_conic_gradient_top_pixel_is_first_color() {
    let gradient = ConicGradient {
      from_angle: Angle::zero(),
      center: BackgroundPosition::default(),
      stops: [
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(100.0))),
        },
      ]
      .into(),
    };

    let context = GlobalContext::default();
    let render_context = RenderContext::new(&context, (100, 100).into(), Default::default());
    let tile = ConicGradientTile::new(&gradient, 100, 100, &render_context);

    // Top center (50, 0) should be red (start of gradient)
    let color_top = tile.get_pixel(50, 0);
    assert_eq!(color_top, Rgba([255, 0, 0, 255]));
  }

  #[test]
  fn test_conic_gradient_hard_stops() {
    // Simulate the card cost gradient: 3 colors with hard stops
    let gradient = ConicGradient {
      from_angle: Angle::zero(),
      center: BackgroundPosition::default(),
      stops: [
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(0.0))),
        },
        GradientStop::ColorHint {
          color: Color([255, 0, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(33.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 255, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(33.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 255, 0, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(66.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(66.0))),
        },
        GradientStop::ColorHint {
          color: Color([0, 0, 255, 255]).into(),
          hint: Some(StopPosition(Length::Percentage(100.0))),
        },
      ]
      .into(),
    };

    let context = GlobalContext::default();
    let render_context = RenderContext::new(&context, (100, 100).into(), Default::default());
    let tile = ConicGradientTile::new(&gradient, 100, 100, &render_context);

    // Top-center should be red
    let top = tile.get_pixel(50, 0);
    assert_eq!(top, Rgba([255, 0, 0, 255]));

    // Bottom should be green (roughly 180deg = 50% of turn, within the 33%–66% green zone)
    let bottom = tile.get_pixel(50, 99);
    assert_eq!(bottom, Rgba([0, 255, 0, 255]));
  }
}
