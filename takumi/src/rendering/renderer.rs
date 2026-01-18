use crate::{
  layout::style::{Affine, Color},
  rendering::BorderProperties,
};
use taffy::Size;
use zeno::Command;

/// SVG renderer that generates vector XML output.
pub struct SvgRenderer {
  buffer: String,
  defs: String,
  width: f32,
  height: f32,
  indent: usize,
  gradient_count: usize,
}

impl SvgRenderer {
  /// Creates a new SVG renderer with the specified dimensions.
  pub fn new(width: u32, height: u32) -> Self {
    Self {
      buffer: String::new(),
      defs: String::new(),
      width: width as f32,
      height: height as f32,
      indent: 0,
      gradient_count: 0,
    }
  }

  /// Returns the SVG content as a string.
  pub fn into_string(self) -> String {
    let mut final_svg = self.buffer;
    if !self.defs.is_empty() {
      // Insert defs after the second line (SVG tag)
      let mut pos = 0;
      for _ in 0..2 {
        if let Some(next_pos) = final_svg[pos..].find('\n') {
          pos += next_pos + 1;
        } else {
          break;
        }
      }

      if pos > 0 {
        let mut defs_block = String::from("  <defs>\n");
        defs_block.push_str(&self.defs);
        defs_block.push_str("  </defs>\n");
        final_svg.insert_str(pos, &defs_block);
      }
    }
    final_svg
  }

  fn indent(&mut self) {
    for _ in 0..self.indent {
      self.buffer.push(' ');
    }
  }

  fn write_line(&mut self, line: &str) {
    self.indent();
    self.buffer.push_str(line);
    self.buffer.push('\n');
  }

  fn write_open_tag(&mut self, tag: &str, attrs: &[(&str, impl AsRef<str>)]) {
    self.indent();
    self.buffer.push('<');
    self.buffer.push_str(tag);
    for (key, value) in attrs {
      self.buffer.push(' ');
      self.buffer.push_str(key);
      self.buffer.push_str("=\"");
      self.buffer.push_str(value.as_ref());
      self.buffer.push('\"');
    }
    self.buffer.push_str(">\n");
    self.indent += 2;
  }

  fn write_close_tag(&mut self, tag: &str) {
    self.indent -= 2;
    self.indent();
    self.buffer.push_str("</");
    self.buffer.push_str(tag);
    self.buffer.push_str(">\n");
  }

  fn write_self_closing_tag(&mut self, tag: &str, attrs: &[(&str, impl AsRef<str>)]) {
    self.indent();
    self.buffer.push('<');
    self.buffer.push_str(tag);
    for (key, value) in attrs {
      self.buffer.push(' ');
      self.buffer.push_str(key);
      self.buffer.push_str("=\"");
      self.buffer.push_str(value.as_ref());
      self.buffer.push('\"');
    }
    self.buffer.push_str("/>\n");
  }

  fn color_to_svg(&self, color: Color) -> String {
    format!(
      "rgba({},{},{},{})",
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
    self.write_line(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    self.write_line(&format!(
      r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
      self.width, self.height, self.width, self.height
    ));
  }

  /// Writes the SVG footer closing element to the buffer.
  pub fn write_svg_footer(&mut self) {
    self.write_line(r#"</svg>"#);
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

    let width = format!("{}", size.width);
    let height = format!("{}", size.height);
    let fill = self.color_to_svg(color);

    let mut attrs: Vec<(&str, String)> = vec![
      ("x", "0".to_string()),
      ("y", "0".to_string()),
      ("width", width),
      ("height", height),
      ("fill", fill),
    ];

    if border.radius.0[0].x > 0.0 {
      attrs.push(("rx", format!("{}", border.radius.0[0].x)));
    }

    if !transform.is_identity() {
      let transform_str = self.transform_to_svg(transform);
      self.write_open_tag("g", &[("transform", &transform_str)]);
    }

    let attr_refs: Vec<(&str, &str)> = attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    self.write_self_closing_tag("rect", &attr_refs);

    if !transform.is_identity() {
      self.write_close_tag("g");
    }
  }

  pub(crate) fn draw_text(&mut self, commands: &[Command], color: Color, transform: Affine) {
    let path_d = self.commands_to_path(commands);
    let transform_str = if !transform.is_identity() {
      self.transform_to_svg(transform)
    } else {
      String::new()
    };

    let fill_color = self.color_to_svg(color);

    let mut attrs: Vec<(&str, &str)> = vec![("d", &path_d), ("fill", &fill_color)];

    if !transform_str.is_empty() {
      attrs.push(("transform", &transform_str));
    }

    self.write_self_closing_tag("path", &attrs);
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

    let transform_str = if !transform.is_identity() {
      self.transform_to_svg(transform)
    } else {
      String::new()
    };

    let x = format!("{}", border.width.left);
    let y = format!("{}", border.width.top);
    let width = format!("{}", size.width - border.width.left - border.width.right);
    let height = format!("{}", size.height - border.width.top - border.width.bottom);
    let stroke_width = format!(
      "{}",
      border
        .width
        .top
        .max(border.width.right)
        .max(border.width.bottom)
        .max(border.width.left)
    );
    let rx = format!("{}", border.radius.0[0].x);
    let stroke = self.color_to_svg(border.color);

    let attrs: Vec<(&str, &str)> = vec![
      ("x", &x),
      ("y", &y),
      ("width", &width),
      ("height", &height),
      ("fill", "none"),
      ("stroke", &stroke),
      ("stroke-width", &stroke_width),
      ("rx", &rx),
    ];

    let mut all_attrs = attrs;

    if !transform_str.is_empty() {
      all_attrs.push(("transform", &transform_str));
    }

    self.write_self_closing_tag("rect", &all_attrs);
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
    self.gradient_count += 1;
    let id = format!("grad{}", self.gradient_count);
    self.defs.push_str(&format!(
      "    <linearGradient id=\"{}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" gradientUnits=\"userSpaceOnUse\">\n",
      id, x1, y1, x2, y2
    ));
    for (offset, color) in stops {
      self.defs.push_str(&format!(
        "      <stop offset=\"{}%\" stop-color=\"{}\" />\n",
        offset * 100.0,
        color
      ));
    }
    self.defs.push_str("    </linearGradient>\n");
    format!("url(#{})", id)
  }

  pub(crate) fn fill_rect_with_fill(
    &mut self,
    size: Size<f32>,
    fill: &str,
    border: BorderProperties,
    transform: Affine,
  ) {
    let width = format!("{}", size.width);
    let height = format!("{}", size.height);

    let mut attrs: Vec<(&str, String)> = vec![
      ("x", "0".to_string()),
      ("y", "0".to_string()),
      ("width", width),
      ("height", height),
      ("fill", fill.to_string()),
    ];

    if border.radius.0[0].x > 0.0 {
      attrs.push(("rx", format!("{}", border.radius.0[0].x)));
    }

    if !transform.is_identity() {
      let transform_str = self.transform_to_svg(transform);
      self.write_open_tag("g", &[("transform", &transform_str)]);
    }

    let attr_refs: Vec<(&str, &str)> = attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    self.write_self_closing_tag("rect", &attr_refs);

    if !transform.is_identity() {
      self.write_close_tag("g");
    }
  }

  pub(crate) fn draw_image(&mut self, href: &str, size: Size<f32>, transform: Affine) {
    let width = format!("{}", size.width);
    let height = format!("{}", size.height);

    let attrs: Vec<(&str, &str)> = vec![
      ("href", href),
      ("x", "0"),
      ("y", "0"),
      ("width", &width),
      ("height", &height),
    ];

    if !transform.is_identity() {
      let transform_str = self.transform_to_svg(transform);
      self.write_open_tag("g", &[("transform", &transform_str)]);
    }

    self.write_self_closing_tag("image", &attrs);

    if !transform.is_identity() {
      self.write_close_tag("g");
    }
  }

  pub(crate) fn add_shadow_filter(
    &mut self,
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    color: &str,
  ) -> String {
    self.gradient_count += 1;
    let id = format!("shadow{}", self.gradient_count);
    let std_dev = blur_radius / 2.0;

    self.defs.push_str(&format!(
      "    <filter id=\"{}\" x=\"-50%\" y=\"-50%\" width=\"200%\" height=\"200%\">\n",
      id
    ));
    self.defs.push_str(&format!(
      "      <feGaussianBlur in=\"SourceAlpha\" stdDeviation=\"{}\" />\n",
      std_dev
    ));
    self.defs.push_str(&format!(
      "      <feOffset dx=\"{}\" dy=\"{}\" result=\"offsetblur\" />\n",
      offset_x, offset_y
    ));
    self
      .defs
      .push_str(&format!("      <feFlood flood-color=\"{}\" />\n", color));
    self
      .defs
      .push_str("      <feComposite in2=\"offsetblur\" operator=\"in\" />\n");
    self.defs.push_str("      <feMerge>\n");
    self.defs.push_str("        <feMergeNode />\n");
    self.defs.push_str("      </feMerge>\n");
    self.defs.push_str("    </filter>\n");

    format!("url(#{})", id)
  }

  pub(crate) fn fill_rect_with_filter(
    &mut self,
    size: Size<f32>,
    color: Color,
    filter: &str,
    border: BorderProperties,
    transform: Affine,
  ) {
    let width = format!("{}", size.width);
    let height = format!("{}", size.height);
    let fill = self.color_to_svg(color);

    let mut attrs: Vec<(&str, String)> = vec![
      ("x", "0".to_string()),
      ("y", "0".to_string()),
      ("width", width),
      ("height", height),
      ("fill", fill),
      ("filter", filter.to_string()),
    ];

    if border.radius.0[0].x > 0.0 {
      attrs.push(("rx", format!("{}", border.radius.0[0].x)));
    }

    if !transform.is_identity() {
      let transform_str = self.transform_to_svg(transform);
      self.write_open_tag("g", &[("transform", &transform_str)]);
    }

    let attr_refs: Vec<(&str, &str)> = attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    self.write_self_closing_tag("rect", &attr_refs);

    if !transform.is_identity() {
      self.write_close_tag("g");
    }
  }
}
