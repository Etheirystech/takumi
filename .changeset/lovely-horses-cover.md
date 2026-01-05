---
"@takumi-rs/image-response": major
"@takumi-rs/core": major
"@takumi-rs/wasm": major
"takumi": major
"@takumi-rs/helpers": major
---

Stabilize API and Finalize v1.0 Release 🎉

This major release stabilizes the public API surface and removes deprecated features.

### Rust Changes

- Access to `GlobalContext` fields are now private. Should use the new accessor methods: `font_context()`, `font_context_mut()`, `persistent_image_store()`, and `persistent_image_store_mut()`.
- `CssValue` enum is now `#[non_exhaustive]`. To allow adding new global keywords in the future without breaking existing match expressions.

### `@takumi-rs/core` and `@takumi-rs/wasm` Changes

- Removed all deprecated `_async` suffixed methods.
  - `render_async` -> `render`
  - `load_font_async` / `load_font_with_info` -> `load_font`
  - `load_fonts_async` -> `load_fonts`
  - `put_persistent_image_async` -> `put_persistent_image`
- Removed `purge_font_cache` method.
- Standardized `OutputFormat` enum variants to lowercase.
  - Change `WebP` -> `webp`, `Png` -> `png`, and `Jpeg` -> `jpeg`.
