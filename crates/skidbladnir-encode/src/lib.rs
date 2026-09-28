//! The encode core for Skidbladnir.
//!
//! Skidbladnir exists because it exposes `cwebp`'s *whole* control surface rather than
//! one quality slider, so this crate's job is to model that surface exactly and encode
//! with it. It is deliberately free of any Tauri dependency: everything here is
//! testable from `cargo test` with no window, no IPC and no frontend.
//!
//! The authority for what the surface *is* was the Electron app this replaced
//! (`index.html` built the settings, `main.js` assembled the `cwebp` command line). It was
//! retired in Phase 6; its last source is at the `electron-final` tag.
//! Every control it exposes is represented in [`EncodeJob`]; see ROADMAP.md Phase 2.

pub mod animation;
pub mod avif;
pub mod avif_transform;
pub mod cwebp;
pub mod encoder;
pub mod gif_input;
pub mod inspect;
pub mod settings;
pub mod source;

pub use cwebp::cwebp_args;
pub use encoder::{EncodeError, RgbaImage, encode_rgba, encode_rgba_with_progress, rescale_rgba};
pub use inspect::{WebpCompression, WebpInfo, inspect_webp};
pub use settings::{AlphaFiltering, AvifAlphaMode, AvifBitDepth, AvifColorModel, AvifSettings, EncodeJob, FilterType, Mode, OutputFormat, Preset, Resize, TargetMetric, ValidationError, WebpSettings};
pub use source::{Conversion, ConvertError, FoundImage, PathInspection, SourceError, SourceFormat, SourceImage, encode_file, encode_file_with_progress, inspect_paths, load, mirrored_output_path, scan_directory};
