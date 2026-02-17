//! Image resource management for the takumi rendering system.
//!
//! This module provides types and utilities for managing image resources,
//! including loading states, error handling, and image processing operations.

use std::{borrow::Cow, sync::Arc};

use dashmap::DashMap;
use image::RgbaImage;

use crate::{
  layout::style::ImageScalingAlgorithm,
  rendering::{fast_resize, unpremultiply_alpha},
};
use thiserror::Error;

/// Represents the state of an image resource.
pub type ImageResult = Result<Arc<ImageSource>, ImageResourceError>;

#[derive(Debug, Clone)]
/// Represents the source of an image.
pub enum ImageSource {
  /// An svg image source
  #[cfg(feature = "svg")]
  Svg(Box<resvg::usvg::Tree>),
  /// A bitmap image source
  Bitmap(RgbaImage),
}

/// Represents a persistent image store.
pub type PersistentImageStore = DashMap<String, Arc<ImageSource>>;

impl From<RgbaImage> for ImageSource {
  fn from(bitmap: RgbaImage) -> Self {
    ImageSource::Bitmap(bitmap)
  }
}

impl ImageSource {
  /// Get the size of the image source.
  pub fn size(&self) -> (f32, f32) {
    match self {
      #[cfg(feature = "svg")]
      ImageSource::Svg(svg) => (svg.size().width(), svg.size().height()),
      ImageSource::Bitmap(bitmap) => (bitmap.width() as f32, bitmap.height() as f32),
    }
  }

  /// Render the image source to an RGBA image with the specified dimensions.
  pub fn render_to_rgba_image<'i>(
    &'i self,
    width: u32,
    height: u32,
    algorithm: ImageScalingAlgorithm,
  ) -> Result<Cow<'i, RgbaImage>, ImageResourceError> {
    match self {
      ImageSource::Bitmap(bitmap) => {
        if bitmap.width() == width && bitmap.height() == height {
          return Ok(Cow::Borrowed(bitmap));
        }

        Ok(Cow::Owned(fast_resize(bitmap, width, height, algorithm)?))
      }
      #[cfg(feature = "svg")]
      ImageSource::Svg(svg) => {
        use resvg::{tiny_skia::Pixmap, usvg::Transform};

        let mut pixmap = Pixmap::new(width, height).ok_or(ImageResourceError::InvalidPixmapSize)?;

        let original_size = svg.size();
        let sx = width as f32 / original_size.width();
        let sy = height as f32 / original_size.height();

        resvg::render(svg, Transform::from_scale(sx, sy), &mut pixmap.as_mut());

        let mut image = RgbaImage::from_raw(width, height, pixmap.take())
          .ok_or(ImageResourceError::MismatchedBufferSize)?;

        for pixel in image.pixels_mut() {
          unpremultiply_alpha(pixel);
        }

        Ok(Cow::Owned(image))
      }
    }
  }
}

/// Try to load an image source from raw bytes.
///
/// - When the `svg` feature is enabled and the bytes look like SVG XML, they
///   are parsed as an SVG using `resvg::usvg`.
/// - Otherwise, the bytes are decoded as a raster image using the `image` crate.
pub fn load_image_source_from_bytes(bytes: &[u8]) -> ImageResult {
  #[cfg(feature = "svg")]
  {
    use std::str::from_utf8;

    if let Ok(text) = from_utf8(bytes)
      && is_svg_like(text)
    {
      return parse_svg_str(text);
    }
  }

  let img = image::load_from_memory(bytes).map_err(ImageResourceError::DecodeError)?;
  Ok(Arc::new(img.into_rgba8().into()))
}

/// Check if the string looks like an SVG image.
pub(crate) fn is_svg_like(src: &str) -> bool {
  src.contains("<svg") && src.contains("xmlns")
}

#[cfg(feature = "svg")]
/// Parse SVG from &str.
pub fn parse_svg_str(src: &str) -> ImageResult {
  use resvg::usvg::Tree;

  let tree = Tree::from_str(src, &Default::default()).map_err(ImageResourceError::SvgParseError)?;

  Ok(Arc::new(ImageSource::Svg(Box::new(tree))))
}

/// Represents the state of an image in the rendering system.
///
/// This enum tracks whether an image has been successfully loaded and decoded,
/// or if there was an error during the process.
#[derive(Debug, Error)]
pub enum ImageResourceError {
  /// An error occurred while decoding the image data
  #[error("An error occurred while decoding the image data: {0}")]
  DecodeError(#[from] image::ImageError),
  /// The image data URI is in an invalid format
  #[error("The image data URI is in an invalid format")]
  InvalidDataUriFormat,
  /// The image data URI is malformed and cannot be parsed
  #[error("The image data URI is malformed and cannot be parsed")]
  MalformedDataUri,
  #[cfg(feature = "svg")]
  /// An error occurred while parsing an SVG image
  #[error("An error occurred while parsing an SVG image: {0}")]
  SvgParseError(#[from] resvg::usvg::Error),
  /// SVG parsing is not supported in this build
  #[cfg(not(feature = "svg"))]
  #[error("SVG parsing is not supported in this build")]
  SvgParseNotSupported,
  /// The image source is unknown
  #[error("The image source is unknown")]
  Unknown,
  /// The pixmap size is invalid
  #[error("The pixmap size is invalid")]
  InvalidPixmapSize,
  /// The buffer size does not match the target image size
  #[error("The buffer size does not match the target image size")]
  MismatchedBufferSize,
  /// An error occurred while resizing the image
  #[error("An error occurred while resizing the image: {0}")]
  ResizeError(#[from] fast_image_resize::ResizeError),
}
