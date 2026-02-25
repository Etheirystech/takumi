//! Asset bundle decryption for client-side rendering.
//!
//! Decrypts AES-256-GCM encrypted asset bundles and returns their contents
//! as a JavaScript `Map<string, Uint8Array>`.
//!
//! ## Bundle format
//!
//! ```text
//! UNENCRYPTED HEADER (17 bytes):
//!   [4 bytes] magic: "TCGB"
//!   [4 bytes] version: u32 LE (1)
//!   [1 byte]  encryption method: 1 = AES-256-GCM
//!   [12 bytes] nonce (IV)
//!
//! ENCRYPTED PAYLOAD (AES-256-GCM, includes 16-byte auth tag at end):
//!   [4 bytes] entry count: u32 LE
//!   INDEX (repeated entry_count times):
//!     [2 bytes] key length: u16 LE
//!     [N bytes] key (UTF-8 asset name)
//!     [4 bytes] data offset: u32 LE (from start of DATA section)
//!     [4 bytes] data length: u32 LE
//!   DATA:
//!     Concatenated raw asset bytes
//! ```

use aes_gcm::{
  Aes256Gcm, Nonce,
  aead::{Aead, KeyInit},
};
use js_sys::{Map, Uint8Array};
use wasm_bindgen::prelude::*;

/// The AES-256-GCM key embedded at compile time.
const BUNDLE_KEY: &[u8; 32] = include_bytes!("bundle_key.bin");

/// Magic bytes identifying a TCGB bundle.
const MAGIC: &[u8; 4] = b"TCGB";

/// Expected bundle format version.
const VERSION: u32 = 1;

/// AES-256-GCM encryption method identifier.
const ENCRYPTION_AES_256_GCM: u8 = 1;

/// Size of the unencrypted header: 4 (magic) + 4 (version) + 1 (encryption) + 12 (nonce).
const HEADER_SIZE: usize = 4 + 4 + 1 + 12;

/// Decrypts an asset bundle and returns a `Map<string, Uint8Array>` of asset entries.
///
/// The input `data` must be the raw bytes of a `.bin` bundle file.
/// Returns a JavaScript `Map` where keys are asset names (e.g. `"assets/Border-Assets/Card-Border.png"`)
/// and values are `Uint8Array` containing the raw asset bytes.
#[wasm_bindgen(js_name = decryptAssetBundle)]
pub fn decrypt_asset_bundle(data: &[u8]) -> Result<Map, js_sys::Error> {
  // --- Parse header ---
  if data.len() < HEADER_SIZE {
    return Err(js_sys::Error::new("Bundle too small: missing header"));
  }

  let magic = &data[0..4];
  if magic != MAGIC {
    return Err(js_sys::Error::new("Invalid bundle: bad magic bytes"));
  }

  let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
  if version != VERSION {
    return Err(js_sys::Error::new(&format!(
      "Unsupported bundle version: {version}"
    )));
  }

  let encryption = data[8];
  if encryption != ENCRYPTION_AES_256_GCM {
    return Err(js_sys::Error::new(&format!(
      "Unsupported encryption method: {encryption}"
    )));
  }

  let nonce_bytes = &data[9..21];
  let nonce = Nonce::from_slice(nonce_bytes);

  let ciphertext = &data[HEADER_SIZE..];

  // --- Decrypt ---
  let cipher =
    Aes256Gcm::new_from_slice(BUNDLE_KEY).map_err(|e| js_sys::Error::new(&e.to_string()))?;

  let plaintext = cipher
    .decrypt(nonce, ciphertext)
    .map_err(|_| js_sys::Error::new("Decryption failed: invalid key or corrupted data"))?;

  // --- Parse decrypted payload ---
  parse_payload(&plaintext)
}

/// Parses the decrypted payload into a JS Map.
fn parse_payload(data: &[u8]) -> Result<Map, js_sys::Error> {
  if data.len() < 4 {
    return Err(js_sys::Error::new(
      "Decrypted payload too small: missing entry count",
    ));
  }

  let entry_count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;

  // Parse index entries
  let mut cursor = 4usize;
  let mut entries: Vec<(String, u32, u32)> = Vec::with_capacity(entry_count);

  for _ in 0..entry_count {
    if cursor + 2 > data.len() {
      return Err(js_sys::Error::new("Truncated index: missing key length"));
    }
    let key_len = u16::from_le_bytes([data[cursor], data[cursor + 1]]) as usize;
    cursor += 2;

    if cursor + key_len > data.len() {
      return Err(js_sys::Error::new("Truncated index: missing key data"));
    }
    let key = std::str::from_utf8(&data[cursor..cursor + key_len])
      .map_err(|e| js_sys::Error::new(&format!("Invalid UTF-8 key: {e}")))?
      .to_string();
    cursor += key_len;

    if cursor + 8 > data.len() {
      return Err(js_sys::Error::new("Truncated index: missing offset/length"));
    }
    let offset = u32::from_le_bytes([
      data[cursor],
      data[cursor + 1],
      data[cursor + 2],
      data[cursor + 3],
    ]);
    cursor += 4;
    let length = u32::from_le_bytes([
      data[cursor],
      data[cursor + 1],
      data[cursor + 2],
      data[cursor + 3],
    ]);
    cursor += 4;

    entries.push((key, offset, length));
  }

  // `cursor` now points to the start of the DATA section
  let data_start = cursor;
  let result = Map::new();

  for (key, offset, length) in &entries {
    let start = data_start + *offset as usize;
    let end = start + *length as usize;

    if end > data.len() {
      return Err(js_sys::Error::new(&format!(
        "Asset data out of bounds: {key}"
      )));
    }

    let js_key = JsValue::from_str(key);
    let js_value = Uint8Array::from(&data[start..end]);
    result.set(&js_key, &js_value);
  }

  Ok(result)
}
