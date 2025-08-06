use std::io::Cursor;

use cosmic_text::{
  Attrs, Buffer, LayoutGlyph, Metrics, PhysicalGlyph, Shaping,
  rustybuzz::Face,
  ttf_parser::{self, GlyphId},
};
use image::{
  ImageFormat, ImageReader, Rgba, RgbaImage,
  imageops::{FilterType, resize},
};
use taffy::{Layout, Point, Size};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

use crate::{
  Color, ColorInput, SidesValue,
  core::RenderContext,
  effects::{BorderProperties, draw_border},
  rendering::FastBlendImage,
  style::{ResolvedFontStyle, TextOverflow},
};

const ELLIPSIS_CHAR: &str = "…";

/// Draws text on the canvas with the specified font style and layout.
pub fn draw_text(
  text: &str,
  style: &ResolvedFontStyle,
  context: &RenderContext,
  canvas: &mut FastBlendImage,
  layout: Layout,
) {
  if style.color.is_transparent() || style.font_size == 0.0 {
    return;
  }

  let content_box = layout.content_box_size();

  let start_x = layout.content_box_x();
  let start_y = layout.content_box_y();

  let buffer = construct_text_buffer(
    text,
    style,
    context,
    Some((Some(content_box.width), Some(content_box.height))),
  );

  let Some(last_run) = buffer.layout_runs().last() else {
    return;
  };

  let Some(last_glyph) = last_run.glyphs.last() else {
    return;
  };

  let should_append_ellipsis =
    style.text_overflow == TextOverflow::Ellipsis && last_glyph.end < text.len();

  if should_append_ellipsis {
    let first_glyph = last_run.glyphs.first().unwrap();
    let mut truncated_text = &text[first_glyph.start..last_glyph.end];

    while !truncated_text.is_empty() {
      let mut text_with_ellipsis =
        String::with_capacity(truncated_text.len() + ELLIPSIS_CHAR.len());

      text_with_ellipsis.push_str(truncated_text);
      text_with_ellipsis.push_str(ELLIPSIS_CHAR);

      let truncated_buffer = construct_text_buffer(&text_with_ellipsis, style, context, None);

      let last_line = truncated_buffer.layout_runs().last().unwrap();

      if last_line.line_w <= content_box.width {
        break;
      }

      truncated_text = &truncated_text[..truncated_text.len() - ELLIPSIS_CHAR.len()];
    }

    let before_last_line = &text[..first_glyph.start];

    let mut text_with_ellipsis =
      String::with_capacity(before_last_line.len() + truncated_text.len() + ELLIPSIS_CHAR.len());

    text_with_ellipsis.push_str(before_last_line);
    text_with_ellipsis.push_str(truncated_text);
    text_with_ellipsis.push_str(ELLIPSIS_CHAR);

    return draw_text(&text_with_ellipsis, style, context, canvas, layout);
  }

  draw_buffer(
    context,
    &buffer,
    canvas,
    content_box,
    &style.color,
    (start_x, start_y),
  );
}

fn draw_buffer(
  context: &RenderContext,
  buffer: &Buffer,
  canvas: &mut FastBlendImage,
  content_box: Size<f32>,
  color: &ColorInput,
  (start_x, start_y): (f32, f32),
) {
  let mut font_system = context.global.font_context.font_system.lock().unwrap();

  let pixmap_width = content_box.width.ceil() as u32;
  let pixmap_height = content_box.height.ceil() as u32;

  if pixmap_width == 0 || pixmap_height == 0 {
    return;
  }

  let mut pixmap = Pixmap::new(pixmap_width, pixmap_height).unwrap();
  let mut paint = Paint::default();

  if let ColorInput::Color(color_value) = color {
    let rgba: Rgba<u8> = (*color_value).into();
    let [r, g, b, a] = rgba.0;

    paint.set_color(tiny_skia::Color::from_rgba8(r, g, b, a));
  } else {
    paint.set_color(tiny_skia::Color::BLACK);
  }
  paint.anti_alias = true;

  let mut render_glyphs = vec![];

  for run in buffer.layout_runs() {
    for glyph in run.glyphs.iter() {
      let physical_glyph = glyph.physical((0., 0.), 1.0);

      if let Some(cached) = context
        .global
        .font_context
        .get_cached_glyph(&physical_glyph.cache_key)
      {
        canvas.overlay_image(
          &cached,
          (start_x + glyph.x as f32) as u32,
          (start_y + run.line_top + glyph.y as f32) as u32,
        );
        continue;
      }

      render_glyphs.push(glyph);
    }
  }

  for run in buffer.layout_runs() {
    for glyph in run.glyphs.iter() {
      let physical_glyph = glyph.physical((0., 0.), 1.0);

      let font = font_system.get_font(glyph.font_id).unwrap();

      let face = font.rustybuzz();

      // Compute consistent glyph position once for both bitmap and outline glyphs
      let glyph_x = physical_glyph.x as f32;
      let glyph_y = run.line_y + physical_glyph.y as f32;

      // Prefer a bitmap strike if available; resize with rounded scaling to avoid truncation
      if let Some(image) = face.glyph_raster_image(GlyphId(glyph.glyph_id), 16) {
        let decoded = ImageReader::with_format(Cursor::new(image.data), ImageFormat::Png)
          .with_guessed_format()
          .unwrap()
          .decode()
          .unwrap();

        let scale = glyph.w / image.width as f32;

        let resized = resize(
          &decoded,
          (image.width as f32 * scale) as u32,
          (image.height as f32 * scale) as u32,
          FilterType::CatmullRom,
        );

        canvas.overlay_image(
          &resized,
          (start_x + glyph_x) as u32,
          (start_y + run.line_top) as u32,
        );

        if context.global.draw_debug_border {
          draw_border(
            canvas,
            BorderProperties {
              width: SidesValue::SingleValue(1.0).into(),
              offset: Point {
                x: start_x + glyph_x,
                y: start_y + run.line_top,
              },
              size: Size {
                width: resized.width() as f32,
                height: resized.height() as f32,
              },
              color: Color::Rgb(0, 0, 255).into(),
              radius: None,
            },
          );
        }

        continue;
      }

      let scale = glyph.font_size / face.units_per_em() as f32;
      let mut outline_builder = OutlineBuilder::new(scale);

      if face
        .outline_glyph(GlyphId(glyph.glyph_id), &mut outline_builder)
        .is_none()
      {
        continue;
      }

      let Some(path) = outline_builder.builder.finish() else {
        continue;
      };

      if context.global.draw_debug_border {
        let bounds = path.bounds();
        draw_border(
          canvas,
          BorderProperties {
            width: SidesValue::SingleValue(1.0).into(),
            offset: Point {
              x: start_x + glyph_x + bounds.left(),
              y: start_y + run.line_y + bounds.top(),
            },
            size: Size {
              width: bounds.width(),
              height: bounds.height(),
            },
            color: Color::Rgb(0, 0, 255).into(),
            radius: None,
          },
        );
      }

      // Create transform for glyph positioning
      let transform = Transform::from_translate(glyph_x, glyph_y);

      // Draw the glyph path to the main pixmap
      pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
    }
  }

  // Convert the entire pixmap to image and overlay once
  let image = RgbaImage::from_raw(pixmap.width(), pixmap.height(), pixmap.take()).unwrap();
  canvas.overlay_image(&image, start_x as u32, start_y as u32);
}

struct OutlineBuilder {
  builder: PathBuilder,
  scale: f32,
}

impl OutlineBuilder {
  pub fn new(scale: f32) -> Self {
    OutlineBuilder {
      builder: PathBuilder::new(),
      scale,
    }
  }

  fn scale_point(&self, x: f32, y: f32) -> (f32, f32) {
    (x * self.scale, -y * self.scale) // Consistent Y-flipping
  }
}

impl ttf_parser::OutlineBuilder for OutlineBuilder {
  fn move_to(&mut self, x: f32, y: f32) {
    let (sx, sy) = self.scale_point(x, y);
    self.builder.move_to(sx, sy);
  }

  fn line_to(&mut self, x: f32, y: f32) {
    let (sx, sy) = self.scale_point(x, y);
    self.builder.line_to(sx, sy);
  }

  fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
    let (sx1, sy1) = self.scale_point(x1, y1);
    let (sx, sy) = self.scale_point(x, y);
    self.builder.quad_to(sx1, sy1, sx, sy);
  }

  fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
    let (sx1, sy1) = self.scale_point(x1, y1);
    let (sx2, sy2) = self.scale_point(x2, y2);
    let (sx, sy) = self.scale_point(x, y);
    self.builder.cubic_to(sx1, sy1, sx2, sy2, sx, sy);
  }

  fn close(&mut self) {
    self.builder.close();
  }
}

pub(crate) fn construct_text_buffer(
  text: &str,
  font_style: &ResolvedFontStyle,
  context: &RenderContext,
  size: Option<(Option<f32>, Option<f32>)>,
) -> Buffer {
  let metrics = Metrics::new(font_style.font_size, font_style.line_height);
  let mut buffer = Buffer::new_empty(metrics);

  let mut attrs = Attrs::new().weight(font_style.font_weight);

  if let Some(font_family) = font_style.font_family.as_ref() {
    attrs = attrs.family(font_family.as_family());
  }

  if let Some(letter_spacing) = font_style.letter_spacing {
    attrs = attrs.letter_spacing(letter_spacing);
  }

  let mut font_system = context.global.font_context.font_system.lock().unwrap();

  if let Some((width, height)) = size {
    buffer.set_size(&mut font_system, width, height);
  }

  buffer.set_rich_text(
    &mut font_system,
    [(text, attrs.clone())],
    &attrs,
    Shaping::Advanced,
    font_style.text_align,
  );

  buffer
}
