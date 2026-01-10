use napi::bindgen_prelude::*;
use takumi::resources::image::{PersistentImageStore, load_image_source_from_bytes};

use crate::map_error;

pub struct PutPersistentImageTask<'s> {
  pub src: Option<String>,
  pub store: &'s PersistentImageStore,
  pub buffer: Buffer,
}

impl Task for PutPersistentImageTask<'_> {
  type Output = ();
  type JsValue = ();

  fn compute(&mut self) -> Result<Self::Output> {
    let Some(src) = self.src.take() else {
      unreachable!()
    };

    let image = load_image_source_from_bytes(&self.buffer).map_err(map_error)?;
    self.store.insert(src, image);

    Ok(())
  }

  fn resolve(&mut self, _env: napi::Env, _output: Self::Output) -> napi::Result<Self::JsValue> {
    Ok(())
  }
}
