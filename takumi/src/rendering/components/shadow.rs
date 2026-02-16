use image::RgbaImage;
use taffy::{Layout, Point, Size};
use zeno::{Command, Fill, PathData, Placement};

use crate::{
  layout::style::{Affine, BlendMode, BoxShadow, Color, ImageScalingAlgorithm, Sides, TextShadow},
  rendering::{
    BlurType, BorderProperties, Canvas, CanvasConstrain, MaskMemory, Sizing, apply_blur, draw_mask,
    overlay_image,
  },
};

/// Represents a resolved box shadow with all its properties.
#[derive(Clone, Copy)]
pub(crate) struct SizedShadow {
  /// Horizontal offset of the shadow.
  pub offset_x: f32,
  /// Vertical offset of the shadow.
  pub offset_y: f32,
  /// Blur radius of the shadow. Higher values create a more blurred shadow.
  pub blur_radius: f32,
  /// Spread radius of the shadow. Positive values expand the shadow, negative values shrink it.
  pub spread_radius: f32,
  /// Color of the shadow.
  pub color: Color,
}

impl SizedShadow {
  /// Creates a new [`SizedShadow`] from a [`BoxShadow`].
  pub fn from_box_shadow(
    shadow: BoxShadow,
    sizing: &Sizing,
    current_color: Color,
    size: Size<f32>,
  ) -> Self {
    Self {
      offset_x: shadow.offset_x.to_px(sizing, size.width),
      offset_y: shadow.offset_y.to_px(sizing, size.height),
      blur_radius: shadow.blur_radius.to_px(sizing, size.width),
      spread_radius: shadow.spread_radius.to_px(sizing, size.width),
      color: shadow.color.resolve(current_color),
    }
  }

  /// Creates a new `SizedShadow` from a `TextShadow`.
  pub fn from_text_shadow(
    shadow: TextShadow,
    sizing: &Sizing,
    current_color: Color,
    size: Size<f32>,
  ) -> Self {
    Self {
      offset_x: shadow.offset_x.to_px(sizing, size.width),
      offset_y: shadow.offset_y.to_px(sizing, size.height),
      blur_radius: shadow.blur_radius.to_px(sizing, size.width),
      // Text shadows do not support spread radius; set to 0.
      spread_radius: 0.0,
      color: shadow.color.resolve(current_color),
    }
  }

  /// Draws the outset mask of the shadow.
  pub fn draw_outset<D: PathData>(
    &self,
    canvas: &mut RgbaImage,
    mask_memory: &mut MaskMemory,
    constrain: Option<&CanvasConstrain>,
    paths: D,
    transform: Affine,
    style: zeno::Style,
  ) {
    let (mask, mut placement) = mask_memory.render(&paths, Some(transform), Some(style));

    placement.left += self.offset_x as i32;
    placement.top += self.offset_y as i32;

    // Fast path: if the blur radius is 0, we can just draw the spread mask
    if self.blur_radius <= 0.0 {
      return draw_mask(
        canvas,
        mask,
        placement,
        self.color,
        BlendMode::Normal,
        constrain,
      );
    }

    // Create a new image with the spread mask on, blurred by the blur radius
    let blur_padding = self.blur_radius * BlurType::Shadow.extent_multiplier();
    let mut image = RgbaImage::new(
      placement.width + (blur_padding * 2.0) as u32,
      placement.height + (blur_padding * 2.0) as u32,
    );

    draw_mask(
      &mut image,
      mask,
      Placement {
        left: blur_padding as i32,
        top: blur_padding as i32,
        width: placement.width,
        height: placement.height,
      },
      self.color,
      BlendMode::Normal,
      None,
    );

    apply_blur(&mut image, self.blur_radius, BlurType::Shadow);

    overlay_image(
      canvas,
      &image,
      BorderProperties::zero(),
      Affine::translation(
        placement.left as f32 - blur_padding,
        placement.top as f32 - blur_padding,
      ),
      ImageScalingAlgorithm::Auto,
      BlendMode::Normal,
      constrain,
      mask_memory,
    );
  }

  /// Draws an outset box shadow as a ring, excluding the element's border-box area.
  /// Per CSS spec, outset box-shadows are drawn outside the border edge only.
  pub fn draw_outset_ring(
    &self,
    canvas: &mut RgbaImage,
    mask_memory: &mut MaskMemory,
    constrain: Option<&CanvasConstrain>,
    shadow_paths: &[Command],
    element_border_radius: BorderProperties,
    element_size: Size<f32>,
    transform: Affine,
  ) {
    // 1. Render the shadow shape as a solid fill.
    let (mask, mut placement) =
      mask_memory.render(shadow_paths, Some(transform), Some(Fill::NonZero.into()));

    placement.left += self.offset_x as i32;
    placement.top += self.offset_y as i32;

    // 2. Draw onto temp image (with blur padding if needed).
    let blur_padding = if self.blur_radius > 0.0 {
      self.blur_radius * BlurType::Shadow.extent_multiplier()
    } else {
      0.0
    };

    let mut image = RgbaImage::new(
      placement.width + (blur_padding * 2.0) as u32,
      placement.height + (blur_padding * 2.0) as u32,
    );

    draw_mask(
      &mut image,
      mask,
      Placement {
        left: blur_padding as i32,
        top: blur_padding as i32,
        width: placement.width,
        height: placement.height,
      },
      self.color,
      BlendMode::Normal,
      None,
    );

    // 3. Apply blur if needed.
    if self.blur_radius > 0.0 {
      apply_blur(&mut image, self.blur_radius, BlurType::Shadow);
    }

    // 4. Erase the element's border-box area from the shadow image.
    // This ensures the shadow only renders outside the element per CSS spec.
    let mut element_paths = Vec::new();
    element_border_radius.append_mask_commands(&mut element_paths, element_size, Point::ZERO);

    let (erase_mask, erase_placement) =
      mask_memory.render(&element_paths, Some(transform), Some(Fill::NonZero.into()));

    let img_origin_x = placement.left as f32 - blur_padding;
    let img_origin_y = placement.top as f32 - blur_padding;
    let img_w = image.width() as i32;
    let img_h = image.height() as i32;

    if !erase_mask.is_empty() {
      for my in 0..erase_placement.height as i32 {
        for mx in 0..erase_placement.width as i32 {
          let canvas_x = erase_placement.left + mx;
          let canvas_y = erase_placement.top + my;
          let ix = canvas_x - img_origin_x as i32;
          let iy = canvas_y - img_origin_y as i32;

          if ix >= 0 && iy >= 0 && ix < img_w && iy < img_h {
            let mask_alpha =
              erase_mask[(my as u32 * erase_placement.width + mx as u32) as usize] as u32;
            if mask_alpha > 0 {
              let pixel = image.get_pixel_mut(ix as u32, iy as u32);
              pixel.0[3] = ((pixel.0[3] as u32 * (255 - mask_alpha)) / 255) as u8;
            }
          }
        }
      }
    }

    // 5. Overlay the ring shadow onto the canvas.
    overlay_image(
      canvas,
      &image,
      BorderProperties::zero(),
      Affine::translation(img_origin_x, img_origin_y),
      ImageScalingAlgorithm::Auto,
      BlendMode::Normal,
      constrain,
      mask_memory,
    );
  }

  pub fn draw_inset(
    &self,
    transform: Affine,
    border_radius: BorderProperties,
    canvas: &mut Canvas,
    layout: Layout,
  ) {
    let image = draw_inset_shadow(self, border_radius, layout.size, &mut canvas.mask_memory);

    canvas.overlay_image(
      &image,
      border_radius,
      transform,
      ImageScalingAlgorithm::Auto,
      BlendMode::Normal,
    );
  }
}

fn draw_inset_shadow(
  shadow: &SizedShadow,
  mut border: BorderProperties,
  border_box: Size<f32>,
  mask_memory: &mut MaskMemory,
) -> RgbaImage {
  // Fill the entire image with shadow color. The inner cutout will be erased below.
  // This avoids anti-aliasing artifacts at the outer boundary that the even-odd
  // approach produces (the outer shape's AA edges would create a dark fringe
  // around the entire element).
  let mut shadow_image = RgbaImage::from_pixel(
    border_box.width as u32,
    border_box.height as u32,
    shadow.color.into(),
  );

  let offset = Point {
    x: shadow.offset_x,
    y: shadow.offset_y,
  };

  // Render only the inner cutout shape — no outer boundary needed since the
  // filled image itself acts as the outer boundary.
  let mut paths = Vec::new();

  // Adjust border radii for the cutout: negate spread because expand_by adds,
  // but contraction (positive spread) needs subtraction.
  border.expand_by(Sides([-shadow.spread_radius; 4]).into());
  border.append_mask_commands(
    &mut paths,
    border_box
      - Size {
        width: shadow.spread_radius * 2.0,
        height: shadow.spread_radius * 2.0,
      },
    offset
      + Point {
        x: shadow.spread_radius,
        y: shadow.spread_radius,
      },
  );

  let (mask, placement) = mask_memory.render(&paths, None, Some(Fill::NonZero.into()));

  // Erase the cutout: reduce shadow alpha where the cutout mask has coverage.
  // This creates the "hole" in the shadow, leaving shadow only at the edges.
  if !mask.is_empty() {
    let img_w = shadow_image.width() as i32;
    let img_h = shadow_image.height() as i32;

    for my in 0..placement.height as i32 {
      for mx in 0..placement.width as i32 {
        let ix = placement.left + mx;
        let iy = placement.top + my;

        if ix >= 0 && iy >= 0 && ix < img_w && iy < img_h {
          let mask_alpha = mask[(my as u32 * placement.width + mx as u32) as usize] as u32;
          if mask_alpha > 0 {
            let pixel = shadow_image.get_pixel_mut(ix as u32, iy as u32);
            pixel.0[3] = ((pixel.0[3] as u32 * (255 - mask_alpha)) / 255) as u8;
          }
        }
      }
    }
  }

  apply_blur(&mut shadow_image, shadow.blur_radius, BlurType::Shadow);

  shadow_image
}
