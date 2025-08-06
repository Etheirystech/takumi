use std::io::Cursor;

use cosmic_text::{LayoutGlyph, PhysicalGlyph, rustybuzz::Face, ttf_parser::GlyphId};
use image::{ImageFormat, ImageReader, RgbaImage};

#[derive(Clone)]
pub struct CachedGlyph(CachedGlyphContent);

#[derive(Clone)]
enum CachedGlyphContent {
  AlphaMask(Vec<(u32, u32, u8)>),
  Image(RgbaImage),
}

impl CachedGlyph {
  pub fn from_glyph(glyph: &LayoutGlyph, face: &Face) -> Self {
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

      return CachedGlyph(CachedGlyphContent::Image(decoded.into_rgba8()));
    }

    CachedGlyph(CachedGlyphContent::AlphaMask(vec![]))
  }
}
