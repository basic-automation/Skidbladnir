//! Print the libwebp version this crate is linked against, as `major.minor.revision`.
//!
//! Exists so CI can build a **version-matched** reference `cwebp` from source. Byte parity
//! is only meaningful between equal libwebp versions, and the version apt or brew ships is
//! not ours to pin — so the reference is built from the same upstream tag as the libwebp
//! that `libwebp-sys` vendors, which this reports. If libwebp-sys bumps its vendored
//! version, CI follows automatically rather than silently falling back to skipping.

fn main() {
	let (major, minor, revision) = skidbladnir_encode::encoder::linked_encoder_version();
	println!("{major}.{minor}.{revision}");
}
