
// cfg if wasm32 or wasm64
#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub fn log(msg: String) {
	use web_sys::console;
	console::log_1(&msg.into());
} // end fn log

// cfg if not wasm32 or wasm64
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
pub fn log(msg: String) {
	println!("{}", msg);
} // end fn log

