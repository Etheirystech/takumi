use image::{GenericImageView, Rgba};
use parley::{GlyphRun, LineMetrics, PositionedInlineBox, PositionedLayoutItem};
use swash::FontRef;
use taffy::{Layout, Point};

use crate::{
  Result,
  layout::{
    inline::{InlineBoxItem, InlineBrush, InlineLayout},
    node::Node,
    style::{Affine, BackgroundClip, SizedFontStyle, TextDecorationLine},
  },
  rendering::{
    BorderProperties, Canvas, RenderContext, collect_background_layers, draw_decoration,
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
) -> Result<()> {
  let decoration_line = style
    .parent
    .text_decoration_line
    .as_ref()
    .unwrap_or(&style.parent.text_decoration.line);

  let run = glyph_run.run();
  let metrics = run.metrics();

  // decoration underline should not overlap with the glyph descent part,
  // as a temporary workaround, we draw the decoration under the glyph.
  if decoration_line.contains(&TextDecorationLine::Underline) {
    draw_decoration(
      canvas,
      glyph_run,
      glyph_run.style().brush.decoration_color,
      glyph_run.baseline() - metrics.underline_offset,
      glyph_run.run().font_size() / 18.0,
      layout,
      context.transform,
    );
  }

  // Collect all glyph IDs for batch processing
  let glyph_ids = glyph_run.positioned_glyphs().map(|glyph| glyph.id);

  let font = FontRef::from_index(run.font().data.as_ref(), run.font().index as usize)
    .ok_or(FontError::InvalidFontIndex)?;
  let resolved_glyphs = context
    .global
    .font_context
    .resolve_glyphs(run, font, glyph_ids);

  let palette = font.color_palettes().next();

  if let Some(clip_image) = clip_image {
    for glyph in glyph_run.positioned_glyphs() {
      let Some(content) = resolved_glyphs.get(&glyph.id) else {
        continue;
      };

      let inline_offset = Point {
        x: layout.border.left + layout.padding.left + glyph.x,
        y: layout.border.top + layout.padding.top + glyph.y,
      };

      draw_glyph_clip_image(
        content,
        canvas,
        style,
        context.transform,
        inline_offset,
        clip_image,
      );
    }
  }

  for glyph in glyph_run.positioned_glyphs() {
    let Some(content) = resolved_glyphs.get(&glyph.id) else {
      continue;
    };

    let inline_offset = Point {
      x: layout.border.left + layout.padding.left + glyph.x,
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
    )?;
  }

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
    );
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
  item.node.draw_outline(&context, canvas, layout)?;

  Ok(())
}

pub(crate) fn draw_inline_layout(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  inline_layout: InlineLayout,
  font_style: &SizedFontStyle,
) -> Result<Vec<PositionedInlineBox>> {
  let clip_image = if context.style.background_clip == BackgroundClip::Text {
    let layers = collect_background_layers(context, layout.size)?;

    rasterize_layers(
      layers,
      layout.size.map(|x| x as u32),
      context,
      BorderProperties::default(),
      Affine::IDENTITY,
      &mut canvas.mask_memory,
    )
  } else {
    None
  };

  let mut positioned_inline_boxes = Vec::new();

  for line in inline_layout.lines() {
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
          )?;
        }
        PositionedLayoutItem::InlineBox(mut inline_box) => {
          fix_inline_box_y(&mut inline_box.y, line.metrics());
          positioned_inline_boxes.push(inline_box)
        }
      }
    }
  }

  Ok(positioned_inline_boxes)
}

// https://github.com/linebender/parley/blob/d7ed9b1ec844fa5a9ed71b84552c603dae3cab18/parley/src/layout/line.rs#L261C28-L261C61
pub(crate) fn fix_inline_box_y(y: &mut f32, metrics: &LineMetrics) {
  *y += metrics.line_height - metrics.baseline;
}
