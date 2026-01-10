#![deny(clippy::unwrap_used, clippy::expect_used)]
#![allow(
  clippy::module_name_repetitions,
  clippy::missing_errors_doc,
  clippy::missing_panics_doc,
  clippy::must_use_candidate
)]

mod load_font_task;
mod put_persistent_image_task;
mod render_animation_task;
mod render_task;
mod renderer;

use std::{fmt::Display, ops::Deref};

use napi::{De, Env, Error, JsString, JsValue, bindgen_prelude::*};
pub use renderer::Renderer;
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use takumi::parley::FontStyle;

#[derive(Deserialize, Default)]
pub(crate) struct FontInput {
  pub name: Option<String>,
  pub weight: Option<f64>,
  pub style: Option<FontStyleInput>,
}

#[derive(Clone, Copy)]
pub struct FontStyleInput(pub FontStyle);

impl<'de> Deserialize<'de> for FontStyleInput {
  fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    Ok(FontStyleInput(FontStyle::parse(&s).unwrap_or_default()))
  }
}

// fetch(url: string): Promise<Response>
pub(crate) type FetchFn<'env> = Function<'env, JsString<'env>, PromiseRaw<'env, Object<'env>>>;

/// Somehow this didn't always return a proper `ArrayBuffer` that napi could handle properly: "Failed to get Buffer pointer and length",
/// have to manually convert it to `Uint8Array`.
/// arrayBuffer(this: Response): Promise<ArrayBuffer>
pub(crate) type ArrayBufferFn<'env> = Function<'env, (), PromiseRaw<'env, Object<'env>>>;

pub(crate) enum MaybeInitialized<B, A> {
  Uninitialized(B),
  Initialized(A),
}

fn buffer_from_object(env: Env, value: Object) -> Result<Buffer> {
  if let Ok(buffer) = unsafe { ArrayBuffer::from_napi_value(env.raw(), value.raw()) } {
    return Ok((*buffer).into());
  }

  unsafe { Buffer::from_napi_value(env.raw(), value.raw()) }
}

pub(crate) enum BufferOrSlice<'env> {
  ArrayBuffer(ArrayBuffer<'env>),
  Slice(BufferSlice<'env>),
}

impl<'env> Deref for BufferOrSlice<'env> {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    match self {
      BufferOrSlice::ArrayBuffer(buffer) => buffer,
      BufferOrSlice::Slice(buffer) => buffer,
    }
  }
}

pub(crate) fn buffer_slice_from_object<'env>(
  env: Env,
  value: Object,
) -> Result<BufferOrSlice<'env>> {
  if let Ok(buffer) = unsafe { ArrayBuffer::from_napi_value(env.raw(), value.raw()) } {
    return Ok(BufferOrSlice::ArrayBuffer(buffer));
  }

  unsafe { BufferSlice::from_napi_value(env.raw(), value.raw()).map(BufferOrSlice::Slice) }
}

pub(crate) fn deserialize_with_tracing<T: DeserializeOwned>(value: Object) -> Result<T> {
  let mut de = De::new(&value);
  T::deserialize(&mut de).map_err(|e| Error::from_reason(e.to_string()))
}

pub(crate) fn map_error<E: Display>(err: E) -> napi::Error {
  napi::Error::from_reason(err.to_string())
}
