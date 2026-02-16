use image::{GenericImageView, Rgba};
use parley::{GlyphRun, LineMetrics, PositionedInlineBox, PositionedLayoutItem};
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
    BorderProperties, Canvas, CanvasConstrain, CanvasConstrainResult, MaxHeight, RenderContext,
    apply_transform, collect_background_layers, draw_decoration, draw_glyph, draw_glyph_clip_image,
    rasterize_layers,
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
    }
    CanvasConstrainResult::Some(constrain) => match constrain {
      CanvasConstrain::ClipPath { .. } | CanvasConstrain::MaskImage { .. } => {
        canvas.push_constrain(constrain);
        item.node.draw_outset_box_shadow(&context, canvas, layout)?;
        item.node.draw_background(&context, canvas, layout)?;
        item.node.draw_inset_box_shadow(&context, canvas, layout)?;
        item.node.draw_border(&context, canvas, layout)?;
      }
      CanvasConstrain::Overflow { .. } => {
        item.node.draw_outset_box_shadow(&context, canvas, layout)?;
        item.node.draw_background(&context, canvas, layout)?;
        item.node.draw_inset_box_shadow(&context, canvas, layout)?;
        item.node.draw_border(&context, canvas, layout)?;
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

    // Draw the child's visual shell (background, border, shadows, etc.)
    match constrain {
      CanvasConstrainResult::None => {
        if let Some(ref node) = child.node {
          node.draw_outset_box_shadow(&child_ctx, canvas, child_layout)?;
          node.draw_background(&child_ctx, canvas, child_layout)?;
          node.draw_inset_box_shadow(&child_ctx, canvas, child_layout)?;
          node.draw_border(&child_ctx, canvas, child_layout)?;
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
          }
        }
        CanvasConstrain::Overflow { .. } => {
          if let Some(ref node) = child.node {
            node.draw_outset_box_shadow(&child_ctx, canvas, child_layout)?;
            node.draw_background(&child_ctx, canvas, child_layout)?;
            node.draw_inset_box_shadow(&child_ctx, canvas, child_layout)?;
            node.draw_border(&child_ctx, canvas, child_layout)?;
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
          fix_inline_box_y(&mut inline_box.y, line.metrics(), inline_box.height);
          positioned_inline_boxes.push(inline_box)
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
