use takumi::layout::{
  node::{ContainerNode, NodeKind, TextNode},
  style::{Length::*, *},
};

use crate::test_utils::run_fixture_test;

#[test]
fn test_svg_basic_rect() {
  let node = NodeKind::Container(ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(200.0))
        .height(Px(100.0))
        .background_color(ColorInput::Value(Color([255, 0, 0, 255])))
        .build()
        .unwrap(),
    ),
    children: None,
  });

  run_fixture_test(node, "basic_rect");
}

#[test]
fn test_svg_with_text() {
  let node = NodeKind::Container(ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(400.0))
        .height(Px(200.0))
        .background_color(ColorInput::Value(Color([0, 0, 255, 255])))
        .display(Display::Flex)
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .build()
        .unwrap(),
    ),
    children: Some(
      [NodeKind::Text(TextNode {
        text: "Hello SVG".to_string(),
        preset: None,
        tw: None,
        style: Some(
          StyleBuilder::default()
            .color(ColorInput::Value(Color([255, 255, 255, 255])))
            .font_size(Px(48.0))
            .build()
            .unwrap(),
        ),
      })]
      .into(),
    ),
  });

  run_fixture_test(node, "text_output");
}

#[test]
fn test_svg_rounded_border() {
  let node = NodeKind::Container(ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(200.0))
        .height(Px(200.0))
        .background_color(ColorInput::Value(Color([0, 255, 0, 255])))
        .border(Border {
          width: Px(10.0).into(),
          color: Some(ColorInput::Value(Color([0, 0, 0, 255]))),
          ..Default::default()
        })
        .border_radius(BorderRadius(Sides(
          [SpacePair::from_single(Length::<false>::Px(50.0)); 4],
        )))
        .build()
        .unwrap(),
    ),
    children: None,
  });

  run_fixture_test(node, "rounded_border");
}

#[test]
fn test_svg_box_shadow() {
  let node = NodeKind::Container(ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(200.0))
        .height(Px(200.0))
        .background_color(ColorInput::Value(Color([255, 255, 255, 255])))
        .box_shadow(BoxShadows::from_str("10px 10px 20px rgba(0,0,0,0.5)").unwrap())
        .build()
        .unwrap(),
    ),
    children: None,
  });

  run_fixture_test(node, "box_shadow");
}

#[test]
fn test_svg_linear_gradient() {
  let node = NodeKind::Container(ContainerNode {
    preset: None,
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Px(300.0))
        .height(Px(300.0))
        .background_image(
          BackgroundImages::from_str("linear-gradient(to right, red, blue)").unwrap(),
        )
        .build()
        .unwrap(),
    ),
    children: None,
  });

  run_fixture_test(node, "linear_gradient");
}
