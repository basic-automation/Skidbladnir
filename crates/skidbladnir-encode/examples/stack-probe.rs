//! How much stack each encoder needs: runs one encode on a thread with a given stack size.
//!
//! The windows run conversions on background threads whose stack they do not choose (gpui's
//! on Windows are the system thread pool's, the executable's default reserve), so this
//! measures what the heaviest settings of each encoder need. A stack too small aborts the
//! process ("has overflowed its stack"); `scripts/stack-probe.sh` bisects for the smallest
//! size each case survives.
//!
//!     cargo run --release --example stack-probe -- <case> <stack bytes> <scratch dir>
//!     cargo run --release --example stack-probe -- list

use std::{env, fs, path::Path, process::ExitCode};

use skidbladnir_encode::{
	encode_file, settings::{AvifSettings, EncodeJob, HeicSettings, JxlSettings, JxlTarget, OutputFormat, WebpSettings}
};

/// Each case: a name, the input it reads (`photo.png` or `animation.gif`) and the job.
fn cases() -> Vec<(&'static str, &'static str, EncodeJob)> {
	let job = |format| EncodeJob { format, ..EncodeJob::default() };
	vec![("webp", "photo.png", job(OutputFormat::Webp)), ("webp -m 6", "photo.png", EncodeJob { webp: WebpSettings { method: 6, ..WebpSettings::default() }, ..job(OutputFormat::Webp) }), ("webp -lossless -m 6", "photo.png", EncodeJob { webp: WebpSettings { lossless: true, method: 6, ..WebpSettings::default() }, ..job(OutputFormat::Webp) }), ("animated webp", "animation.gif", job(OutputFormat::Webp)), ("animated webp -m 6", "animation.gif", EncodeJob { webp: WebpSettings { method: 6, ..WebpSettings::default() }, ..job(OutputFormat::Webp) }), ("avif", "photo.png", job(OutputFormat::Avif)), ("avif -s 0", "photo.png", EncodeJob { avif: AvifSettings { speed: Some(0), ..AvifSettings::default() }, ..job(OutputFormat::Avif) }), ("avif -l -s 0", "photo.png", EncodeJob { avif: AvifSettings { lossless: true, speed: Some(0), ..AvifSettings::default() }, ..job(OutputFormat::Avif) }), ("jxl", "photo.png", job(OutputFormat::Jxl)), ("jxl -e 9", "photo.png", EncodeJob { jxl: JxlSettings { effort: 9, ..JxlSettings::default() }, ..job(OutputFormat::Jxl) }), ("jxl -d 0 -e 9", "photo.png", EncodeJob { jxl: JxlSettings { target: JxlTarget::Distance(0.0), effort: 9, ..JxlSettings::default() }, ..job(OutputFormat::Jxl) }), ("heic", "photo.png", job(OutputFormat::Heic)), ("heic -q 100", "photo.png", EncodeJob { heic: HeicSettings { quality: 100, ..HeicSettings::default() }, ..job(OutputFormat::Heic) })]
}

/// A 1024x768 photo-like RGBA image: smooth gradients with noise and soft alpha.
fn write_photo(path: &Path) {
	let (width, height) = (1024_u32, 768_u32);
	let mut seed = 0x2545_f491_u32;
	let image = image::RgbaImage::from_fn(width, height, |x, y| {
		seed ^= seed << 13;
		seed ^= seed >> 17;
		seed ^= seed << 5;
		let noise = (seed % 24) as u8;
		let channel = |value: u32| u8::try_from(value % 232).expect("under 232") + noise;
		image::Rgba([channel(x / 4), channel(y / 3), channel((x + y) / 7), u8::try_from(128 + (x * 127 / width)).expect("under 256")])
	});
	image.save(path).expect("write the PNG");
}

/// A 320x240 GIF of eight frames, each a shifted pattern.
fn write_animation(path: &Path) {
	let (width, height) = (320_u16, 240_u16);
	let file = fs::File::create(path).expect("create the GIF");
	let mut encoder = gif::Encoder::new(file, width, height, &[]).expect("start the GIF");
	encoder.set_repeat(gif::Repeat::Infinite).expect("loop the GIF");
	for frame in 0..8_u32 {
		let mut pixels: Vec<u8> = (0..u32::from(width) * u32::from(height))
			.flat_map(|i| {
				let (x, y) = (i % u32::from(width), i / u32::from(width));
				let value = |shift: u32| u8::try_from((x + y * 2 + frame * 9 + shift) % 256).expect("under 256");
				[value(0), value(85), value(170), 255]
			})
			.collect();
		let mut gif_frame = gif::Frame::from_rgba_speed(width, height, &mut pixels, 10);
		gif_frame.delay = 8;
		encoder.write_frame(&gif_frame).expect("write a GIF frame");
	}
}

fn main() -> ExitCode {
	let args: Vec<String> = env::args().skip(1).collect();
	if args.first().map(String::as_str) == Some("list") {
		for (name, ..) in cases() {
			println!("{name}");
		}
		return ExitCode::SUCCESS;
	}
	let [case, stack, dir] = args.as_slice() else {
		eprintln!("usage: stack-probe <case> <stack bytes> <scratch dir> | stack-probe list");
		return ExitCode::from(2);
	};
	let Some((_, input, job)) = cases().into_iter().find(|(name, ..)| name == case) else {
		eprintln!("no case {case:?}; `stack-probe list` lists them");
		return ExitCode::from(2);
	};
	let stack: usize = stack.parse().expect("the stack size, in bytes");
	let dir = Path::new(dir);
	fs::create_dir_all(dir).expect("create the scratch directory");
	let (photo, animation) = (dir.join("photo.png"), dir.join("animation.gif"));
	if !photo.exists() {
		write_photo(&photo);
	}
	if !animation.exists() {
		write_animation(&animation);
	}
	let input = dir.join(input);
	let output = dir.join(format!("out-{}", std::process::id()));
	let result = std::thread::Builder::new().stack_size(stack).spawn(move || encode_file(&job, &input, &output).map(|_| fs::remove_file(&output))).expect("spawn the encode thread").join();
	match result {
		Ok(Ok(_)) => ExitCode::SUCCESS,
		Ok(Err(error)) => {
			eprintln!("{case}: {error}");
			ExitCode::from(3)
		}
		Err(_) => ExitCode::from(4),
	}
}
