use std::{
  num::NonZeroUsize,
  sync::{Mutex, RwLock},
};

use cosmic_text::{CacheKey, FontSystem, fontdb::Database};
use image::RgbaImage;
use lru::LruCache;
use taffy::Point;

use crate::{core::glyph::CachedGlyph, resources::{load_font, FontError}};

/// A context for managing fonts in the rendering system.
///
/// Holds the font system and an LRU cache for glyph images keyed by `CacheKey`.
#[derive(Debug)]
pub struct FontContext {
  /// The font system used for text layout and rendering
  pub font_system: Mutex<FontSystem>,
  /// Cache for glyph images to avoid re-rendering the same glyphs
  pub glyph_image_cache: RwLock<LruCache<CacheKey, CachedGlyph>>,
}

impl Default for FontContext {
  fn default() -> Self {
    Self {
      font_system: Mutex::new(Self::init_font_system()),
      glyph_image_cache: RwLock::new(Self::init_glyph_cache(1000)),
    }
  }
}

impl FontContext {
  /// Initializes the `FontSystem` with default locale and empty database.
  fn init_font_system() -> FontSystem {
    FontSystem::new_with_locale_and_db("en-US".to_string(), Database::new())
  }

  /// Initializes the glyph cache with a given capacity.
  fn init_glyph_cache(capacity: usize) -> LruCache<CacheKey, (Point<f32>, RgbaImage)> {
    let cap = NonZeroUsize::new(capacity.max(1)).expect("capacity must be > 0");
    LruCache::new(cap)
  }

  /// Convenience helper to mutate the font system with proper locking.
  /// Exposed publicly for modules that need short-lived mutable access (e.g., rendering).
  pub fn with_font_system_mut<F, T>(&self, f: F) -> T
  where
    F: FnOnce(&mut FontSystem) -> T,
  {
    let mut guard = self.font_system.lock().expect("font_system poisoned");
    f(&mut *guard)
  }

  /// Try get a glyph image from cache using `CacheKey` from cosmic-text's layout.
  pub fn get_cached_glyph(&self, key: &CacheKey) -> Option<RgbaImage> {
    // Use read lock first to reduce contention
    if let Ok(guard) = self.glyph_image_cache.read() {
      if let Some(img) = guard.peek(key) {
        // Clone to return an owned image; RgbaImage is cheap to clone (Vec<u8> clone)
        return Some(img.clone());
      }
    }
    None
  }

  /// Insert/update a glyph image in cache.
  pub fn put_cached_glyph(&self, key: CacheKey, img: RgbaImage) {
    if let Ok(mut guard) = self.glyph_image_cache.write() {
      guard.put(key, img);
    }
  }

  /// Loads font into internal font db
  pub fn load_font(&self, source: Vec<u8>) -> Result<(), FontError> {
    let font_data = load_font(source, None)?;
    self.with_font_system_mut(|fs| {
      fs.db_mut().load_font_data(font_data);
    });
    Ok(())
  }

  /// Render a single glyph by its CacheKey, caching the result.
  ///
  /// This method is designed to be called glyph-by-glyph using `CacheKey` generated
  /// by cosmic-text during layout (see cosmic-text's layout.rs PhysicalGlyph.cache_key).
  ///
  /// The actual rasterization is delegated to the provided `renderer` closure so this
  /// context remains decoupled from a concrete rasterizer.
  ///
  /// If the glyph is already cached, the cached image is returned.
  ///
  /// renderer: FnOnce(&mut FontSystem, &CacheKey) -> Option<RgbaImage>
  pub fn render_glyph_if_needed<R>(&self, key: &CacheKey, renderer: R) -> Option<RgbaImage>
  where
    R: FnOnce(&mut FontSystem, &CacheKey) -> Option<RgbaImage>,
  {
    // 1) Try cache
    if let Some(img) = self.get_cached_glyph(key) {
      return Some(img);
    }

    // 2) Rasterize using provided renderer with exclusive access to FontSystem
    let rendered = self.with_font_system_mut(|fs| renderer(fs, key));

    // 3) Store in cache if present
    if let Some(img) = rendered.clone() {
      self.put_cached_glyph(key.clone(), img);
    }

    rendered
  }
}
