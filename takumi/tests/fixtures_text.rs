use takumi::{
  Color, DefaultNodeKind, FontWeight, InheritableStyle,
  LengthUnit::{Percentage, Px},
  Style, TextAlign, TextNode, TextOverflow,
};

mod test_utils;
use test_utils::run_style_width_test;

// Basic text render with defaults
#[test]
fn fixtures_text_basic() {
  let text = TextNode {
    style: Style {
      background_color: Some(Color::Rgb(240, 240, 240)),
      ..Default::default()
    },
    text: "🦊 The quick brown fox jumps over the lazy dog 12345".to_string(),
  };

  run_style_width_test(DefaultNodeKind::Text(text), "tests/fixtures/text_basic.png");
}

#[test]
fn fixtures_text_typography_regular_24px() {
  let text = TextNode {
    style: Style {
      background_color: Some(Color::Rgb(240, 240, 240)),
      inheritable_style: InheritableStyle {
        font_size: Some(Px(24.0)),
        ..Default::default()
      },
      ..Default::default()
    },
    text: "Regular 24px".to_string(),
  };

  run_style_width_test(
    text.into(),
    "tests/fixtures/text_typography_regular_24px.png",
  );
}

#[test]
fn fixtures_text_typography_medium_weight_500() {
  let text = TextNode {
    style: Style {
      background_color: Some(Color::Rgb(240, 240, 240)),
      inheritable_style: InheritableStyle {
        font_size: Some(Px(24.0)),
        font_weight: Some(FontWeight(500)),
        ..Default::default()
      },
      ..Default::default()
    },
    text: "Medium 24px".to_string(),
  };

  run_style_width_test(
    text.into(),
    "tests/fixtures/text_typography_medium_weight_500.png",
  );
}

#[test]
fn fixtures_text_typography_line_height_40px() {
  let text = TextNode {
    style: Style {
      background_color: Some(Color::Rgb(240, 240, 240)),
      inheritable_style: InheritableStyle {
        font_size: Some(Px(24.0)),
        line_height: Some(Px(40.0)),
        ..Default::default()
      },
      ..Default::default()
    },
    text: "Line height 40px".to_string(),
  };

  run_style_width_test(
    text.into(),
    "tests/fixtures/text_typography_line_height_40px.png",
  );
}

#[test]
fn fixtures_text_typography_letter_spacing_2px() {
  let text = TextNode {
    style: Style {
      background_color: Some(Color::Rgb(240, 240, 240)),
      inheritable_style: InheritableStyle {
        font_size: Some(Px(24.0)),
        letter_spacing: Some(Px(2.0)),
        ..Default::default()
      },
      ..Default::default()
    },
    text: "Letter spacing 2px".to_string(),
  };

  run_style_width_test(
    text.into(),
    "tests/fixtures/text_typography_letter_spacing_2px.png",
  );
}

#[test]
fn fixtures_text_align_start() {
  let text = TextNode {
    style: Style {
      background_color: Some(Color::Rgb(240, 240, 240)),
      width: Percentage(100.0),
      inheritable_style: InheritableStyle {
        font_size: Some(Px(24.0)),
        text_align: Some(TextAlign::Start),
        ..Default::default()
      },
      ..Default::default()
    },
    text: "Start aligned".to_string(),
  };

  run_style_width_test(text.into(), "tests/fixtures/text_align_start.png");
}

#[test]
fn fixtures_text_align_center() {
  let text = TextNode {
    style: Style {
      background_color: Some(Color::Rgb(240, 240, 240)),
      width: Percentage(100.0),
      inheritable_style: InheritableStyle {
        font_size: Some(Px(24.0)),
        text_align: Some(TextAlign::Center),
        ..Default::default()
      },
      ..Default::default()
    },
    text: "Center aligned".to_string(),
  };

  run_style_width_test(text.into(), "tests/fixtures/text_align_center.png");
}

#[test]
fn fixtures_text_align_right() {
  let text = TextNode {
    style: Style {
      background_color: Some(Color::Rgb(240, 240, 240)),
      width: Percentage(100.0),
      inheritable_style: InheritableStyle {
        font_size: Some(Px(24.0)),
        text_align: Some(TextAlign::Right),
        ..Default::default()
      },
      ..Default::default()
    },
    text: "Right aligned".to_string(),
  };

  run_style_width_test(text.into(), "tests/fixtures/text_align_right.png");
}

#[test]
fn fixtures_text_justify_clip() {
  let long_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. \
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.";

  let text = TextNode {
    style: Style {
      background_color: Some(Color::Rgb(240, 240, 240)),
      inheritable_style: InheritableStyle {
        font_size: Some(Px(18.0)),
        line_height: Some(Px(26.0)),
        text_align: Some(TextAlign::Justify),
        text_overflow: Some(TextOverflow::Clip),
        ..Default::default()
      },
      ..Default::default()
    },
    text: long_text.to_string(),
  };

  run_style_width_test(text.into(), "tests/fixtures/text_justify_clip.png");
}

#[test]
fn fixtures_text_ellipsis_line_clamp_2() {
  let long_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. \
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.";

  let text = TextNode {
    style: Style {
      background_color: Some(Color::Rgb(240, 240, 240)),
      inheritable_style: InheritableStyle {
        font_size: Some(Px(18.0)),
        line_height: Some(Px(24.0)),
        text_overflow: Some(TextOverflow::Ellipsis),
        line_clamp: Some(2),
        ..Default::default()
      },
      ..Default::default()
    },
    text: long_text.to_string(),
  };

  run_style_width_test(text.into(), "tests/fixtures/text_ellipsis_line_clamp_2.png");
}

#[test]
fn fixtures_text_arabic_rtl() {
  let text = TextNode {
    style: Style {
      background_color: Some(Color::Rgb(240, 240, 240)),
      inheritable_style: InheritableStyle {
        font_size: Some(Px(24.0)),
        ..Default::default()
      },
      width: Percentage(100.0),
      ..Default::default()
    },
    // Arabic with punctuation and mixed Western/Arabic-Indic digits to exercise shaping and bidi
    text: "مرحبا بالعالم — الاختبار 123٤٥٦، العربية!".to_string(),
  };

  run_style_width_test(text.into(), "tests/fixtures/text_arabic_rtl.png");
}