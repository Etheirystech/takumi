use std::{borrow::Cow, io::Write};

use rustc_hash::FxHashMap;

use image::{ExtendedColorType, ImageEncoder, ImageFormat, RgbaImage, codecs::jpeg::JpegEncoder};
use png::{BitDepth, ColorType, Compression, Filter};
use serde::Deserialize;

use image_webp::WebPEncoder;

#[cfg(feature = "rayon")]
use rayon::prelude::*;

use crate::{Error::IoError, rendering::quantize_alpha};

/// Output format for rendered images.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageOutputFormat {
  /// WebP image format, provides good compression and supports animation.
  /// It is useful for images in web contents.
  WebP,

  /// PNG image format, lossless and widely supported, and its the fastest format to encode.
  Png,

  /// JPEG image format, lossy and does not support transparency.
  Jpeg,

  /// SVG vector format, scalable and suitable for text-heavy graphics.
  Svg,
}

impl ImageOutputFormat {
  /// Returns the MIME type for the image output format.
  pub fn content_type(&self) -> &'static str {
    match self {
      ImageOutputFormat::WebP => "image/webp",
      ImageOutputFormat::Png => "image/png",
      ImageOutputFormat::Jpeg => "image/jpeg",
      ImageOutputFormat::Svg => "image/svg+xml",
    }
  }
}

impl From<ImageOutputFormat> for ImageFormat {
  fn from(format: ImageOutputFormat) -> Self {
    match format {
      ImageOutputFormat::WebP => Self::WebP,
      ImageOutputFormat::Png => Self::Png,
      ImageOutputFormat::Jpeg => Self::Jpeg,
      ImageOutputFormat::Svg => Self::Png,
    }
  }
}

/// Represents a single frame of an animated image.
#[derive(Debug, Clone)]
pub struct AnimationFrame {
  /// The image data for the frame.
  pub image: RgbaImage,
  /// The duration of the frame in milliseconds.
  /// Maximum value is 0xffffff (24-bit), overflow will be clamped.
  pub duration_ms: u32,
}

impl AnimationFrame {
  /// Creates a new animation frame.
  pub fn new(image: RgbaImage, duration_ms: u32) -> Self {
    Self { image, duration_ms }
  }
}

const U24_MAX: u32 = 0xffffff;

// Strip alpha channel into a tightly packed RGB buffer
fn strip_alpha_channel(image: &RgbaImage) -> Vec<u8> {
  let raw = image.as_raw();

  let pixel_count = raw.len() / 4 * 3;
  let mut rgb = Vec::with_capacity(pixel_count);

  for chunk in raw.chunks_exact(4) {
    rgb.extend_from_slice(&chunk[..3]);
  }

  rgb
}

fn has_any_alpha_pixel(image: &RgbaImage) -> bool {
  #[cfg(feature = "rayon")]
  {
    image
      .par_pixels()
      .with_min_len(1024)
      .any(|pixel| pixel[3] != u8::MAX)
  }

  #[cfg(not(feature = "rayon"))]
  {
    image.pixels().any(|pixel| pixel[3] != u8::MAX)
  }
}

/// Palette data for indexed PNG.
struct PaletteData {
  /// RGB palette, 3 bytes per color.
  palette: Vec<u8>,
  /// Alpha channel for palette entries, 1 byte per color.
  trns: Vec<u8>,
  /// Indexed pixel data (packed according to bit_depth).
  indices: Vec<u8>,
  /// Optimal bit depth for this palette (1, 2, 4, or 8).
  bit_depth: BitDepth,
}

/// Try to collect a palette from the image.
/// Returns None if there are more than MAX_PALETTE_SIZE unique colors.
fn try_collect_palette(image: &RgbaImage) -> Option<PaletteData> {
  // Pass 1: Collect unique colors with their first occurrence position
  // This preserves spatial locality for better PNG filter compression
  #[cfg(feature = "rayon")]
  let color_positions: FxHashMap<[u8; 4], usize> = {
    use rayon::prelude::*;

    let map = image
      .par_enumerate_pixels()
      .with_min_len(4096)
      .fold(
        || FxHashMap::with_capacity_and_hasher(256, Default::default()),
        |mut acc, (x, y, pixel)| {
          let mut rgba: [u8; 4] = pixel.0;
          rgba[3] = quantize_alpha(rgba[3]);

          // Only insert if not seen before (keeps first occurrence)
          acc
            .entry(rgba)
            .or_insert_with(|| (y as usize) * (image.width() as usize) + (x as usize));

          acc
        },
      )
      .reduce(
        || FxHashMap::with_capacity_and_hasher(256, Default::default()),
        |mut a, b| {
          // Merge keeping the minimum (first) position for each color
          for (color, pos) in b {
            a.entry(color)
              .and_modify(|p| *p = (*p).min(pos))
              .or_insert(pos);
          }
          a
        },
      );

    // Only check total after merge - individual chunks may have duplicates
    if map.len() > 256 {
      return None;
    }

    map
  };

  #[cfg(not(feature = "rayon"))]
  let color_positions: FxHashMap<[u8; 4], usize> = {
    let mut map = FxHashMap::with_capacity_and_hasher(256, Default::default());

    for (idx, pixel) in image.pixels().enumerate() {
      let mut rgba: [u8; 4] = pixel.0;
      rgba[3] = quantize_alpha(rgba[3]);

      if !map.contains_key(&rgba) {
        if map.len() >= 256 {
          return None;
        }
        map.insert(rgba, idx);
      }
    }

    map
  };

  // Sort colors by first occurrence position (preserves spatial locality)
  let mut sorted_colors: Vec<([u8; 4], usize)> = color_positions.into_iter().collect();
  sorted_colors.sort_unstable_by_key(|(_, pos)| *pos);

  // Build palette and color map
  let mut palette = Vec::with_capacity(sorted_colors.len() * 3);
  let mut trns = Vec::with_capacity(sorted_colors.len());
  let mut color_map: FxHashMap<[u8; 4], u8> =
    FxHashMap::with_capacity_and_hasher(sorted_colors.len(), Default::default());

  for (rgba, _) in sorted_colors.iter() {
    let i = color_map.len() as u8;

    color_map.insert(*rgba, i);
    palette.extend_from_slice(&rgba[..3]);

    trns.push(rgba[3]);
  }

  // Determine optimal bit depth based on palette size
  let (bit_depth, bits_per_pixel) = match sorted_colors.len() {
    0..=2 => (BitDepth::One, 1),
    3..=4 => (BitDepth::Two, 2),
    5..=16 => (BitDepth::Four, 4),
    _ => (BitDepth::Eight, 8),
  };

  // Pass 2: Build indices (packed according to bit depth)
  let width = image.width() as usize;
  let pixels_per_byte = 8 / bits_per_pixel;
  let row_bytes = width.div_ceil(pixels_per_byte);

  let mut indices: Vec<u8> = Vec::with_capacity(row_bytes * image.height() as usize);

  for row in image.rows() {
    let mut current_byte: u8 = 0;
    let mut bit_offset = 8 - bits_per_pixel;

    for pixel in row {
      let mut rgba: [u8; 4] = pixel.0;
      rgba[3] = quantize_alpha(rgba[3]);

      let idx = color_map.get(&rgba).copied().unwrap_or(0);

      current_byte |= idx << bit_offset;

      if bit_offset == 0 {
        indices.push(current_byte);
        current_byte = 0;
        bit_offset = 8 - bits_per_pixel;
      } else {
        bit_offset -= bits_per_pixel;
      }
    }

    // Push remaining byte if row doesn't align to byte boundary
    if bit_offset != 8 - bits_per_pixel {
      indices.push(current_byte);
    }
  }

  Some(PaletteData {
    palette,
    trns,
    indices,
    bit_depth,
  })
}

/// Writes a single rendered image to `destination` using `format`.
pub fn write_image<T: Write>(
  image: &RgbaImage,
  destination: &mut T,
  format: ImageOutputFormat,
  quality: Option<u8>,
) -> Result<(), crate::Error> {
  match format {
    ImageOutputFormat::Jpeg => {
      let rgb = strip_alpha_channel(image);

      let encoder = JpegEncoder::new_with_quality(destination, quality.unwrap_or(75));
      encoder.write_image(&rgb, image.width(), image.height(), ExtendedColorType::Rgb8)?;
    }
    ImageOutputFormat::Png => {
      let mut encoder = png::Encoder::new(destination, image.width(), image.height());

      if let Some(palette) = try_collect_palette(image) {
        // Use color quantization when there are too many unique colors
        encoder.set_color(ColorType::Indexed);
        encoder.set_depth(palette.bit_depth);
        encoder.set_palette(palette.palette);

        if palette.trns.iter().any(|&a| a != u8::MAX) {
          encoder.set_trns(palette.trns);
        }

        encoder.set_compression(Compression::Fast);

        // For sub-byte depths, up filter works better.
        encoder.set_filter(match palette.bit_depth {
          BitDepth::Eight => Filter::Sub,
          _ => Filter::Up,
        });

        let mut writer = encoder.write_header()?;
        writer.write_image_data(&palette.indices)?;
        writer.finish()?;
      } else {
        // Final fallback to RGB/RGBA if quantization fails
        let has_alpha = has_any_alpha_pixel(image);

        let image_data = if has_alpha {
          Cow::Borrowed(image.as_raw())
        } else {
          Cow::Owned(strip_alpha_channel(image))
        };

        encoder.set_color(if has_alpha {
          ColorType::Rgba
        } else {
          ColorType::Rgb
        });

        encoder.set_compression(Compression::Fast);
        encoder.set_filter(Filter::Sub);

        let mut writer = encoder.write_header()?;
        writer.write_image_data(&image_data)?;
        writer.finish()?;
      }
    }
    ImageOutputFormat::WebP => {
      let encoder = WebPEncoder::new(destination);

      let has_alpha = has_any_alpha_pixel(image);

      let image_data = if has_alpha {
        Cow::Borrowed(image.as_raw())
      } else {
        Cow::Owned(strip_alpha_channel(image))
      };

      encoder.encode(
        &image_data,
        image.width(),
        image.height(),
        if has_alpha {
          image_webp::ColorType::Rgba8
        } else {
          image_webp::ColorType::Rgb8
        },
      )?;
    }
    ImageOutputFormat::Svg => {
      unreachable!("SVG output requires using SvgRenderer, not write_image() with RgbaImage");
    }
  }

  Ok(())
}

/// Writes SVG content to `destination`.
pub fn write_svg<T: Write>(svg_content: &str, destination: &mut T) -> Result<(), crate::Error> {
  destination.write_all(svg_content.as_bytes())?;
  Ok(())
}

/// Scans the RIFF container and returns (offset, length) of the VP8/VP8L payload.
/// Returns None if the tag is not found or if the buffer is truncated.
fn vp8_payload_coords(buf: &[u8]) -> Option<(usize, usize)> {
  // Skip RIFF header (12 bytes)
  if buf.len() < 12 {
    return None;
  }

  let mut i = 12;
  let buf_len = buf.len();

  // Iterate over chunks
  while i + 8 <= buf_len {
    let tag = &buf[i..i + 4];

    let len = u32::from_le_bytes(buf[i + 4..i + 8].try_into().ok()?) as usize;

    // Check for VP8 (Lossy) or VP8L (Lossless)
    if tag == b"VP8 " || tag == b"VP8L" {
      let start = i + 8;
      let end = start.checked_add(len)?; // Protect against usize overflow

      // Ensure the actual data exists in the buffer.
      if end > buf_len {
        return None;
      }

      return Some((start, len));
    }

    // Calculate next chunk offset (Size + Padding)
    let padding = len & 1;

    let chunk_size = len.checked_add(padding)?;
    i = (i + 8).checked_add(chunk_size)?;
  }

  None
}

// NAME + size (4 bytes)
const BASE_HEADER_SIZE: u32 = 8;

// x (3 bytes) + y (3 bytes) + w (3 bytes) + h (3 bytes) + duration (3 bytes) + flags (1 byte)
const ANMF_HEADER_SIZE: u32 = 16;

// flags (1 byte) + cw (3 bytes) + ch (3 bytes)
const VP8X_HEADER_SIZE: u32 = 10;

// background color (4 bytes) + loop count (2 bytes)
const ANIM_HEADER_SIZE: u32 = 6;

fn estimate_vp8_payload_size(buf: &[u8]) -> Result<u32, crate::Error> {
  let (_, len) = vp8_payload_coords(buf)
    .ok_or_else(|| IoError(std::io::Error::other("VP8/VP8L chunk not found")))?;

  let padding = len & 1;

  // ANMF chunk + VP8L chunk
  Ok(BASE_HEADER_SIZE + ANMF_HEADER_SIZE + BASE_HEADER_SIZE + len as u32 + padding as u32)
}

fn estimate_riff_size<'a, I: Iterator<Item = &'a [u8]>>(frames: I) -> Result<u32, crate::Error> {
  // "WEBP" +  VPX8 chunk + ANIM chunk + [ANMF chunks]
  let mut size = 4 + BASE_HEADER_SIZE + VP8X_HEADER_SIZE + BASE_HEADER_SIZE + ANIM_HEADER_SIZE;

  for frame in frames {
    size += estimate_vp8_payload_size(frame)?;
  }

  Ok(size)
}

/// Encode a sequence of RGBA frames into an animated WebP and write to `destination`.
pub fn encode_animated_webp<W: Write>(
  frames: &[AnimationFrame],
  destination: &mut W,
  blend: bool,
  dispose: bool,
  loop_count: Option<u16>,
) -> Result<(), crate::Error> {
  assert_ne!(frames.len(), 0);

  // encode frames losslessly and collect VP8L/VP8 payloads
  #[cfg(feature = "rayon")]
  let frames_payloads: Vec<(&AnimationFrame, Vec<u8>)> = frames
    .par_iter()
    .map(|frame| {
      let mut buf = Vec::new();
      WebPEncoder::new(&mut buf).encode(
        &frame.image,
        frame.image.width(),
        frame.image.height(),
        image_webp::ColorType::Rgba8,
      )?;

      Ok((frame, buf))
    })
    .collect::<Result<Vec<(&AnimationFrame, Vec<u8>)>, crate::Error>>()?;

  #[cfg(not(feature = "rayon"))]
  let frames_payloads: Vec<(&AnimationFrame, Vec<u8>)> = frames
    .iter()
    .map(|frame| {
      let mut buf = Vec::new();
      WebPEncoder::new(&mut buf)
        .encode(
          &frame.image,
          frame.image.width(),
          frame.image.height(),
          image_webp::ColorType::Rgba8,
        )
        .map_err(|_| IoError(std::io::Error::other("WebP encode error")))?;

      Ok((frame, buf))
    })
    .collect::<Result<Vec<(&AnimationFrame, Vec<u8>)>, crate::Error>>()?;

  let riff_size = estimate_riff_size(frames_payloads.iter().map(|(_, buf)| buf.as_slice()))?;

  // RIFF header
  destination.write_all(b"RIFF")?;
  destination.write_all(&(riff_size as u32).to_le_bytes())?;
  destination.write_all(b"WEBP")?;

  // VP8X chunk
  let vp8x_flags: u8 = (1 << 1) | (1 << 4); // animation + alpha
  let cw = (frames[0].image.width() - 1).to_le_bytes();
  let ch = (frames[0].image.height() - 1).to_le_bytes();

  destination.write_all(b"VP8X")?;
  destination.write_all(&VP8X_HEADER_SIZE.to_le_bytes())?;
  destination.write_all(&[vp8x_flags])?;
  destination.write_all(&[0u8; 3])?;
  destination.write_all(&cw[..3])?;
  destination.write_all(&ch[..3])?;

  // ANIM chunk
  destination.write_all(b"ANIM")?;
  destination.write_all(&ANIM_HEADER_SIZE.to_le_bytes())?;
  destination.write_all(&[0u8; 4])?; // bgcolor (4 bytes)
  destination.write_all(&loop_count.unwrap_or(0).to_le_bytes())?;

  let frame_flags = ((blend as u8) << 1) | (dispose as u8);

  // ANMF frames
  for (frame, vp8_data) in frames_payloads.into_iter() {
    let w_bytes = (frame.image.width() - 1).to_le_bytes();
    let h_bytes = (frame.image.height() - 1).to_le_bytes();

    let (start, len) = vp8_payload_coords(&vp8_data)
      .ok_or_else(|| IoError(std::io::Error::other("VP8/VP8L chunk not found")))?;

    let vp8_payload = &vp8_data[start..start + len];

    let padding = vp8_payload.len() & 1;

    let anmf_size = ANMF_HEADER_SIZE + BASE_HEADER_SIZE + vp8_payload.len() as u32 + padding as u32; // x, y, w, h, duration, flags, payload

    destination.write_all(b"ANMF")?;
    destination.write_all(&anmf_size.to_le_bytes())?;

    // frame header (16 bytes)
    destination.write_all(&[0u8; 6])?; // x, y (3 bytes each)
    destination.write_all(&w_bytes[..3])?; // w (3 bytes)
    destination.write_all(&h_bytes[..3])?; // h (3 bytes)
    destination.write_all(&frame.duration_ms.clamp(0, U24_MAX).to_le_bytes()[..3])?; // duration (3 bytes)
    destination.write_all(&[frame_flags])?; // flags (1 byte)

    // VP8L chunk: VP8L payload
    destination.write_all(b"VP8L")?;
    destination.write_all(&(vp8_payload.len() as u32).to_le_bytes())?;
    destination.write_all(vp8_payload)?;

    // padding
    if padding == 1 {
      destination.write_all(&[0u8])?;
    }
  }

  destination.flush()?;

  Ok(())
}

/// Encode a sequence of RGBA frames into an animated PNG and write to `destination`.
pub fn encode_animated_png<W: Write>(
  frames: &[AnimationFrame],
  destination: &mut W,
  loop_count: Option<u16>,
) -> Result<(), crate::Error> {
  assert_ne!(frames.len(), 0);

  let mut encoder = png::Encoder::new(
    destination,
    frames[0].image.width(),
    frames[0].image.height(),
  );

  encoder.set_color(ColorType::Rgba);
  encoder.set_compression(png::Compression::Fastest);
  encoder.set_animated(frames.len() as u32, loop_count.unwrap_or(0) as u32)?;

  // Since APNG doesn't support variable frame duration, we use the minimum duration of all frames.
  let min_duration_ms = frames
    .iter()
    .map(|frame| frame.duration_ms)
    .min()
    .unwrap_or(0);

  encoder.set_frame_delay(min_duration_ms.clamp(0, u16::MAX as u32) as u16, 1000)?;

  let mut writer = encoder.write_header()?;

  for frame in frames {
    writer.write_image_data(frame.image.as_raw())?;
  }

  writer.finish()?;

  Ok(())
}
