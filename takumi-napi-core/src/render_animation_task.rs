use napi::bindgen_prelude::*;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use takumi::{
  GlobalContext,
  layout::{Viewport, node::NodeKind},
  rendering::{
    AnimationFrame, RenderOptionsBuilder, encode_animated_png, encode_animated_webp, render,
  },
};

use crate::{map_error, renderer::AnimationOutputFormat};

pub struct RenderAnimationTask<'g> {
  pub nodes: Option<Vec<(NodeKind, u32)>>,
  pub context: &'g GlobalContext,
  pub viewport: Viewport,
  pub format: AnimationOutputFormat,
  pub draw_debug_border: bool,
}

impl Task for RenderAnimationTask<'_> {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    let Some(nodes) = self.nodes.take() else {
      unreachable!()
    };

    let frames = nodes
      .into_par_iter()
      .map(|(node, duration_ms)| {
        Ok(AnimationFrame::new(
          render(
            RenderOptionsBuilder::default()
              .viewport(self.viewport)
              .node(node)
              .global(self.context)
              .draw_debug_border(self.draw_debug_border)
              .build()
              .map_err(map_error)?,
          )
          .map_err(map_error)?,
          duration_ms,
        ))
      })
      .collect::<Result<Vec<_>, _>>()?;

    let mut buffer = Vec::new();

    match self.format {
      AnimationOutputFormat::webp => {
        encode_animated_webp(&frames, &mut buffer, true, false, None)
          .map_err(|e| napi::Error::from_reason(e.to_string()))?;
      }
      AnimationOutputFormat::apng => {
        encode_animated_png(&frames, &mut buffer, None)
          .map_err(|e| napi::Error::from_reason(e.to_string()))?;
      }
    }

    Ok(buffer)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
