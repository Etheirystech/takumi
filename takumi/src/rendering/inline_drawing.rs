use image::{GenericImageView, Rgba, RgbaImage};
use parley::style::FontWeight as ParleyFontWeight;
use parley::{FontWidth, GlyphRun, LineMetrics, PositionedInlineBox, PositionedLayoutItem};
use swash::FontRef;
use taffy::{AvailableSpace, Layout, Point, Size};

use crate::{
  Result,
  layout::{
    inline::{
      InlineBoxItem, InlineBrush, InlineLayout, InlineLayoutStage, ProcessedInlineSpan,
      create_inline_layout,
    },
    node::Node,
    style::{Affine, BackgroundClip, Overflow, SizedFontStyle, TextDecorationLine},
  },
  rendering::{
    BackgroundTile, BorderProperties, Canvas, CanvasConstrain, CanvasConstrainResult, DrawPhase,
    MaxHeight, RenderContext, TextClipBackground, apply_transform, collect_background_layers,
    draw_decoration, draw_glyph, draw_glyph_clip_image, rasterize_layers,
  },
  resources::font::FontError,
};

#[allow(clippy::too_many_arguments)]
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
  // Use parley/fontique's synthesis info for faux italic — it knows per-run
  // whether the matched font lacks italic and needs synthetic slant.
  let faux_italic_skew = run
    .synthesis()
    .skew()
    .map(|deg| deg.to_radians().tan())
    .unwrap_or(0.0);

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
        faux_italic_skew,
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
      faux_italic_skew,
    )?;
  }

  Ok(())
}

#[inline]
fn metric_neutral_y_offset(actual_height: f32, parley_height: f32) -> f32 {
  // Metric-neutral boxes are positioned against text-top in fix_inline_box_y,
  // so they don't need the baseline compensation used for regular inline boxes.
  if parley_height < 0.5 {
    return 0.0;
  }

  (actual_height - parley_height).max(0.0)
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

  // Offset by margin to translate from margin box origin (parley's position)
  // to border box origin (where backgrounds, borders, and content are drawn).
  let mut context = RenderContext {
    transform: transform
      * Affine::translation(
        inline_box.x + item.margin.left,
        // For metric-neutral inline-boxes (parley height = 0, actual render height > 0),
        // draw using the actual box height so visual layers don't affect parent line metrics.
        inline_box.y + item.margin.top
          - metric_neutral_y_offset(item.inline_box.height, inline_box.height),
      ),
    ..item.context.clone()
  };
  let layout: Layout = item.into();

  // Apply the inline-block element's own CSS transform (scale, rotate, translate).
  // Without this, CSS `transform: scale(...)` on inline-block elements is ignored
  // because they don't go through `render_node` which normally handles transforms.
  apply_transform(
    &mut context.transform,
    &context.style,
    layout.size,
    &context.sizing,
  );

  // Apply clip-path / mask-image / overflow constraints (same as render.rs)
  let constrain = CanvasConstrain::from_node(
    &context,
    &context.style,
    layout,
    context.transform,
    &mut canvas.mask_memory,
  )?;

  if matches!(constrain, CanvasConstrainResult::SkipRendering) {
    return Ok(());
  }

  let has_constrain = constrain.is_some();

  match constrain {
    CanvasConstrainResult::None => {
      item.node.draw_outset_box_shadow(&context, canvas, layout)?;
      item.node.draw_background(&context, canvas, layout)?;
      item.node.draw_inset_box_shadow(&context, canvas, layout)?;
      item.node.draw_border(&context, canvas, layout)?;
      item.node.draw_outline(&context, canvas, layout)?;
    }
    CanvasConstrainResult::Some(constrain) => match constrain {
      CanvasConstrain::ClipPath { .. } | CanvasConstrain::MaskImage { .. } => {
        canvas.push_constrain(constrain);
        item.node.draw_outset_box_shadow(&context, canvas, layout)?;
        item.node.draw_background(&context, canvas, layout)?;
        item.node.draw_inset_box_shadow(&context, canvas, layout)?;
        item.node.draw_border(&context, canvas, layout)?;
        item.node.draw_outline(&context, canvas, layout)?;
      }
      CanvasConstrain::Overflow { .. } => {
        item.node.draw_outset_box_shadow(&context, canvas, layout)?;
        item.node.draw_background(&context, canvas, layout)?;
        item.node.draw_inset_box_shadow(&context, canvas, layout)?;
        item.node.draw_border(&context, canvas, layout)?;
        item.node.draw_outline(&context, canvas, layout)?;
        canvas.push_constrain(constrain);
      }
    },
    CanvasConstrainResult::SkipRendering => unreachable!(),
  }

  item.node.draw_content(&context, canvas, layout)?;

  // For inline-block nodes, draw the internal inline layout of their children.
  // Abs-pos children are rendered FIRST so they appear behind in-flow text
  // (they serve as background layers, e.g., trigger badge trapezoid).
  if let Some(node_tree) = item.node_tree {
    if let Some(abs_children) = &node_tree.abs_pos_children {
      render_abs_pos_children(abs_children, &context, canvas, layout)?;
    }

    if node_tree.should_create_inline_layout() {
      draw_inline_block_content(node_tree, &context, canvas, layout)?;
    }
  }

  if has_constrain {
    canvas.pop_constrain();
  }

  Ok(())
}

/// Draws the internal inline layout of an inline-block element.
/// This creates a fresh parley layout from the inline-block's children and renders it.
fn draw_inline_block_content<'g, N: Node<N>>(
  node_tree: &crate::layout::tree::NodeTree<'g, N>,
  positioned_context: &RenderContext<'g>,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let font_style = node_tree
    .context
    .style
    .to_sized_font_style(&node_tree.context);

  // Only impose a max height constraint when the element uses overflow: hidden.
  // overflow: hidden should limit line breaking (content that doesn't fit is dropped).
  // overflow: clip only clips visually during rendering (canvas constrain) but
  // should NOT limit line breaking — matching CSS spec where overflow:clip preserves
  // content-based sizing. overflow: visible allows text to overflow freely.
  let overflow = node_tree.context.style.resolve_overflows();
  let limits_content = overflow.x == Overflow::Hidden || overflow.y == Overflow::Hidden;

  let max_height = match (limits_content, font_style.parent.line_clamp.as_ref()) {
    (true, Some(clamp)) => Some(MaxHeight::HeightAndLines(
      layout.content_box_height(),
      clamp.count,
    )),
    (true, None) => Some(MaxHeight::Absolute(layout.content_box_height())),
    (false, Some(clamp)) => Some(MaxHeight::Lines(clamp.count)),
    (false, None) => None,
  };

  let (inline_layout, _, spans) = create_inline_layout(
    node_tree.inline_items_iter(),
    Size {
      width: AvailableSpace::Definite(layout.content_box_width()),
      height: AvailableSpace::Definite(layout.content_box_height()),
    },
    layout.content_box_width(),
    max_height,
    &font_style,
    positioned_context.global,
    InlineLayoutStage::Draw,
  );

  let boxes = spans.iter().filter_map(|span| match span {
    ProcessedInlineSpan::Box(item) => Some(item),
    _ => None,
  });

  let positioned_inline_boxes = draw_inline_layout(
    positioned_context,
    canvas,
    layout,
    inline_layout,
    &font_style,
  )?;

  let inline_transform = Affine::translation(
    layout.border.left + layout.padding.left,
    layout.border.top + layout.padding.top,
  ) * positioned_context.transform;

  for (item, positioned) in boxes.zip(positioned_inline_boxes.iter()) {
    draw_inline_box(positioned, item, canvas, inline_transform)?;
  }

  Ok(())
}

/// Renders absolutely-positioned children that were separated from in-flow children.
/// Uses a temporary taffy tree to compute each child's layout (size + position),
/// then renders using the NodeTree's drawing methods.
pub(crate) fn render_abs_pos_children<'g, N: Node<N>>(
  children: &[crate::layout::tree::NodeTree<'g, N>],
  parent_context: &RenderContext<'g>,
  canvas: &mut Canvas,
  parent_layout: Layout,
) -> Result<()> {
  use taffy::{TaffyTree, style::Dimension};

  // CSS spec: the containing block for abs-pos children of a positioned
  // ancestor is the padding box (border-box minus borders).
  let padding_width =
    parent_layout.size.width - parent_layout.border.left - parent_layout.border.right;
  let padding_height =
    parent_layout.size.height - parent_layout.border.top - parent_layout.border.bottom;

  // Offset from parent's border-box origin to padding-box origin
  let padding_offset_x = parent_layout.border.left;
  let padding_offset_y = parent_layout.border.top;

  for child in children {
    // Use a temporary taffy tree to compute the abs-pos child's layout.
    // This handles inset resolution (top/left/right/bottom) and sizing correctly.
    let child_taffy_style = child.context.style.to_taffy_style(&child.context);

    let mut temp_taffy: TaffyTree<()> = TaffyTree::new();
    let child_id = temp_taffy.new_leaf(child_taffy_style)?;

    // Container represents the containing block (parent's padding box)
    let container_style = taffy::Style {
      size: Size {
        width: Dimension::length(padding_width),
        height: Dimension::length(padding_height),
      },
      ..Default::default()
    };
    let container_id = temp_taffy.new_with_children(container_style, &[child_id])?;

    temp_taffy.compute_layout(
      container_id,
      Size {
        width: AvailableSpace::Definite(padding_width),
        height: AvailableSpace::Definite(padding_height),
      },
    )?;

    let child_layout = *temp_taffy.layout(child_id)?;

    // Compute the child's world-space transform
    let mut child_transform = parent_context.transform
      * Affine::translation(
        padding_offset_x + child_layout.location.x,
        padding_offset_y + child_layout.location.y,
      );

    // Apply the child's own CSS transform (scale, rotate, translate)
    apply_transform(
      &mut child_transform,
      &child.context.style,
      child_layout.size,
      &child.context.sizing,
    );

    if !child_transform.is_invertible() {
      continue;
    }

    // Create render context with the computed transform
    let child_ctx = RenderContext {
      transform: child_transform,
      ..child.context.clone()
    };

    // Apply clip-path / mask-image / overflow constraints
    let constrain = CanvasConstrain::from_node(
      &child_ctx,
      &child_ctx.style,
      child_layout,
      child_transform,
      &mut canvas.mask_memory,
    )?;

    if matches!(constrain, CanvasConstrainResult::SkipRendering) {
      continue;
    }

    let has_constrain = constrain.is_some();

    // Draw the child's visual shell (background, border, shadows, outline, etc.)
    match constrain {
      CanvasConstrainResult::None => {
        if let Some(ref node) = child.node {
          node.draw_outset_box_shadow(&child_ctx, canvas, child_layout)?;
          node.draw_background(&child_ctx, canvas, child_layout)?;
          node.draw_inset_box_shadow(&child_ctx, canvas, child_layout)?;
          node.draw_border(&child_ctx, canvas, child_layout)?;
          node.draw_outline(&child_ctx, canvas, child_layout)?;
        }
      }
      CanvasConstrainResult::Some(constrain) => match constrain {
        CanvasConstrain::ClipPath { .. } | CanvasConstrain::MaskImage { .. } => {
          canvas.push_constrain(constrain);
          if let Some(ref node) = child.node {
            node.draw_outset_box_shadow(&child_ctx, canvas, child_layout)?;
            node.draw_background(&child_ctx, canvas, child_layout)?;
            node.draw_inset_box_shadow(&child_ctx, canvas, child_layout)?;
            node.draw_border(&child_ctx, canvas, child_layout)?;
            node.draw_outline(&child_ctx, canvas, child_layout)?;
          }
        }
        CanvasConstrain::Overflow { .. } => {
          if let Some(ref node) = child.node {
            node.draw_outset_box_shadow(&child_ctx, canvas, child_layout)?;
            node.draw_background(&child_ctx, canvas, child_layout)?;
            node.draw_inset_box_shadow(&child_ctx, canvas, child_layout)?;
            node.draw_border(&child_ctx, canvas, child_layout)?;
            node.draw_outline(&child_ctx, canvas, child_layout)?;
          }
          canvas.push_constrain(constrain);
        }
      },
      CanvasConstrainResult::SkipRendering => unreachable!(),
    }

    // Draw content (images, etc.)
    if let Some(ref node) = child.node {
      node.draw_content(&child_ctx, canvas, child_layout)?;
    }

    // Render nested abs-pos children first (behind text), then inline content
    if let Some(nested_abs) = &child.abs_pos_children {
      render_abs_pos_children(nested_abs, &child_ctx, canvas, child_layout)?;
    }

    if child.should_create_inline_layout() {
      draw_inline_block_content(child, &child_ctx, canvas, child_layout)?;
    }

    if has_constrain {
      canvas.pop_constrain();
    }
  }

  Ok(())
}

pub(crate) fn draw_inline_layout(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  inline_layout: InlineLayout,
  font_style: &SizedFontStyle,
) -> Result<Vec<PositionedInlineBox>> {
  // clip_offset: extra offset to add when sampling the clip image.
  // Non-zero when the clip image comes from an ancestor and includes area
  // before the child's origin (to accommodate text stroke overflow).
  let (clip_image, clip_offset) = if context.style.background_clip == BackgroundClip::Text {
    // Local background-clip: text — rasterize this element's own background.
    // Extend the clip image by the stroke width so that text stroke extending
    // beyond the element's border box is still covered by the clip image.
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
  } else if let Some(ancestor_clip) = canvas.text_clip_backgrounds.last() {
    // Ancestor has background-clip: text — create a cropped view of the ancestor's background
    match create_ancestor_clip_crop(ancestor_clip, context, layout) {
      Some((image, offset)) => (Some(BackgroundTile::Image(image)), offset),
      None => (None, Point::ZERO),
    }
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

  // Determine which phases to run. When the canvas has a text_draw_phase set
  // (from an ancestor with background-clip: text doing two-pass child rendering),
  // only execute that single phase. Otherwise do both phases locally.
  let phases: &[DrawPhase] = match canvas.text_draw_phase {
    Some(phase) => match phase {
      DrawPhase::Stroke => &[DrawPhase::Stroke],
      DrawPhase::Fill => &[DrawPhase::Fill],
    },
    None => &[DrawPhase::Stroke, DrawPhase::Fill],
  };

  // Two-phase rendering across ALL glyph runs: draw all strokes first, then
  // all fills. This matches CSS painting order where text-stroke renders behind
  // text fill, preventing a later run's stroke from covering an earlier run's fill.
  for &phase in phases {
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
            if phase == phases[0] {
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

/// Creates a cropped `RgbaImage` from an ancestor's text clip background.
///
/// Maps the current element's position into the ancestor's coordinate space to
/// determine the crop offset, then copies the relevant rectangle.
///
/// Returns `(image, offset)` where `offset` is the child's origin within the
/// cropped image. Callers must add this offset to sampling coordinates to
/// correctly map child-local positions to clip image positions.
fn create_ancestor_clip_crop(
  ancestor_clip: &TextClipBackground,
  context: &RenderContext,
  _layout: Layout,
) -> Option<(RgbaImage, Point<f32>)> {
  let ancestor_inv = ancestor_clip.transform.invert()?;

  // Compute the child's border-box origin in the ancestor's coordinate space.
  let child_origin = (ancestor_inv * context.transform).transform_point(Point::ZERO);

  // The child's origin within the full ancestor image.
  let offset = Point {
    x: child_origin.x.max(0.0),
    y: child_origin.y.max(0.0),
  };

  Some((ancestor_clip.image.clone(), offset))
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
