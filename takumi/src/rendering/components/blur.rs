use image::RgbaImage;
use wide::u32x4;

/// Specifies the type of blur operation, which affects how the CSS radius is interpreted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlurType {
  /// CSS `filter: blur()` - radius equals σ (standard deviation).
  Filter,
  /// CSS `box-shadow` / `text-shadow` blur - radius equals 2σ.
  Shadow,
}

impl BlurType {
  #[inline]
  pub fn to_sigma(self, css_radius: f32) -> f32 {
    match self {
      BlurType::Filter => css_radius,
      BlurType::Shadow => css_radius * 0.5,
    }
  }

  #[inline]
  pub fn extent_multiplier(self) -> f32 {
    match self {
      BlurType::Filter => 3.0,
      BlurType::Shadow => 1.5,
    }
  }
}

/// Applies a Gaussian approximation using 3-pass Box Blur with SIMD (u32x4).
pub(crate) fn apply_blur(image: &mut RgbaImage, radius: f32, blur_type: BlurType) {
  let sigma = blur_type.to_sigma(radius);
  if sigma <= 0.5 {
    return;
  }

  let box_radius = (((4.0 * sigma * sigma + 1.0).sqrt() - 1.0) * 0.5)
    .round()
    .max(1.0) as u32;

  let (width, height) = image.dimensions();
  if width == 0 || height == 0 {
    return;
  }

  premultiply_alpha(image);

  let mut temp_data = vec![0u8; (width * height * 4) as usize];
  let img_data = &mut **image;
  let stride = width as usize * 4;

  // 3-pass Box Blur to approximate Gaussian
  for _ in 0..3 {
    box_blur_h(img_data, &mut temp_data, width, height, box_radius, stride);
    box_blur_v(&temp_data, img_data, width, height, box_radius, stride);
  }

  unpremultiply_alpha(image);
}

/// Horizontal Box Blur Pass
fn box_blur_h(src: &[u8], dst: &mut [u8], width: u32, height: u32, radius: u32, stride: usize) {
  let r = radius as i32;
  let w = width as i32;
  let div = (2 * r + 1) as u32;
  let (mul_val, shg) = compute_mul_shg(div);
  let mul = u32x4::new([mul_val; 4]);

  for y in 0..height {
    let line_offset = y as usize * stride;
    let mut sum = u32x4::ZERO;

    let p_first = load_pixel(src, line_offset);
    sum += p_first * (r as u32 + 1);

    for x in 1..=r {
      sum += load_pixel(src, line_offset + (x.min(w - 1) as usize * 4));
    }

    for x in 0..w {
      store_pixel(dst, line_offset + (x as usize * 4), (sum * mul) >> shg);

      let p_leaving = load_pixel(src, line_offset + ((x - r).max(0) as usize * 4));
      let p_entering = load_pixel(src, line_offset + ((x + r + 1).min(w - 1) as usize * 4));
      // Wrapping subtraction is fine for u32x4 since the final sum stays positive
      sum = sum + p_entering - p_leaving;
    }
  }
}

/// Vertical Box Blur Pass
fn box_blur_v(src: &[u8], dst: &mut [u8], width: u32, height: u32, radius: u32, stride: usize) {
  let r = radius as i32;
  let h = height as i32;
  let div = (2 * r + 1) as u32;
  let (mul_val, shg) = compute_mul_shg(div);
  let mul = u32x4::new([mul_val; 4]);

  for x in 0..width {
    let col_offset = x as usize * 4;
    let mut sum = u32x4::ZERO;

    let p_first = load_pixel(src, col_offset);
    sum += p_first * (r as u32 + 1);

    for y in 1..=r {
      sum += load_pixel(src, col_offset + (y.min(h - 1) as usize * stride));
    }

    for y in 0..h {
      store_pixel(dst, col_offset + (y as usize * stride), (sum * mul) >> shg);

      let p_leaving = load_pixel(src, col_offset + ((y - r).max(0) as usize * stride));
      let p_entering = load_pixel(src, col_offset + ((y + r + 1).min(h - 1) as usize * stride));
      sum = sum + p_entering - p_leaving;
    }
  }
}

#[inline(always)]
fn compute_mul_shg(d: u32) -> (u32, i32) {
  let shg = 23;
  let mul = ((1u64 << shg) as f64 / d as f64).round() as u32;
  (mul, shg)
}

#[inline(always)]
fn load_pixel(buffer: &[u8], offset: usize) -> u32x4 {
  u32x4::new([
    buffer[offset] as u32,
    buffer[offset + 1] as u32,
    buffer[offset + 2] as u32,
    buffer[offset + 3] as u32,
  ])
}

#[inline(always)]
fn store_pixel(buffer: &mut [u8], offset: usize, pixel: u32x4) {
  let arr: [u32; 4] = pixel.into();
  buffer[offset] = arr[0].min(255) as u8;
  buffer[offset + 1] = arr[1].min(255) as u8;
  buffer[offset + 2] = arr[2].min(255) as u8;
  buffer[offset + 3] = arr[3].min(255) as u8;
}

fn premultiply_alpha(image: &mut RgbaImage) {
  for pixel in image.pixels_mut() {
    let a = pixel.0[3] as u16;
    if a == 0 {
      pixel.0 = [0, 0, 0, 0];
    } else if a < 255 {
      pixel.0[0] = fast_div_255(pixel.0[0] as u16 * a);
      pixel.0[1] = fast_div_255(pixel.0[1] as u16 * a);
      pixel.0[2] = fast_div_255(pixel.0[2] as u16 * a);
    }
  }
}

fn unpremultiply_alpha(image: &mut RgbaImage) {
  for pixel in image.pixels_mut() {
    let a = pixel.0[3] as u16;
    if a != 0 && a < 255 {
      pixel.0[0] = ((pixel.0[0] as u16 * 255 + a / 2) / a).min(255) as u8;
      pixel.0[1] = ((pixel.0[1] as u16 * 255 + a / 2) / a).min(255) as u8;
      pixel.0[2] = ((pixel.0[2] as u16 * 255 + a / 2) / a).min(255) as u8;
    }
  }
}

#[inline(always)]
pub(crate) fn fast_div_255(v: u16) -> u8 {
  ((v + 128 + (v >> 8)) >> 8) as u8
}
