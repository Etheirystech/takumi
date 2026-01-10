use std::collections::HashMap;

use crossbeam_channel::{Receiver, bounded};
use napi::bindgen_prelude::*;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::sync::Arc;
use takumi::{
  GlobalContext,
  layout::{
    DEFAULT_DEVICE_PIXEL_RATIO, DEFAULT_FONT_SIZE, Viewport,
    node::{Node, NodeKind},
  },
  rendering::{RenderOptionsBuilder, render, write_image},
  resources::{
    image::{ImageSource, load_image_source_from_bytes},
    task::{FetchTask, FetchTaskCollection},
  },
};

use crate::{
  ArrayBufferFn, MaybeInitialized, buffer_from_object, map_error,
  renderer::{OutputFormat, RenderOptions, ResourceCache},
};

pub struct RenderTask<'g> {
  pub draw_debug_border: bool,
  pub node: Option<NodeKind>,
  pub global: &'g GlobalContext,
  pub viewport: Viewport,
  pub format: OutputFormat,
  pub quality: Option<u8>,
  pub(crate) resource_cache: ResourceCache,
  pub(crate) tasks_rx: Receiver<(FetchTask, MaybeInitialized<Buffer, Arc<ImageSource>>)>,
}

impl<'g> RenderTask<'g> {
  pub fn from_options(
    env: Env,
    node: NodeKind,
    options: RenderOptions,
    resources_cache: &ResourceCache,
    global: &'g GlobalContext,
  ) -> Result<Self> {
    let mut collection = FetchTaskCollection::default();

    node.collect_fetch_tasks(&mut collection);
    node.collect_style_fetch_tasks(&mut collection);

    let collection = collection.into_inner();

    let fetch = options
      .fetch
      .or_else(|| {
        env
          .get_global()
          .ok()
          .and_then(|global| global.get_named_property("fetch").ok())
      })
      .ok_or(Error::from_reason(
        "No global fetch() function found. Please provide your own.",
      ))?;

    let (tx, rx) = bounded(1);

    for task in collection {
      if let Some(resources_cache) = resources_cache.as_ref() {
        let mut lock = resources_cache
          .lock()
          .map_err(|e| Error::from_reason(e.to_string()))?;

        if let Some(cached) = lock.get(&task).cloned() {
          drop(lock);

          tx.send((task, MaybeInitialized::Initialized(cached)))
            .map_err(|e| Error::from_reason(e.to_string()))?;

          continue;
        }
      }

      let tx = tx.clone();

      fetch.call(env.create_string(&task)?)?.then(move |ctx| {
        let array_buffer_fn = ctx
          .value
          .get_named_property::<ArrayBufferFn>("arrayBuffer")?;

        array_buffer_fn.apply(ctx.value, ())?.then(move |ctx| {
          tx.send((
            task,
            MaybeInitialized::Uninitialized(buffer_from_object(ctx.env, ctx.value)?),
          ))
          .map_err(|e| Error::from_reason(e.to_string()))?;

          Ok(())
        })
      })?;
    }

    Ok(RenderTask {
      node: Some(node),
      global,
      viewport: Viewport {
        width: options.width,
        height: options.height,
        font_size: DEFAULT_FONT_SIZE,
        device_pixel_ratio: options
          .device_pixel_ratio
          .map(|ratio| ratio as f32)
          .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
      },
      format: options.format.unwrap_or(OutputFormat::png),
      quality: options.quality,
      draw_debug_border: options.draw_debug_border.unwrap_or_default(),
      tasks_rx: rx,
      resource_cache: resources_cache.clone(),
    })
  }
}

impl Task for RenderTask<'_> {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    let Some(node) = self.node.take() else {
      unreachable!()
    };

    let resources: Vec<_> = self.tasks_rx.iter().collect();

    let resource_cache = self.resource_cache.clone();
    let fetched_resources: HashMap<_, _> = resources
      .into_par_iter()
      .filter_map(|(task, buffer)| match buffer {
        MaybeInitialized::Initialized(source) => Some(Ok((task, source))),
        MaybeInitialized::Uninitialized(buffer) => {
          let Ok(source) = load_image_source_from_bytes(&buffer) else {
            return None;
          };

          if let Some(cache) = resource_cache.as_ref() {
            let mut lock = match cache.lock() {
              Ok(l) => l,
              Err(e) => return Some(Err(map_error(e))),
            };

            lock.put(task.clone(), source.clone());
          }

          Some(Ok((task, source)))
        }
      })
      .collect::<Result<HashMap<_, _>, _>>()?;

    let image = render(
      RenderOptionsBuilder::default()
        .viewport(self.viewport)
        .fetched_resources(fetched_resources)
        .node(node)
        .global(self.global)
        .draw_debug_border(self.draw_debug_border)
        .build()
        .map_err(map_error)?,
    )
    .map_err(map_error)?;

    if self.format == OutputFormat::raw {
      return Ok(image.into_raw());
    }

    let mut buffer = Vec::new();

    write_image(&image, &mut buffer, self.format.into(), self.quality).map_err(map_error)?;

    Ok(buffer)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}
