// The window is the product, so on Windows the console that would otherwise open behind
// it is suppressed in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
	skidbladnir_lib::run();
}
