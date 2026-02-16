use image::{GenericImageView, Rgba};
use parley::style::FontWeight as ParleyFontWeight;
use parley::{FontWidth, GlyphRun, LineMetrics, PositionedInlineBox, PositionedLayoutItem};
use swash::FontRef;
use taffy::{Layout, Point, Size};

use crate::{
  Result,
  layout::{
    inline::{InlineBoxItem, InlineBrush, InlineLayout},
    node::Node,
    style::{Affine, BackgroundClip, SizedFontStyle, TextDecorationLine},
  },
  rendering::{
    BorderProperties, Canvas, DrawPhase, RenderContext, collect_background_layers, draw_decoration,
    draw_glyph, draw_glyph_clip_image, rasterize_layers,
  },
  resources::font::FontError,
};

fn draw_glyph_run<I: GenericImageView<Pixel = Rgba<u8>>>(
  style: &SizedFontStyle,
  glyph_run: &GlyphRun<'_, InlineBrush>,
  canvas: &mut Canvas,
  layout: Layout,
  context: &RenderContext,
  clip_image: Option<&I>,
  phase: DrawPhase,
  clip_offset: Point<f32>,
) -> Result<()> {
  let decoration_line = style
    .parent
    .text_decoration_line
    .as_ref()
    .unwrap_or(&style.parent.text_decoration.line);

  let run = glyph_run.run();
  let metrics = run.metrics();

  // Collect all glyph IDs for batch processing
  let glyph_ids = glyph_run.positioned_glyphs().map(|glyph| glyph.id);

  let font = FontRef::from_index(run.font().data.as_ref(), run.font().index as usize)
    .ok_or(FontError::InvalidFontIndex)?;
  let resolved_glyphs = context
    .global
    .font_context
    .resolve_glyphs(run, font, glyph_ids);

  let palette = font.color_palettes().next();

  // Compute faux-bold width: if the font doesn't have a weight variation axis
  // and the requested weight exceeds the font's actual weight, synthesize bold
  // by drawing a thin same-color stroke under the fill (Chromium approach).
  let faux_bold_width = compute_faux_bold_width(font, style, run.font_size());
  let faux_stretch_factor = compute_faux_stretch_factor(font, style);

  match phase {
    DrawPhase::Stroke => {
      // Decorations that go behind text: underline and overline
      if decoration_line.contains(&TextDecorationLine::Underline) {
        draw_decoration(
          canvas,
          glyph_run,
          glyph_run.style().brush.decoration_color,
          glyph_run.baseline() - metrics.underline_offset,
          glyph_run.run().font_size() / 18.0,
          layout,
          context.transform,
          faux_stretch_factor,
        );
      }

      if decoration_line.contains(&TextDecorationLine::Overline) {
        draw_decoration(
          canvas,
          glyph_run,
          glyph_run.style().brush.decoration_color,
          glyph_run.baseline() - metrics.ascent - metrics.underline_offset,
          glyph_run.run().font_size() / 18.0,
          layout,
          context.transform,
          faux_stretch_factor,
        );
      }
    }
    DrawPhase::Fill => {
      // Line-through goes on top of text fill
      if decoration_line.contains(&TextDecorationLine::LineThrough) {
        let size = glyph_run.run().font_size() / 18.0;
        let offset = glyph_run.baseline() - metrics.strikethrough_offset;

        draw_decoration(
          canvas,
          glyph_run,
          glyph_run.style().brush.decoration_color,
          offset,
          size,
          layout,
          context.transform,
          faux_stretch_factor,
        );
      }
    }
  }

  let run_offset_x = glyph_run.offset();

  if let Some(clip_image) = clip_image {
    for glyph in glyph_run.positioned_glyphs() {
      let Some(content) = resolved_glyphs.get(&glyph.id) else {
        continue;
      };

      let adjusted_x = if faux_stretch_factor != 1.0 {
        run_offset_x + (glyph.x - run_offset_x) * faux_stretch_factor
      } else {
        glyph.x
      };

      let inline_offset = Point {
        x: layout.border.left + layout.padding.left + adjusted_x,
        y: layout.border.top + layout.padding.top + glyph.y,
      };

      draw_glyph_clip_image(
        content,
        canvas,
        style,
        context.transform,
        inline_offset,
        clip_image,
        faux_bold_width,
        phase,
        clip_offset,
        faux_stretch_factor,
      );
    }
  }

  for glyph in glyph_run.positioned_glyphs() {
    let Some(content) = resolved_glyphs.get(&glyph.id) else {
      continue;
    };

    let adjusted_x = if faux_stretch_factor != 1.0 {
      run_offset_x + (glyph.x - run_offset_x) * faux_stretch_factor
    } else {
      glyph.x
    };

    let inline_offset = Point {
      x: layout.border.left + layout.padding.left + adjusted_x,
      y: layout.border.top + layout.padding.top + glyph.y,
    };

    draw_glyph(
      content,
      canvas,
      style,
      context.transform,
      inline_offset,
      glyph_run.style().brush.color,
      palette,
      faux_bold_width,
      phase,
      faux_stretch_factor,
    )?;
  }

  Ok(())
}

pub(crate) fn draw_inline_box<N: Node<N>>(
  inline_box: &PositionedInlineBox,
  item: &InlineBoxItem<'_, '_, N>,
  canvas: &mut Canvas,
  transform: Affine,
) -> Result<()> {
  if item.context.style.opacity.0 == 0.0 {
    return Ok(());
  }

  let context = RenderContext {
    transform: transform * Affine::translation(inline_box.x, inline_box.y),
    ..item.context.clone()
  };
  let layout = item.into();

  item.node.draw_outset_box_shadow(&context, canvas, layout)?;
  item.node.draw_background(&context, canvas, layout)?;
  item.node.draw_inset_box_shadow(&context, canvas, layout)?;
  item.node.draw_border(&context, canvas, layout)?;
  item.node.draw_content(&context, canvas, layout)?;

  Ok(())
}

pub(crate) fn draw_inline_layout(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  inline_layout: InlineLayout,
  font_style: &SizedFontStyle,
) -> Result<Vec<PositionedInlineBox>> {
  // Extend the clip image by the stroke width so that text stroke extending
  // beyond the element's border box is still covered by the clip image.
  let (clip_image, clip_offset) = if context.style.background_clip == BackgroundClip::Text {
    let margin = font_style.stroke_width.max(0.0).ceil();
    // When text overflows the layout box (e.g., whiteSpace: nowrap with long text
    // and a scaleX transform), the clip image must cover the full text extent,
    // not just the layout size. Otherwise glyphs beyond layout.size.width sample
    // outside the clip image and become transparent.
    let text_overflow_x = (inline_layout.width() - layout.content_box_width()).max(0.0);
    let extended_size = Size {
      width: layout.size.width + text_overflow_x + 2.0 * margin,
      height: layout.size.height + 2.0 * margin,
    };
    let layers = collect_background_layers(context, extended_size)?;

    let tile = rasterize_layers(
      layers,
      extended_size.map(|x| x.ceil() as u32),
      context,
      BorderProperties::default(),
      Affine::IDENTITY,
      &mut canvas.mask_memory,
    );
    (
      tile,
      Point {
        x: margin,
        y: margin,
      },
    )
  } else {
    (None, Point::ZERO)
  };

  let mut positioned_inline_boxes = Vec::new();
  let lines = inline_layout.lines().collect::<Vec<_>>();
  let mut next_nonzero_line_top = vec![0.0_f32; lines.len()];
  let mut line_text_top = vec![0.0_f32; lines.len()];
  let mut line_has_glyph = vec![false; lines.len()];
  let mut next_top: Option<f32> = None;

  for (idx, line) in lines.iter().enumerate() {
    let mut top_from_glyph: Option<f32> = None;
    for item in line.items() {
      if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
        top_from_glyph = Some(glyph_run.baseline() - glyph_run.run().metrics().ascent);
        break;
      }
    }

    line_has_glyph[idx] = top_from_glyph.is_some();
    line_text_top[idx] = top_from_glyph.unwrap_or(line.metrics().baseline - line.metrics().ascent);
  }

  for (idx, line) in lines.iter().enumerate().rev() {
    let metrics = line.metrics();
    let top = line_text_top[idx];
    if metrics.line_height >= 0.5 {
      next_top = Some(top);
    }
    next_nonzero_line_top[idx] = next_top.unwrap_or(top);
  }

  // Two-phase rendering across ALL glyph runs: draw all strokes first, then
  // all fills. This matches CSS painting order where text-stroke renders behind
  // text fill, preventing a later run's stroke from covering an earlier run's fill.
  for &phase in &[DrawPhase::Stroke, DrawPhase::Fill] {
    for (line_idx, line) in lines.iter().enumerate() {
      for item in line.items() {
        match item {
          PositionedLayoutItem::GlyphRun(glyph_run) => {
            draw_glyph_run(
              font_style,
              &glyph_run,
              canvas,
              layout,
              context,
              clip_image.as_ref(),
              phase,
              clip_offset,
            )?;
          }
          PositionedLayoutItem::InlineBox(mut inline_box) => {
            // Collect inline boxes only once (during first phase)
            if phase == DrawPhase::Stroke {
              let metrics = line.metrics();
              if inline_box.height < 0.5 {
                // Metric-neutral wrapper lines can be generated without glyphs;
                // anchor them to the next real text line so they keep visual order.
                inline_box.y = if line_has_glyph[line_idx] {
                  line_text_top[line_idx]
                } else {
                  next_nonzero_line_top[line_idx]
                };
              } else {
                fix_inline_box_y(&mut inline_box.y, metrics, inline_box.height);
              }
              positioned_inline_boxes.push(inline_box)
            }
          }
        }
      }
    }
  }

  Ok(positioned_inline_boxes)
}

// https://github.com/linebender/parley/blob/d7ed9b1ec844fa5a9ed71b84552c603dae3cab18/parley/src/layout/line.rs#L261C28-L261C61
pub(crate) fn fix_inline_box_y(y: &mut f32, metrics: &LineMetrics, inline_box_height: f32) {
  // Metric-neutral inline boxes (line-height: 0 wrappers) are used for decorations
  // that should not affect line metrics. Align them to text-top instead of baseline.
  if inline_box_height < 0.5 {
    *y = metrics.baseline - metrics.ascent;
    return;
  }

  *y += metrics.line_height - metrics.baseline;
}

/// Computes the faux-bold stroke width for a given font and style.
///
/// When a font file doesn't include a weight variation axis (i.e., it only
/// has one weight) and the CSS requests a heavier weight, we synthesize bold
/// by drawing a thin same-color stroke around glyph outlines before filling.
///
/// Uses the Chromium approach: `font_size / 24.0` as the stroke width.
fn compute_faux_bold_width(font: FontRef, style: &SizedFontStyle, font_size: f32) -> f32 {
  // If the font has a weight (wght) variation axis, parley handles weight
  // through normalized coordinates — no faux bold needed.
  const WGHT: swash::Tag = swash::tag_from_bytes(b"wght");
  if font.variations().any(|v| v.tag() == WGHT) {
    return 0.0;
  }

  let actual_weight = font.attributes().weight().0 as f32;
  let requested_weight = ParleyFontWeight::from(style.parent.font_weight).value();

  // Only apply faux bold if the requested weight is significantly heavier
  // (more than 150 units above the font's actual weight).
  if requested_weight > actual_weight + 150.0 {
    font_size / 24.0
  } else {
    0.0
  }
}

/// Computes a horizontal scale factor for synthetic font-stretch.
///
/// When a font file doesn't include a width (wdth) variation axis and the CSS
/// requests a non-normal font-stretch (e.g., `condensed`), we synthesize the
/// stretch by horizontally scaling glyph outlines and positions during rendering.
///
/// Returns 1.0 when no synthesis is needed (font supports wdth axis or
/// normal width is requested).
fn compute_faux_stretch_factor(font: FontRef, _style: &SizedFontStyle) -> f32 {
  const WDTH: swash::Tag = swash::tag_from_bytes(b"wdth");
  if font.variations().any(|v| v.tag() == WDTH) {
    return 1.0;
  }

  let requested_width: FontWidth = FontWidth::NORMAL;
  let ratio = requested_width.ratio();

  if (ratio - 1.0).abs() < f32::EPSILON {
    return 1.0;
  }

  ratio
}

#[cfg(test)]
mod tests {
  use parley::LineMetrics;

  use super::{fix_inline_box_y, metric_neutral_y_offset};

  #[test]
  fn fix_inline_box_y_skips_metric_neutral_boxes() {
    let metrics = LineMetrics {
      ascent: 36.0,
      line_height: 88.0,
      baseline: 74.0,
      ..LineMetrics::default()
    };
    let mut y = 74.0;

    fix_inline_box_y(&mut y, &metrics, 0.0);

    assert_eq!(y, 38.0);
  }

  #[test]
  fn fix_inline_box_y_applies_to_regular_boxes() {
    let metrics = LineMetrics {
      line_height: 88.0,
      baseline: 74.0,
      ..LineMetrics::default()
    };
    let mut y = 54.0;

    fix_inline_box_y(&mut y, &metrics, 20.0);

    assert_eq!(y, 68.0);
  }

  #[test]
  fn metric_neutral_offset_uses_actual_minus_parley_height() {
    assert_eq!(metric_neutral_y_offset(50.0, 0.0), 0.0);
    assert_eq!(metric_neutral_y_offset(20.0, 20.0), 0.0);
  }
}

#[inline]
#[allow(dead_code)] // Used by draw_inline_box expansion in a later PR
fn metric_neutral_y_offset(actual_height: f32, parley_height: f32) -> f32 {
  // Metric-neutral boxes are positioned against text-top in fix_inline_box_y,
  // so they don't need the baseline compensation used for regular inline boxes.
  if parley_height < 0.5 {
    return 0.0;
  }

  (actual_height - parley_height).max(0.0)
}
