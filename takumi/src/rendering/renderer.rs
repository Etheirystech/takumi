use crate::{
  layout::style::{Affine, Color},
  rendering::BorderProperties,
};
use taffy::Size;
use xmlwriter::{Options, XmlWriter};
use zeno::Command;

/// SVG renderer that generates vector XML output.
pub struct SvgRenderer {
  writer: XmlWriter,
  defs: XmlWriter,
  width: f32,
  height: f32,
  gradient_count: usize,
  in_defs: bool,
}

impl SvgRenderer {
  /// Creates a new SVG renderer with the specified dimensions.
  pub fn new(width: u32, height: u32) -> Self {
    let options = Options::default();
    Self {
      writer: XmlWriter::new(options),
      defs: XmlWriter::new(options),
      width: width as f32,
      height: height as f32,
      gradient_count: 0,
      in_defs: false,
    }
  }

  /// Returns the SVG content as a string.
  pub fn into_string(mut self) -> String {
    if self.in_defs {
      self.defs.end_element();
    }

    self.writer.end_element();

    let main_content = self.writer.end_document();
    let defs_content = self.defs.end_document();

    let mut result = String::with_capacity(main_content.len() + defs_content.len() + 50);
    result.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    result.push('\n');

    if !defs_content.is_empty() {
      if let Some(pos) = main_content.rfind("</svg>") {
        result.push_str(&main_content[..pos]);
        result.push_str(&defs_content);
        result.push_str(&main_content[pos..]);
      } else {
        result.push_str(&main_content);
      }
    } else {
      result.push_str(&main_content);
    }

    result
  }

  fn color_to_svg(&self, color: Color) -> String {
    format!(
      "rgba({},{},{},{:.3})",
      color.0[0],
      color.0[1],
      color.0[2],
      color.0[3] as f32 / 255.0
    )
  }

  fn transform_to_svg(&self, transform: Affine) -> String {
    format!(
      "matrix({},{},{},{},{},{})",
      transform.a, transform.b, transform.c, transform.d, transform.x, transform.y
    )
  }

  fn commands_to_path(&self, commands: &[Command]) -> String {
    let mut path = String::new();
    for cmd in commands {
      match cmd {
        Command::MoveTo(p) => {
          path.push_str(&format!("M{} {}", p.x, p.y));
        }
        Command::LineTo(p) => {
          path.push_str(&format!("L{} {}", p.x, p.y));
        }
        Command::QuadTo(p1, p2) => {
          path.push_str(&format!("Q{} {},{} {}", p1.x, p1.y, p2.x, p2.y));
        }
        Command::CurveTo(p1, p2, p3) => {
          path.push_str(&format!(
            "C{} {},{} {},{} {}",
            p1.x, p1.y, p2.x, p2.y, p3.x, p3.y
          ));
        }
        Command::Close => {
          path.push('Z');
        }
      }
      path.push(' ');
    }
    path
  }

  /// Writes the SVG header element to the buffer.
  pub fn write_svg_header(&mut self) {
    self.writer.start_element("svg");
    self
      .writer
      .write_attribute("xmlns", "http://www.w3.org/2000/svg");
    self.writer.write_attribute_fmt(
      "viewBox",
      format_args!("0 0 {} {}", self.width, self.height),
    );
    self
      .writer
      .write_attribute_fmt("width", format_args!("{}", self.width));
    self
      .writer
      .write_attribute_fmt("height", format_args!("{}", self.height));
  }

  /// Writes the SVG footer element to the buffer.
  pub fn write_svg_footer(&mut self) {
    self.writer.end_element();
  }

  fn start_defs(&mut self) {
    if !self.in_defs {
      self.defs.start_element("defs");
      self.in_defs = true;
    }
  }

  pub(crate) fn fill_rect(
    &mut self,
    size: Size<f32>,
    color: Color,
    border: BorderProperties,
    transform: Affine,
  ) {
    if color.0[3] == 0 {
      return;
    }

    let fill = self.color_to_svg(color);

    if !transform.is_identity() {
      self.writer.start_element("g");
      self
        .writer
        .write_attribute("transform", &self.transform_to_svg(transform));
    }

    self.writer.start_element("rect");
    self.writer.write_attribute("x", "0");
    self.writer.write_attribute("y", "0");
    self
      .writer
      .write_attribute_fmt("width", format_args!("{}", size.width));
    self
      .writer
      .write_attribute_fmt("height", format_args!("{}", size.height));
    self.writer.write_attribute("fill", &fill);

    if border.radius.0[0].x > 0.0 {
      self
        .writer
        .write_attribute_fmt("rx", format_args!("{}", border.radius.0[0].x));
    }

    self.writer.end_element();

    if !transform.is_identity() {
      self.writer.end_element();
    }
  }

  pub(crate) fn draw_text(&mut self, commands: &[Command], color: Color, transform: Affine) {
    let path_d = self.commands_to_path(commands);

    self.writer.start_element("path");
    self.writer.write_attribute("d", &path_d);
    self
      .writer
      .write_attribute("fill", &self.color_to_svg(color));

    if !transform.is_identity() {
      self
        .writer
        .write_attribute("transform", &self.transform_to_svg(transform));
    }

    self.writer.end_element();
  }

  pub(crate) fn draw_border(
    &mut self,
    size: Size<f32>,
    border: BorderProperties,
    transform: Affine,
  ) {
    if border.width == Default::default() {
      return;
    }

    if !transform.is_identity() {
      self.writer.start_element("g");
      self
        .writer
        .write_attribute("transform", &self.transform_to_svg(transform));
    }

    let x = border.width.left;
    let y = border.width.top;
    let width = size.width - border.width.left - border.width.right;
    let height = size.height - border.width.top - border.width.bottom;
    let stroke_width = border
      .width
      .top
      .max(border.width.right)
      .max(border.width.bottom)
      .max(border.width.left);
    let rx = border.radius.0[0].x;

    self.writer.start_element("rect");
    self.writer.write_attribute_fmt("x", format_args!("{}", x));
    self.writer.write_attribute_fmt("y", format_args!("{}", y));
    self
      .writer
      .write_attribute_fmt("width", format_args!("{}", width));
    self
      .writer
      .write_attribute_fmt("height", format_args!("{}", height));
    self.writer.write_attribute("fill", "none");
    self
      .writer
      .write_attribute("stroke", &self.color_to_svg(border.color));
    self
      .writer
      .write_attribute_fmt("stroke-width", format_args!("{}", stroke_width));
    self
      .writer
      .write_attribute_fmt("rx", format_args!("{}", rx));
    self.writer.end_element();

    if !transform.is_identity() {
      self.writer.end_element();
    }
  }

  pub(crate) fn draw_background_color(
    &mut self,
    size: Size<f32>,
    color: Color,
    border: BorderProperties,
    transform: Affine,
  ) {
    self.fill_rect(size, color, border, transform);
  }

  pub(crate) fn add_linear_gradient(
    &mut self,
    x1: &str,
    y1: &str,
    x2: &str,
    y2: &str,
    stops: &[(f32, String)],
  ) -> String {
    self.start_defs();

    self.gradient_count += 1;
    let id = format!("grad{}", self.gradient_count);

    self.defs.start_element("linearGradient");
    self.defs.write_attribute("id", &id);
    self.defs.write_attribute("x1", x1);
    self.defs.write_attribute("y1", y1);
    self.defs.write_attribute("x2", x2);
    self.defs.write_attribute("y2", y2);
    self.defs.write_attribute("gradientUnits", "userSpaceOnUse");

    for (offset, color) in stops {
      self.defs.start_element("stop");
      self
        .defs
        .write_attribute_fmt("offset", format_args!("{}%", offset * 100.0));
      self.defs.write_attribute("stop-color", color);
      self.defs.end_element();
    }

    self.defs.end_element();

    format!("url(#{id})")
  }

  pub(crate) fn fill_rect_with_fill(
    &mut self,
    size: Size<f32>,
    fill: &str,
    border: BorderProperties,
    transform: Affine,
  ) {
    if !transform.is_identity() {
      self.writer.start_element("g");
      self
        .writer
        .write_attribute("transform", &self.transform_to_svg(transform));
    }

    self.writer.start_element("rect");
    self.writer.write_attribute("x", "0");
    self.writer.write_attribute("y", "0");
    self
      .writer
      .write_attribute_fmt("width", format_args!("{}", size.width));
    self
      .writer
      .write_attribute_fmt("height", format_args!("{}", size.height));
    self.writer.write_attribute("fill", fill);

    if border.radius.0[0].x > 0.0 {
      self
        .writer
        .write_attribute_fmt("rx", format_args!("{}", border.radius.0[0].x));
    }

    self.writer.end_element();

    if !transform.is_identity() {
      self.writer.end_element();
    }
  }

  pub(crate) fn draw_image(&mut self, href: &str, size: Size<f32>, transform: Affine) {
    if !transform.is_identity() {
      self.writer.start_element("g");
      self
        .writer
        .write_attribute("transform", &self.transform_to_svg(transform));
    }

    self.writer.start_element("image");
    self.writer.write_attribute("href", href);
    self.writer.write_attribute("x", "0");
    self.writer.write_attribute("y", "0");
    self
      .writer
      .write_attribute_fmt("width", format_args!("{}", size.width));
    self
      .writer
      .write_attribute_fmt("height", format_args!("{}", size.height));
    self.writer.end_element();

    if !transform.is_identity() {
      self.writer.end_element();
    }
  }

  pub(crate) fn add_shadow_filter(
    &mut self,
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    color: &str,
  ) -> String {
    self.start_defs();

    self.gradient_count += 1;
    let id = format!("shadow{}", self.gradient_count);
    let std_dev = blur_radius / 2.0;

    self.defs.start_element("filter");
    self.defs.write_attribute("id", &id);
    self.defs.write_attribute("x", "-50%");
    self.defs.write_attribute("y", "-50%");
    self.defs.write_attribute("width", "200%");
    self.defs.write_attribute("height", "200%");

    self.defs.start_element("feGaussianBlur");
    self.defs.write_attribute("in", "SourceAlpha");
    self
      .defs
      .write_attribute_fmt("stdDeviation", format_args!("{}", std_dev));
    self.defs.end_element();

    self.defs.start_element("feOffset");
    self
      .defs
      .write_attribute_fmt("dx", format_args!("{}", offset_x));
    self
      .defs
      .write_attribute_fmt("dy", format_args!("{}", offset_y));
    self.defs.write_attribute("result", "offsetblur");
    self.defs.end_element();

    self.defs.start_element("feFlood");
    self.defs.write_attribute("flood-color", color);
    self.defs.end_element();

    self.defs.start_element("feComposite");
    self.defs.write_attribute("in2", "offsetblur");
    self.defs.write_attribute("operator", "in");
    self.defs.end_element();

    self.defs.start_element("feMerge");
    self.defs.start_element("feMergeNode");
    self.defs.end_element();
    self.defs.end_element();

    self.defs.end_element();

    format!("url(#{id})")
  }

  pub(crate) fn add_inset_shadow_filter(
    &mut self,
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    color: &str,
  ) -> String {
    self.start_defs();

    self.gradient_count += 1;
    let id = format!("insetShadow{}", self.gradient_count);
    let std_dev = blur_radius / 2.0;

    self.defs.start_element("filter");
    self.defs.write_attribute("id", &id);
    self.defs.write_attribute("x", "-50%");
    self.defs.write_attribute("y", "-50%");
    self.defs.write_attribute("width", "200%");
    self.defs.write_attribute("height", "200%");

    self.defs.start_element("feGaussianBlur");
    self.defs.write_attribute("in", "SourceAlpha");
    self
      .defs
      .write_attribute_fmt("stdDeviation", format_args!("{}", std_dev));
    self.defs.end_element();

    self.defs.start_element("feOffset");
    self
      .defs
      .write_attribute_fmt("dx", format_args!("{}", offset_x));
    self
      .defs
      .write_attribute_fmt("dy", format_args!("{}", offset_y));
    self.defs.write_attribute("result", "offsetblur");
    self.defs.end_element();

    self.defs.start_element("feFlood");
    self.defs.write_attribute("flood-color", color);
    self.defs.end_element();

    self.defs.start_element("feComposite");
    self.defs.write_attribute("in2", "offsetblur");
    self.defs.write_attribute("operator", "in");
    self.defs.end_element();

    self.defs.start_element("feComposite");
    self.defs.write_attribute("in2", "SourceAlpha");
    self.defs.write_attribute("operator", "in");
    self.defs.end_element();

    self.defs.end_element();

    format!("url(#{id})")
  }

  pub(crate) fn add_radial_gradient(
    &mut self,
    cx: &str,
    cy: &str,
    r: &str,
    fx: Option<&str>,
    fy: Option<&str>,
    stops: &[(f32, String)],
  ) -> String {
    self.start_defs();

    self.gradient_count += 1;
    let id = format!("radialGrad{}", self.gradient_count);

    self.defs.start_element("radialGradient");
    self.defs.write_attribute("id", &id);
    self.defs.write_attribute("cx", cx);
    self.defs.write_attribute("cy", cy);
    self.defs.write_attribute("r", r);
    self.defs.write_attribute("gradientUnits", "userSpaceOnUse");

    if let (Some(fx), Some(fy)) = (fx, fy) {
      self.defs.write_attribute("fx", fx);
      self.defs.write_attribute("fy", fy);
    }

    for (offset, color) in stops {
      self.defs.start_element("stop");
      self
        .defs
        .write_attribute_fmt("offset", format_args!("{}%", offset * 100.0));
      self.defs.write_attribute("stop-color", color);
      self.defs.end_element();
    }

    self.defs.end_element();

    format!("url(#{id})")
  }

  pub(crate) fn fill_rect_with_filter(
    &mut self,
    size: Size<f32>,
    color: Color,
    filter: &str,
    border: BorderProperties,
    transform: Affine,
  ) {
    if !transform.is_identity() {
      self.writer.start_element("g");
      self
        .writer
        .write_attribute("transform", &self.transform_to_svg(transform));
    }

    self.writer.start_element("rect");
    self.writer.write_attribute("x", "0");
    self.writer.write_attribute("y", "0");
    self
      .writer
      .write_attribute_fmt("width", format_args!("{}", size.width));
    self
      .writer
      .write_attribute_fmt("height", format_args!("{}", size.height));
    self
      .writer
      .write_attribute("fill", &self.color_to_svg(color));
    self.writer.write_attribute("filter", filter);

    if border.radius.0[0].x > 0.0 {
      self
        .writer
        .write_attribute_fmt("rx", format_args!("{}", border.radius.0[0].x));
    }

    self.writer.end_element();

    if !transform.is_identity() {
      self.writer.end_element();
    }
  }
}
