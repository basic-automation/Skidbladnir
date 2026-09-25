//! The encode core for Skidbladnir.
//!
//! Skidbladnir exists because it exposes `cwebp`'s *whole* control surface rather than
//! one quality slider, so this crate's job is to model that surface exactly and encode
//! with it. It is deliberately free of any Tauri dependency: everything here is
//! testable from `cargo test` with no window, no IPC and no frontend.
//!
//! The authority for what the surface *is* is the Electron app still at the repo root
//! (`index.html` builds the settings, `main.js` assembles the `cwebp` command line).
//! Every control it exposes is represented in [`EncodeSettings`]; see ROADMAP.md Phase 2.

pub mod cwebp;
pub mod encoder;
pub mod settings;
pub mod source;

pub use cwebp::cwebp_args;
pub use encoder::{EncodeError, RgbaImage, encode_rgba, encode_rgba_with_progress};
pub use settings::{AlphaFiltering, EncodeSettings, FilterType, Mode, Preset, Resize, TargetMetric, ValidationError};
pub use source::{Conversion, ConvertError, PathInspection, SourceError, SourceFormat, SourceImage, encode_file, encode_file_with_progress, inspect_paths, load};
