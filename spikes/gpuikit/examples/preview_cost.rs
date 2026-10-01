//! What one settings change costs to preview, Tauri's way and the spike's way.
//!
//! Both sides run the same encoder with the same settings. The difference measured is
//! everything around it:
//!
//! - **Tauri** (`src-tauri/src/preview.rs`, still-image path): read and decode the source
//!   file again, encode, encode the original losslessly so the webview can show it, wrap
//!   both as base64 `data:` URLs, and serialize the result to JSON for IPC. The webview
//!   then decodes both WebPs, which this cannot measure.
//! - **Spike**: the source is decoded once at open, so per change it encodes, decodes the
//!   result with libwebp and swaps it to BGRA for the GPU.
//!
//! `cargo run --release --example preview_cost -- image.png [runs]`

use std::{path::Path, time::Instant};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use skidbladnir_encode::{EncodeJob, WebpSettings, encode_rgba, encode_source_with_progress, source};

fn main() {
	let mut args = std::env::args().skip(1);
	let input = args.next().expect("usage: preview_cost <image> [runs]");
	let runs: u32 = args.next().map_or(5, |runs| runs.parse().expect("runs is a number"));
	let input = Path::new(&input);
	let job = EncodeJob::default();

	// Warm the page cache and the allocator before timing either.
	let source = source::load(input).unwrap();
	let _ = encode_source_with_progress(&job, &source, &mut |_| true).unwrap();

	let mut tauri = Vec::new();
	let mut tauri_original = Vec::new();
	let mut payload = 0;
	for _ in 0..runs {
		let started = Instant::now();
		let bytes = std::fs::read(input).unwrap();
		let image = source::decode(input, bytes).unwrap();
		let encoded = encode_source_with_progress(&job, &image, &mut |_| true).unwrap();
		let original_started = Instant::now();
		let lossless = EncodeJob { webp: WebpSettings { lossless: true, exact: true, quality: 100.0, ..Default::default() }, ..Default::default() };
		let original = encode_rgba(&lossless, &image.as_rgba()).unwrap();
		tauri_original.push(original_started.elapsed());
		let json = serde_json::json!({
			"original": format!("data:image/webp;base64,{}", STANDARD.encode(&original)),
			"encoded": format!("data:image/webp;base64,{}", STANDARD.encode(&encoded)),
		})
		.to_string();
		payload = json.len();
		tauri.push(started.elapsed());
	}

	let mut spike = Vec::new();
	for _ in 0..runs {
		let started = Instant::now();
		let encoded = encode_source_with_progress(&job, &source, &mut |_| true).unwrap();
		let mut decoded = source::decode(Path::new("preview.webp"), encoded).unwrap();
		for pixel in decoded.pixels.as_chunks_mut::<4>().0 {
			pixel.swap(0, 2);
		}
		spike.push(started.elapsed());
	}

	let median = |times: &mut Vec<std::time::Duration>| {
		times.sort();
		times[times.len() / 2].as_millis()
	};
	println!("{}x{}, default WebP settings, median of {runs}", source.width, source.height);
	println!("tauri: {} ms per change ({} ms of it the lossless original), {:.1} MB of JSON over IPC", median(&mut tauri), median(&mut tauri_original), payload as f64 / 1e6);
	println!("spike: {} ms per change, 0 bytes over IPC", median(&mut spike));
}
