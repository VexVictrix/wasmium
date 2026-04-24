mod host_function; pub use host_function::WebHostFunction;
mod wasm_module; pub use wasm_module::WebWasmiumModule;
mod memory_handle; pub use memory_handle::MemoryHandle;

use wasm_bindgen::prelude::*;
use js_sys::{BigInt, Number};
use web_sys::console;
fn js_to_u64(value: &JsValue) -> Result<u64, JsValue> {
	let bigint = value.clone().dyn_into::<BigInt>().map_err(|_| JsValue::from_str("Value is not a BigInt"))?;
	u64::try_from(bigint).map_err(|_| JsValue::from_str("Failed to convert BigInt to u64"))
} // end fn js_to_u64