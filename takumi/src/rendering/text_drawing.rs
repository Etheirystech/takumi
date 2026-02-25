use std::{borrow::Cow, convert::Into};
use unicode_linebreak::linebreaks;

use image::{GenericImageView, Pixel, Rgba, RgbaImage};
use parley::GlyphRun;
use swash::{ColorPalette, scale::outline::Outline};
use taffy::{Layout, Point, Size};
use zeno::{Command, PathData, Stroke};

use crate::{
  Result,
  layout::{
    inline::{InlineBrush, InlineLayout, break_lines},
    style::{
      Affine, BlendMode, Color, ImageScalingAlgorithm, SizedFontStyle, TextTransform,
      WhiteSpaceCollapse,
    },
  },
  rendering::{
    BorderProperties, Canvas, CanvasConstrain, ColorTile, MaskMemory, apply_mask_alpha_to_pixel,
    blend_pixel, draw_mask, mask_index_from_coord, overlay_area, sample_transformed_pixel,
  },
  resources::font::ResolvedGlyph,
};

struct SwashImageView<'a>(&'a swash::scale::image::Image);

impl<'a> GenericImageView for SwashImageView<'a> {
  type Pixel = Rgba<u8>;

  fn dimensions(&self) -> (u32, u32) {
    (self.0.placement.width, self.0.placement.height)
  }

  fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
    let index = ((y * self.0.placement.width + x) * 4) as usize;

    *Rgba::from_slice(&self.0.data[index..index + 4])
  }
}

/// Controls which drawing operations are performed per glyph.
///
/// Text rendering is split into two phases so that all strokes render
/// before any fills across a glyph run, matching CSS painting order
/// where text-stroke is behind text fill.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrawPhase {
  /// Draw text shadows, faux-bold, and text strokes.
  Stroke,
  /// Draw glyph fills.
  Fill,
}

fn invert_y_coordinate(command: Command) -> Command {
  match command {
    Command::MoveTo(point) => Command::MoveTo((point.x, -point.y).into()),
    Command::LineTo(point) => Command::LineTo((point.x, -point.y).into()),
    Command::CurveTo(point1, point2, point3) => Command::CurveTo(
      (point1.x, -point1.y).into(),
      (point2.x, -point2.y).into(),
      (point3.x, -point3.y).into(),
    ),
    Command::QuadTo(point1, point2) => {
      Command::QuadTo((point1.x, -point1.y).into(), (point2.x, -point2.y).into())
    }
    Command::Close => Command::Close,
  }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_decoration(
  canvas: &mut Canvas,
  glyph_run: &GlyphRun<'_, InlineBrush>,
  color: Color,
  offset: f32,
  size: f32,
  layout: Layout,
  transform: Affine,
  faux_stretch_factor: f32,
) {
  let tile = ColorTile {
    color: color.into(),
    width: (glyph_run.advance() * faux_stretch_factor) as u32,
    height: size as u32,
  };

  canvas.overlay_image(
    &tile,
    BorderProperties::default(),
    transform
      * Affine::translation(
        layout.border.left + layout.padding.left + glyph_run.offset(),
        layout.border.top + layout.padding.top + offset,
      ),
    ImageScalingAlgorithm::Auto,
    BlendMode::Normal,
  );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_glyph_clip_image<I: GenericImageView<Pixel = Rgba<u8>>>(
  glyph: &ResolvedGlyph,
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  mut transform: Affine,
  inline_offset: Point<f32>,
  clip_image: &I,
  faux_bold_width: f32,
  phase: DrawPhase,
  clip_offset: Point<f32>,
  faux_stretch_factor: f32,
  faux_italic_skew: f32,
) {
  // clip_sample_offset combines inline_offset (glyph position) with clip_offset
  // (ancestor crop margin). Used only for sampling the clip image, not for the
  // glyph rendering transform.
  let clip_sample_offset = Point {
    x: inline_offset.x + clip_offset.x,
    y: inline_offset.y + clip_offset.y,
  };

  transform *= Affine::translation(inline_offset.x, inline_offset.y);
  if faux_stretch_factor != 1.0 {
    transform *= Affine::scale(faux_stretch_factor, 1.0);
  }
  if faux_italic_skew != 0.0 {
    transform *= Affine {
      a: 1.0,
      b: 0.0,
      c: -faux_italic_skew,
      d: 1.0,
      x: 0.0,
      y: 0.0,
    };
  }

  match glyph {
    ResolvedGlyph::Image(bitmap) => {
      // Image glyphs (emojis) have no stroke; draw only in fill phase.
      if phase != DrawPhase::Fill {
        return;
      }

      transform *= Affine::translation(bitmap.placement.left as f32, -bitmap.placement.top as f32);

      let mask = bitmap
        .data
        .iter()
        .skip(3)
        .step_by(4)
        .copied()
        .collect::<Vec<_>>();

      let mut bottom = RgbaImage::new(bitmap.placement.width, bitmap.placement.height);

      let fill_dimensions = clip_image.dimensions();

      overlay_area(
        &mut bottom,
        Point::ZERO,
        Size {
          width: bitmap.placement.width,
          height: bitmap.placement.height,
        },
        BlendMode::Normal,
        &[],
        |x, y| {
          let alpha = mask[mask_index_from_coord(x, y, bitmap.placement.width)];

          let source_x = (x as i32 + clip_sample_offset.x as i32 + bitmap.placement.left) as u32;
          let source_y = (y as i32 + clip_sample_offset.y as i32 - bitmap.placement.top) as u32;

          if source_x >= fill_dimensions.0 || source_y >= fill_dimensions.1 {
            return Color::transparent().into();
          }

          let mut pixel = clip_image.get_pixel(source_x, source_y);

          apply_mask_alpha_to_pixel(&mut pixel, alpha);

          pixel
        },
      );

      canvas.overlay_image(
        &bottom,
        BorderProperties::default(),
        transform,
        ImageScalingAlgorithm::Auto,
        BlendMode::Normal,
      );
    }
    ResolvedGlyph::Outline(outline) => {
      // If the transform is not invertible, we can't draw the glyph
      let Some(inverse) = transform.invert() else {
        return;
      };

      let paths = collect_outline_paths(outline);

      match phase {
        DrawPhase::Stroke => {
          draw_text_shadow(canvas, style, transform, &paths);
          draw_faux_bold_clip_image(
            canvas,
            style,
            transform,
            inverse,
            &paths,
            faux_bold_width,
            clip_image,
            clip_sample_offset,
          );
          draw_text_stroke_clip_image(
            canvas,
            style,
            transform,
            &paths,
            clip_image,
            clip_sample_offset,
          );
        }
        DrawPhase::Fill => {
          let (mask, placement) = canvas.mask_memory.render(&paths, Some(transform), None);

          overlay_area(
            &mut canvas.image,
            Point {
              x: placement.left as f32,
              y: placement.top as f32,
            },
            Size {
              width: placement.width,
              height: placement.height,
            },
            BlendMode::Normal,
            &canvas.constrains,
            |x, y| {
              let alpha = mask[mask_index_from_coord(x, y, placement.width)];

              if alpha == 0 {
                return Color::transparent().into();
              }

              let sampled_pixel = sample_transformed_pixel(
                clip_image,
                inverse,
                style.parent.image_rendering,
                (x as i32 + placement.left) as f32,
                (y as i32 + placement.top) as f32,
                clip_sample_offset,
              );

              let Some(mut pixel) = sampled_pixel else {
                return Color::transparent().into();
              };

              apply_mask_alpha_to_pixel(&mut pixel, alpha);

              pixel
            },
          );
        }
      }
    }
  }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_glyph(
  glyph: &ResolvedGlyph,
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  mut transform: Affine,
  inline_offset: Point<f32>,
  color: Color,
  palette: Option<ColorPalette>,
  faux_bold_width: f32,
  phase: DrawPhase,
  faux_stretch_factor: f32,
  faux_italic_skew: f32,
) -> Result<()> {
  transform *= Affine::translation(inline_offset.x, inline_offset.y);
  if faux_stretch_factor != 1.0 {
    transform *= Affine::scale(faux_stretch_factor, 1.0);
  }
  if faux_italic_skew != 0.0 {
    // Skew horizontally: negative c component tilts glyphs to the right (italic)
    transform *= Affine {
      a: 1.0,
      b: 0.0,
      c: -faux_italic_skew,
      d: 1.0,
      x: 0.0,
      y: 0.0,
    };
  }

  match glyph {
    ResolvedGlyph::Image(bitmap) => {
      // Image glyphs (emojis) have no stroke; draw only in fill phase.
      if phase != DrawPhase::Fill {
        return Ok(());
      }

      transform *= Affine::translation(bitmap.placement.left as f32, -bitmap.placement.top as f32);

      let image = SwashImageView(bitmap);

      canvas.overlay_image(
        &image,
        Default::default(),
        transform,
        Default::default(),
        BlendMode::Normal,
      );
    }
    ResolvedGlyph::Outline(outline) => {
      let paths = collect_outline_paths(outline);

      match phase {
        DrawPhase::Stroke => {
          draw_text_shadow(canvas, style, transform, &paths);
          draw_faux_bold(canvas, transform, &paths, color, faux_bold_width);
          draw_text_stroke(canvas, style, transform, &paths);
        }
        DrawPhase::Fill => {
          if outline.is_color()
            && let Some(palette) = palette
          {
            draw_color_outline_image(
              &mut canvas.image,
              &mut canvas.mask_memory,
              outline,
              palette,
              transform,
              &canvas.constrains,
              color.0[3],
            );
          } else {
            let (mask, placement) = canvas.mask_memory.render(&paths, Some(transform), None);

            draw_mask(
              &mut canvas.image,
              mask,
              placement,
              color,
              BlendMode::Normal,
              &canvas.constrains,
            );
          }
        }
      }
    }
  }

  Ok(())
}

fn draw_text_stroke_clip_image<I: GenericImageView<Pixel = Rgba<u8>>>(
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  transform: Affine,
  paths: &[Command],
  clip_image: &I,
  inline_offset: Point<f32>,
) {
  if style.stroke_width <= 0.0 {
    return;
  }

  let Some(inverse) = transform.invert() else {
    return;
  };

  let mut stroke = Stroke::new(style.stroke_width);
  stroke.scale = false;
  stroke.join = style.parent.stroke_linejoin.into();

  let (stroke_mask, stroke_placement) =
    canvas
      .mask_memory
      .render(paths, Some(transform), Some(stroke.into()));

  overlay_area(
    &mut canvas.image,
    Point {
      x: stroke_placement.left as f32,
      y: stroke_placement.top as f32,
    },
    Size {
      width: stroke_placement.width,
      height: stroke_placement.height,
    },
    BlendMode::Normal,
    &canvas.constrains,
    |x, y| {
      let alpha = stroke_mask[mask_index_from_coord(x, y, stroke_placement.width)];

      if alpha == 0 {
        return Color::transparent().into();
      }

      let inline_x = (x as i32 + stroke_placement.left) as f32;
      let inline_y = (y as i32 + stroke_placement.top) as f32;

      let sampled_pixel = sample_transformed_pixel(
        clip_image,
        inverse,
        style.parent.image_rendering,
        inline_x,
        inline_y,
        inline_offset,
      );

      let Some(mut pixel) = sampled_pixel else {
        return Color::transparent().into();
      };

      blend_pixel(
        &mut pixel,
        style.text_stroke_color.into(),
        BlendMode::Normal,
      );
      apply_mask_alpha_to_pixel(&mut pixel, alpha);

      pixel
    },
  );
}

fn draw_text_stroke(
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  transform: Affine,
  paths: &[Command],
) {
  if style.stroke_width <= 0.0 {
    return;
  }

  let mut stroke = Stroke::new(style.stroke_width);
  stroke.scale = false;
  stroke.join = style.parent.stroke_linejoin.into();

  let (stroke_mask, stroke_placement) =
    canvas
      .mask_memory
      .render(paths, Some(transform), Some(stroke.into()));

  draw_mask(
    &mut canvas.image,
    stroke_mask,
    stroke_placement,
    style.text_stroke_color,
    BlendMode::Normal,
    &canvas.constrains,
  );
}

/// Draws a faux-bold stroke to thicken glyphs when the requested font weight
/// exceeds what the font file provides (e.g., requesting weight 800 on a
/// weight-400-only font). Uses the fill color so the stroke blends seamlessly
/// with the glyph fill drawn on top.
fn draw_faux_bold(
  canvas: &mut Canvas,
  transform: Affine,
  paths: &[Command],
  color: Color,
  faux_bold_width: f32,
) {
  if faux_bold_width <= 0.0 {
    return;
  }

  let stroke = Stroke::new(faux_bold_width);

  let (mask, placement) = canvas
    .mask_memory
    .render(paths, Some(transform), Some(stroke.into()));

  draw_mask(
    &mut canvas.image,
    mask,
    placement,
    color,
    BlendMode::Normal,
    &canvas.constrains,
  );
}

/// Faux-bold for the background-clip:text path, sampling from the clip image.
#[allow(clippy::too_many_arguments)]
fn draw_faux_bold_clip_image<I: GenericImageView<Pixel = Rgba<u8>>>(
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  transform: Affine,
  inverse: Affine,
  paths: &[Command],
  faux_bold_width: f32,
  clip_image: &I,
  inline_offset: Point<f32>,
) {
  if faux_bold_width <= 0.0 {
    return;
  }

  let stroke = Stroke::new(faux_bold_width);

  let (mask, placement) = canvas
    .mask_memory
    .render(paths, Some(transform), Some(stroke.into()));

  overlay_area(
    &mut canvas.image,
    Point {
      x: placement.left as f32,
      y: placement.top as f32,
    },
    Size {
      width: placement.width,
      height: placement.height,
    },
    BlendMode::Normal,
    &canvas.constrains,
    |x, y| {
      let alpha = mask[mask_index_from_coord(x, y, placement.width)];

      if alpha == 0 {
        return Color::transparent().into();
      }

      let sampled_pixel = sample_transformed_pixel(
        clip_image,
        inverse,
        style.parent.image_rendering,
        (x as i32 + placement.left) as f32,
        (y as i32 + placement.top) as f32,
        inline_offset,
      );

      let Some(mut pixel) = sampled_pixel else {
        return Color::transparent().into();
      };

      apply_mask_alpha_to_pixel(&mut pixel, alpha);

      pixel
    },
  );
}

fn draw_text_shadow(
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  transform: Affine,
  paths: &[Command],
) {
  let Some(ref shadows) = style.text_shadow else {
    return;
  };

  for shadow in shadows.iter() {
    shadow.draw_outset(
      &mut canvas.image,
      &mut canvas.mask_memory,
      &canvas.constrains,
      paths,
      transform,
      Default::default(),
    );
  }
}

fn collect_outline_paths(outline: &Outline) -> Vec<Command> {
  outline
    .path()
    .commands()
    .map(invert_y_coordinate)
    .collect::<Vec<_>>()
}

// https://github.com/dfrg/swash/blob/3d8e6a781c93454dadf97e5c15764ceafab228e0/src/scale/mod.rs#L921
#[allow(clippy::too_many_arguments)]
fn draw_color_outline_image(
  canvas: &mut RgbaImage,
  mask_memory: &mut MaskMemory,
  outline: &Outline,
  palette: ColorPalette,
  transform: Affine,
  constrains: &[CanvasConstrain],
  opacity: u8,
) {
  if opacity == 0 {
    return;
  }

  for i in 0..outline.len() {
    let Some(layer) = outline.get(i) else {
      break;
    };

    let Some(color) = layer.color_index().map(|index| Color(palette.get(index))) else {
      continue;
    };

    let color = color.with_opacity(opacity);

    let paths = layer
      .path()
      .commands()
      .map(invert_y_coordinate)
      .collect::<Vec<_>>();

    let (mask, placement) = mask_memory.render(&paths, Some(transform), None);

    draw_mask(
      canvas,
      mask,
      placement,
      color,
      BlendMode::Normal,
      constrains,
    );
  }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum MaxHeight {
  Absolute(f32),
  Lines(u32),
  HeightAndLines(f32, u32),
}

/// Applies text transform to the input text.
pub(crate) fn apply_text_transform<'a>(input: &'a str, transform: TextTransform) -> Cow<'a, str> {
  match transform {
    TextTransform::None => Cow::Borrowed(input),
    TextTransform::Uppercase => Cow::Owned(input.to_uppercase()),
    TextTransform::Lowercase => Cow::Owned(input.to_lowercase()),
    TextTransform::Capitalize => {
      let mut result = String::with_capacity(input.len());
      let mut start_of_word = true;
      for ch in input.chars() {
        if ch.is_alphabetic() {
          if start_of_word {
            result.extend(ch.to_uppercase());
            start_of_word = false;
          } else {
            result.extend(ch.to_lowercase());
          }
        } else {
          start_of_word = !ch.is_numeric();
          result.push(ch);
        }
      }
      Cow::Owned(result)
    }
  }
}

/// Applies whitespace collapse rules to the input text according to `WhiteSpaceCollapse`.
pub(crate) fn apply_white_space_collapse<'a>(
  input: &'a str,
  collapse: WhiteSpaceCollapse,
) -> Cow<'a, str> {
  match collapse {
    WhiteSpaceCollapse::Preserve => Cow::Borrowed(input),

    // Collapse sequences of whitespace (spaces, tabs, line breaks) into a single space.
    // Do NOT trim leading/trailing spaces here — in CSS, inter-element spaces are
    // preserved within a line. Line-edge whitespace trimming is handled by the
    // text layout engine (parley), not at the per-span level.
    WhiteSpaceCollapse::Collapse => {
      let mut out = String::with_capacity(input.len());
      let mut last_was_ws = false;

      for ch in input.chars() {
        if ch.is_whitespace() {
          if !last_was_ws {
            out.push(' ');
            last_was_ws = true;
          }
        } else {
          out.push(ch);
          last_was_ws = false;
        }
      }

      Cow::Owned(out)
    }

    // Preserve sequences of spaces/tabs but remove line breaks (replace them with a single space).
    WhiteSpaceCollapse::PreserveSpaces => {
      let mut out = String::with_capacity(input.len());
      let mut last_was_space = false;

      for ch in input.chars() {
        // treat common line break characters as breaks to be removed/replaced
        if matches!(ch, '\n' | '\r' | '\x0B' | '\x0C' | '\u{2028}' | '\u{2029}') {
          if !last_was_space {
            out.push(' ');
            last_was_space = true;
          }
        } else {
          out.push(ch);
          last_was_space = ch == ' ' || ch == '\t';
        }
      }

      Cow::Owned(out)
    }

    // Preserve line breaks but collapse consecutive spaces and tabs into single spaces.
    // Also remove leading spaces after line breaks.
    WhiteSpaceCollapse::PreserveBreaks => {
      let mut out = String::with_capacity(input.len());
      let mut last_was_space = false;
      let mut last_was_line_break = false;

      for ch in input.chars() {
        if ch == ' ' || ch == '\t' {
          // Skip leading spaces after line breaks
          if last_was_line_break {
            continue;
          }
          if !last_was_space {
            out.push(' ');
            last_was_space = true;
          }
        } else {
          out.push(ch);
          last_was_space = false;
          // Track if we just processed a line break
          last_was_line_break =
            matches!(ch, '\n' | '\r' | '\x0B' | '\x0C' | '\u{2028}' | '\u{2029}');
        }
      }

      Cow::Owned(out.trim().to_string())
    }
  }
}

/// Counts the number of word splits caused by line breaks.
/// A word split occurs when a line break happens at a position that is not
/// a legal Unicode line break opportunity (e.g. forced by break-word).
fn count_word_splits(layout: &InlineLayout, text: &str) -> usize {
  let mut splits = 0;
  let lines: Vec<_> = layout.lines().collect();

  // Get all legal break opportunities
  let legal_breaks: Vec<usize> = linebreaks(text).map(|(offset, _)| offset).collect();

  for i in 0..lines.len().saturating_sub(1) {
    let end = lines[i].text_range().end;
    let start = lines[i + 1].text_range().start;

    if end == start && end > 0 && end < text.len() && !legal_breaks.contains(&end) {
      splits += 1;
    }
  }

  splits
}

/// Use binary search to find the minimum width that maintains the same number of lines.
/// Returns `true` if a meaningful adjustment was made.
pub(crate) fn make_balanced_text(
  inline_layout: &mut InlineLayout,
  text: &str,
  max_width: f32,
  max_height: Option<MaxHeight>,
  target_lines: usize,
  device_pixel_ratio: f32,
) -> bool {
  if target_lines <= 1 {
    return false;
  }

  let initial_splits = count_word_splits(inline_layout, text);

  // Binary search between half width and full width
  let mut left = max_width / 2.0;
  let mut right = max_width;

  // Safety limit on iterations to prevent infinite loops
  const MAX_ITERATIONS: u32 = 20;
  let mut iterations = 0;

  while left + device_pixel_ratio < right && iterations < MAX_ITERATIONS {
    iterations += 1;
    let mid = (left + right) / 2.0;

    break_lines(inline_layout, mid, None);
    let lines_at_mid = inline_layout.lines().count();

    if lines_at_mid > target_lines || count_word_splits(inline_layout, text) > initial_splits {
      // Too narrow or introduced new word splits
      left = mid;
    } else {
      // Can fit in target lines, try narrower
      right = mid;
    }
  }

  let balanced_width = right.ceil();

  // No meaningful adjustment if within 1px * DPR of max_width
  if (balanced_width - max_width).abs() < device_pixel_ratio {
    // Reset to original max_width
    break_lines(inline_layout, max_width, max_height);
    false
  } else {
    // Apply the balanced width
    break_lines(inline_layout, balanced_width, max_height);
    true
  }
}

/// Attempts to avoid orphans (single short words on the last line) by adjusting line breaks.
/// Returns `true` if a meaningful adjustment was made.
pub(crate) fn make_pretty_text(
  inline_layout: &mut InlineLayout,
  max_width: f32,
  max_height: Option<MaxHeight>,
) -> bool {
  // Get the last line width at the current max width (layout should already be broken)
  let Some(last_line_width) = inline_layout
    .lines()
    .last()
    .map(|line| line.runs().map(|run| run.advance()).sum::<f32>())
  else {
    return false;
  };

  // Check if the last line is too short (less than 1/3 of container width)
  if last_line_width >= max_width / 3.0 {
    return false;
  }

  // Get original line count
  let original_lines = inline_layout.lines().count();

  // Only apply if we have more than one line (single line text doesn't need adjustment)
  if original_lines <= 1 {
    return false;
  }

  // Try reflowing with 90% width to redistribute words
  let adjusted_width = max_width * 0.9;
  break_lines(inline_layout, adjusted_width, None);
  let adjusted_lines = inline_layout.lines().count();

  // Use the adjusted width only if it doesn't add too many lines (at most 30% more)
  let max_acceptable_lines = ((original_lines as f32) * 1.3).ceil() as usize;

  if adjusted_lines <= max_acceptable_lines {
    true
  } else {
    // Reset to original max_width
    break_lines(inline_layout, max_width, max_height);
    false
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_white_space_preserve() {
    let input = "  a \t b\n";
    let out = apply_white_space_collapse(input, WhiteSpaceCollapse::Preserve);
    assert_eq!(out, input);
  }

  #[test]
  fn test_white_space_collapse() {
    let input = "  a \n\t b  c\n\n ";
    let out = apply_white_space_collapse(input, WhiteSpaceCollapse::Collapse);
    assert_eq!(out, " a b c ");
  }

  #[test]
  fn test_white_space_preserve_spaces() {
    let input = "a \n b";
    let out = apply_white_space_collapse(input, WhiteSpaceCollapse::PreserveSpaces);
    // line break should be replaced with a single space; existing spaces preserved
    assert_eq!(out, "a  b");
  }

  #[test]
  fn test_white_space_preserve_breaks() {
    let input = "a \n b\tc";
    let out = apply_white_space_collapse(input, WhiteSpaceCollapse::PreserveBreaks);
    // spaces and tabs collapsed to single space, line break preserved
    assert_eq!(out, "a \nb c");
  }
}
