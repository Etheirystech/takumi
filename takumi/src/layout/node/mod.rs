mod container;
mod image;
mod text;

pub use container::*;
pub use image::{ImageNode, resolve_image};
pub use text::*;

use std::borrow::Cow;

use serde::Deserialize;
use taffy::{AvailableSpace, Layout, Size};

pub use crate::layout::style::gradient_utils::resolve_stops_along_axis;
use crate::{
  Result,
  layout::{
    Viewport,
    inline::InlineContentKind,
    style::{Affine, BackgroundClip, BackgroundImage, Color, CssValue, InheritedStyle, Style},
  },
  rendering::{
    BorderProperties, Canvas, RenderContext, SizedShadow, collect_background_layers,
    rasterize_layers,
  },
  resources::task::FetchTaskCollection,
};

/// Implements the Node trait for an enum type that contains different node variants.
macro_rules! impl_node_enum {
  ($name:ident, $($variant:ident => $variant_type:ty),*) => {
    impl $crate::layout::node::Node<$name> for $name {
      fn take_children(&mut self) -> Option<Box<[$name]>> {
        match self {
          $( $name::$variant(inner) => inner.take_children(), )*
        }
      }

      fn children_ref(&self) -> Option<&[$name]> {
        match self {
          $( $name::$variant(inner) => inner.children_ref(), )*
        }
      }

      fn create_inherited_style(&mut self, parent: &$crate::layout::style::InheritedStyle, viewport: $crate::layout::Viewport) -> $crate::layout::style::InheritedStyle {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::create_inherited_style(inner, parent, viewport), )*
        }
      }

      fn inline_content(&self) -> Option<$crate::layout::inline::InlineContentKind<'_>> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::inline_content(inner), )*
        }
      }

      fn measure(
        &self,
        context: &$crate::rendering::RenderContext,
        available_space: $crate::taffy::Size<$crate::taffy::AvailableSpace>,
        known_dimensions: $crate::taffy::Size<Option<f32>>,
        style: &taffy::Style,
      ) -> $crate::taffy::Size<f32> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::measure(inner, context, available_space, known_dimensions, style), )*
        }
      }

      fn draw_content(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_content(inner, context, canvas, layout), )*
        }
      }

      fn draw_content_svg(&self, context: &$crate::rendering::RenderContext, svg: &mut $crate::rendering::SvgRenderer, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_content_svg(inner, context, svg, layout), )*
        }
      }

      fn draw_border(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_border(inner, context, canvas, layout), )*
        }
      }

      fn draw_outset_box_shadow(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_outset_box_shadow(inner, context, canvas, layout), )*
        }
      }

      fn draw_outset_box_shadow_svg(&self, context: &$crate::rendering::RenderContext, svg: &mut $crate::rendering::SvgRenderer, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_outset_box_shadow_svg(inner, context, svg, layout), )*
        }
      }

      fn draw_inset_box_shadow(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_inset_box_shadow(inner, context, canvas, layout), )*
        }
      }

      fn draw_inset_box_shadow_svg(&self, context: &$crate::rendering::RenderContext, svg: &mut $crate::rendering::SvgRenderer, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_inset_box_shadow_svg(inner, context, svg, layout), )*
        }
      }

      fn get_style(&self) -> Option<&Style> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::get_style(inner), )*
        }
      }

      fn collect_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::collect_fetch_tasks(inner, collection), )*
        }
      }

      fn collect_style_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::collect_style_fetch_tasks(inner, collection), )*
        }
      }

      fn draw_background(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_background(inner, context, canvas, layout), )*
        }
      }

      fn draw_background_image_svg(&self, context: &$crate::rendering::RenderContext, svg: &mut $crate::rendering::SvgRenderer, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_background_image_svg(inner, context, svg, layout), )*
        }
      }
    }

    $(
      impl From<$variant_type> for $name {
        fn from(inner: $variant_type) -> Self {
          $name::$variant(inner)
        }
      }
    )*
  };
}

/// A trait representing a node in the layout tree.
///
/// This trait defines the common interface for all elements that can be
/// rendered in the layout system, including containers, text, and images.
pub trait Node<N: Node<N>>: Send + Sync + Clone {
  /// Gets reference of children.
  fn children_ref(&self) -> Option<&[N]> {
    None
  }

  /// Creates resolving tasks for node's http resources.
  fn collect_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
    let Some(children) = self.children_ref() else {
      return;
    };

    for child in children {
      child.collect_fetch_tasks(collection);
    }
  }

  /// Returns a reference to this node's raw [`Style`], if any.
  fn get_style(&self) -> Option<&Style>;

  /// Creates resolving tasks for style's http resources.
  fn collect_style_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
    if let Some(style) = self.get_style() {
      if let CssValue::Value(Some(images)) = &style.background_image {
        collection.insert_many(images.iter().filter_map(|image| {
          if let BackgroundImage::Url(url) = image {
            Some(url.clone())
          } else {
            None
          }
        }))
      };

      if let CssValue::Value(background) = &style.background {
        collection.insert_many(background.iter().filter_map(|background| {
          if let BackgroundImage::Url(url) = &background.image {
            Some(url.clone())
          } else {
            None
          }
        }));
      };

      if let CssValue::Value(Some(images)) = &style.mask_image {
        collection.insert_many(images.iter().filter_map(|image| {
          if let BackgroundImage::Url(url) = image {
            Some(url.clone())
          } else {
            None
          }
        }));
      };

      if let CssValue::Value(mask) = &style.mask {
        collection.insert_many(mask.iter().filter_map(|background| {
          if let BackgroundImage::Url(url) = &background.image {
            Some(url.clone())
          } else {
            None
          }
        }));
      };
    };

    let Some(children) = self.children_ref() else {
      return;
    };

    for child in children {
      child.collect_fetch_tasks(collection);
    }
  }

  /// Return reference to children nodes.
  fn take_children(&mut self) -> Option<Box<[N]>> {
    None
  }

  /// Create a [`InheritedStyle`] instance or clone the parent's.
  fn create_inherited_style(
    &mut self,
    _parent: &InheritedStyle,
    viewport: Viewport,
  ) -> InheritedStyle;

  /// Retrieve content for inline layout.
  fn inline_content(&self) -> Option<InlineContentKind<'_>> {
    None
  }

  /// Measures content size of this node.
  fn measure(
    &self,
    _context: &RenderContext,
    _available_space: Size<AvailableSpace>,
    _known_dimensions: Size<Option<f32>>,
    _style: &taffy::Style,
  ) -> Size<f32> {
    Size::ZERO
  }

  /// Draws the outset box shadow of the node.
  fn draw_outset_box_shadow(
    &self,
    _context: &RenderContext,
    _canvas: &mut Canvas,
    _layout: Layout,
  ) -> Result<()> {
    // Default implementation does nothing
    Ok(())
  }

  /// Draws the outset box shadow of the node as SVG.
  fn draw_outset_box_shadow_svg(
    &self,
    context: &RenderContext,
    svg: &mut crate::rendering::SvgRenderer,
    layout: Layout,
  ) -> Result<()> {
    let Some(box_shadow) = context.style.box_shadow.as_ref() else {
      return Ok(());
    };

    let border_radius = BorderProperties::from_context(context, layout.size, layout.border);

    for shadow in box_shadow.iter() {
      if shadow.inset {
        continue;
      }

      let shadow =
        SizedShadow::from_box_shadow(*shadow, &context.sizing, context.current_color, layout.size);

      let color = format!(
        "rgba({},{},{},{})",
        shadow.color.0[0],
        shadow.color.0[1],
        shadow.color.0[2],
        shadow.color.0[3] as f32 / 255.0
      );

      let filter =
        svg.add_shadow_filter(shadow.offset_x, shadow.offset_y, shadow.blur_radius, &color);

      // Use an opaque fill for the shadow source rect so feGaussianBlur in="SourceAlpha" works,
      // but the filter itself only outputs the shadow (not the source graphic).
      svg.fill_rect_with_filter(
        layout.size,
        Color::black(),
        &filter,
        border_radius,
        context.transform,
      );
    }

    Ok(())
  }

  /// Draws the inset box shadow of the node.
  fn draw_inset_box_shadow(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    let Some(box_shadow) = context.style.box_shadow.as_ref() else {
      return Ok(());
    };

    let border_radius = BorderProperties::from_context(context, layout.size, layout.border);

    for shadow in box_shadow.iter() {
      if !shadow.inset {
        continue;
      }

      let sized_shadow =
        SizedShadow::from_box_shadow(*shadow, &context.sizing, context.current_color, layout.size);

      sized_shadow.draw_inset(context.transform, border_radius, canvas, layout);
    }

    Ok(())
  }

  /// Draws the inset box shadow of the node as SVG.
  fn draw_inset_box_shadow_svg(
    &self,
    context: &RenderContext,
    svg: &mut crate::rendering::SvgRenderer,
    layout: Layout,
  ) -> Result<()> {
    let Some(box_shadow) = context.style.box_shadow.as_ref() else {
      return Ok(());
    };

    let border_radius = BorderProperties::from_context(context, layout.size, layout.border);

    for shadow in box_shadow.iter() {
      if !shadow.inset {
        continue;
      }

      let shadow =
        SizedShadow::from_box_shadow(*shadow, &context.sizing, context.current_color, layout.size);

      let color = format!(
        "rgba({},{},{},{})",
        shadow.color.0[0],
        shadow.color.0[1],
        shadow.color.0[2],
        shadow.color.0[3] as f32 / 255.0
      );

      let filter =
        svg.add_inset_shadow_filter(shadow.offset_x, shadow.offset_y, shadow.blur_radius, &color);

      svg.fill_rect_with_filter(
        layout.size,
        shadow.color,
        &filter,
        border_radius,
        context.transform,
      );
    }

    Ok(())
  }

  /// Draws the background image(s) of the node.
  fn draw_background(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    let mut border_radius = BorderProperties::from_context(context, layout.size, layout.border);

    match context.style.background_clip {
      BackgroundClip::BorderBox => {
        let tiles = collect_background_layers(context, layout.size)?;

        for tile in tiles {
          for y in &tile.ys {
            for x in &tile.xs {
              canvas.overlay_image(
                &tile.tile,
                border_radius,
                context.transform * Affine::translation(*x as f32, *y as f32),
                context.style.image_rendering,
              );
            }
          }
        }
      }
      BackgroundClip::PaddingBox => {
        border_radius.inset_by_border_width();

        let layers = collect_background_layers(context, layout.size)?;

        let Some(rasterized) = rasterize_layers(
          layers,
          Size {
            width: (layout.size.width - layout.border.left - layout.border.right) as u32,
            height: (layout.size.height - layout.border.top - layout.border.bottom) as u32,
          },
          context,
          border_radius,
          Affine::translation(-layout.border.left, -layout.border.top),
          &mut canvas.mask_memory,
        ) else {
          return Ok(());
        };

        canvas.overlay_image(
          &rasterized,
          BorderProperties::default(),
          context.transform * Affine::translation(layout.border.left, layout.border.top),
          context.style.image_rendering,
        );
      }
      BackgroundClip::ContentBox => {
        border_radius.inset_by_border_width();
        border_radius.expand_by(layout.padding.map(|size| -size));

        let layers = collect_background_layers(context, layout.size)?;

        let Some(rasterized) = rasterize_layers(
          layers,
          layout.content_box_size().map(|x| x as u32),
          context,
          border_radius,
          Affine::translation(
            -layout.padding.left - layout.border.left,
            -layout.padding.top - layout.border.top,
          ),
          &mut canvas.mask_memory,
        ) else {
          return Ok(());
        };

        canvas.overlay_image(
          &rasterized,
          BorderProperties::default(),
          context.transform
            * Affine::translation(
              layout.padding.left + layout.border.left,
              layout.padding.top + layout.border.top,
            ),
          context.style.image_rendering,
        );
      }
      _ => {}
    }

    Ok(())
  }

  /// Draws the background image(s) of the node as SVG.
  fn draw_background_image_svg(
    &self,
    context: &RenderContext,
    svg: &mut crate::rendering::SvgRenderer,
    layout: Layout,
  ) -> Result<()> {
    let background_image = context
      .style
      .background_image
      .as_deref()
      .map(Cow::Borrowed)
      .unwrap_or_else(|| {
        Cow::Owned(
          context
            .style
            .background
            .iter()
            .map(|background| background.image.clone())
            .collect::<Vec<_>>(),
        )
      });

    if background_image.is_empty() {
      return Ok(());
    }

    let (draw_size, draw_transform, border_radius) = match context.style.background_clip {
      BackgroundClip::BorderBox | BackgroundClip::BorderArea => {
        let radius = BorderProperties::from_context(context, layout.size, layout.border);
        (layout.size, context.transform, radius)
      }
      BackgroundClip::PaddingBox => {
        let mut radius = BorderProperties::from_context(context, layout.size, layout.border);
        radius.inset_by_border_width();
        let size = Size {
          width: layout.size.width - layout.border.left - layout.border.right,
          height: layout.size.height - layout.border.top - layout.border.bottom,
        };
        let transform =
          context.transform * Affine::translation(layout.border.left, layout.border.top);
        (size, transform, radius)
      }
      BackgroundClip::ContentBox => {
        let mut radius = BorderProperties::from_context(context, layout.size, layout.border);
        radius.inset_by_border_width();
        radius.expand_by(layout.padding.map(|size| -size));
        let size = layout.content_box_size();
        let transform = context.transform
          * Affine::translation(
            layout.padding.left + layout.border.left,
            layout.padding.top + layout.border.top,
          );
        (size, transform, radius)
      }
      BackgroundClip::Text => return Ok(()),
    };

    for image in background_image.iter() {
      match image {
        BackgroundImage::Linear(gradient) => {
          let rad = (*gradient.angle).to_radians();
          let (dir_x, dir_y) = (rad.sin(), -rad.cos());

          let cx = draw_size.width / 2.0;
          let cy = draw_size.height / 2.0;
          let max_extent =
            ((draw_size.width * dir_x.abs()) + (draw_size.height * dir_y.abs())) / 2.0;

          let x1 = cx - dir_x * max_extent;
          let y1 = cy - dir_y * max_extent;
          let x2 = cx + dir_x * max_extent;
          let y2 = cy + dir_y * max_extent;

          let mut stops = Vec::new();
          let resolved_stops =
            resolve_stops_along_axis(&gradient.stops, (max_extent * 2.0).max(1e-6), context);
          for stop in resolved_stops {
            let color = format!(
              "rgba({},{},{},{})",
              stop.color.0[0],
              stop.color.0[1],
              stop.color.0[2],
              stop.color.0[3] as f32 / 255.0
            );
            stops.push((stop.position / (max_extent * 2.0).max(1e-6), color));
          }

          let fill = svg.add_linear_gradient(
            &format!("{}", x1),
            &format!("{}", y1),
            &format!("{}", x2),
            &format!("{}", y2),
            &stops,
          );

          svg.fill_rect_with_fill(draw_size, &fill, border_radius, draw_transform);
        }
        BackgroundImage::Radial(gradient) => {
          let center_point = gradient.center.to_point(&context.sizing, draw_size);
          let cx = center_point.x;
          let cy = center_point.y;

          let dx_left = cx;
          let dx_right = draw_size.width - cx;
          let dy_top = cy;
          let dy_bottom = draw_size.height - cy;

          let (radius_x, radius_y) = match (gradient.shape, gradient.size) {
            (super::style::RadialShape::Ellipse, super::style::RadialSize::FarthestCorner) => {
              (dx_left.max(dx_right), dy_top.max(dy_bottom))
            }
            (super::style::RadialShape::Circle, super::style::RadialSize::FarthestCorner) => {
              let candidates = [
                (cx, cy),
                (cx, dy_bottom),
                (dx_right, cy),
                (dx_right, dy_bottom),
              ];
              let r = candidates
                .iter()
                .map(|(dx, dy)| (dx * dx + dy * dy).sqrt())
                .fold(0.0_f32, f32::max);
              (r, r)
            }
            (super::style::RadialShape::Ellipse, super::style::RadialSize::FarthestSide) => {
              (dx_left.max(dx_right), dy_top.max(dy_bottom))
            }
            (super::style::RadialShape::Ellipse, super::style::RadialSize::ClosestSide) => {
              (dx_left.min(dx_right), dy_top.min(dy_bottom))
            }
            (super::style::RadialShape::Circle, super::style::RadialSize::FarthestSide) => {
              let r = dx_left.max(dx_right).max(dy_top.max(dy_bottom));
              (r, r)
            }
            (super::style::RadialShape::Circle, super::style::RadialSize::ClosestSide) => {
              let r = dx_left.min(dx_right).min(dy_top.min(dy_bottom));
              (r, r)
            }
            (super::style::RadialShape::Ellipse, super::style::RadialSize::ClosestCorner) => {
              (dx_left.max(dx_right), dy_top.max(dy_bottom))
            }
            (super::style::RadialShape::Circle, super::style::RadialSize::ClosestCorner) => {
              let candidates = [
                (cx, cy),
                (cx, dy_bottom),
                (dx_right, cy),
                (dx_right, dy_bottom),
              ];
              let r = candidates
                .iter()
                .map(|(dx, dy)| (dx * dx + dy * dy).sqrt())
                .fold(f32::INFINITY, f32::min);
              (r, r)
            }
          };

          let r = radius_x.max(radius_y);
          let radius_scale = radius_x.max(radius_y);
          let resolved_stops =
            resolve_stops_along_axis(&gradient.stops, radius_scale.max(1e-6), context);

          let mut stops = Vec::new();
          for stop in resolved_stops {
            let color = format!(
              "rgba({},{},{},{})",
              stop.color.0[0],
              stop.color.0[1],
              stop.color.0[2],
              stop.color.0[3] as f32 / 255.0
            );
            stops.push((stop.position / radius_scale.max(1e-6), color));
          }

          let fill = svg.add_radial_gradient(
            &format!("{}", cx),
            &format!("{}", cy),
            &format!("{}", r),
            None,
            None,
            &stops,
          );

          svg.fill_rect_with_fill(draw_size, &fill, border_radius, draw_transform);
        }
        BackgroundImage::Url(url) => {
          // For now, we just draw the image without tiling support
          svg.draw_image(url, draw_size, draw_transform);
        }
        _ => {
          // TODO: Implement noise in SVG
        }
      }
    }

    Ok(())
  }

  /// Draws the main content of the node.
  fn draw_content(
    &self,
    _context: &RenderContext,
    _canvas: &mut Canvas,
    _layout: Layout,
  ) -> Result<()> {
    // Default implementation does nothing
    Ok(())
  }

  /// Draws the main content of the node as SVG.
  fn draw_content_svg(
    &self,
    _context: &RenderContext,
    _svg: &mut crate::rendering::SvgRenderer,
    _layout: Layout,
  ) -> Result<()> {
    // Default implementation does nothing
    Ok(())
  }

  /// Draws the border of the node.
  fn draw_border(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    let clip_image = if context.style.background_clip == BackgroundClip::BorderArea {
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

    BorderProperties::from_context(context, layout.size, layout.border).draw(
      canvas,
      layout.size,
      context.transform,
      clip_image.as_ref(),
    );
    Ok(())
  }
}

/// Represents the nodes enum.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NodeKind {
  /// A node that contains other nodes.
  Container(ContainerNode<NodeKind>),
  /// A node that displays an image.
  Image(ImageNode),
  /// A node that displays text.
  Text(TextNode),
}

impl_node_enum!(
  NodeKind,
  Container => ContainerNode<NodeKind>,
  Image => ImageNode,
  Text => TextNode
);
