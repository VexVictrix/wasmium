use crate::WasmiumModule;

// Wasmium: Web
#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub mod web_wasmium_module {
	use wasm_bindgen::JsValue;
	use js_sys::{Function, Object, Reflect, Uint8Array, WebAssembly};
	use wasm_bindgen::prelude::*;
	use crate::web::MemoryHandle;
	use super::HostFunction;
	use super::WasmiumError;

	pub struct WasmiumModule {
		module: crate::web::WebWasmiumModule
	} // end struct WebWasmiumModule

	impl WasmiumModule {
		pub fn new(wasm_bytes: &[u8], host_functions: Vec<HostFunction>) -> Result<Self, WasmiumError> {
			let module = crate::web::WebWasmiumModule::new(wasm_bytes, host_functions)?;
			return Ok(Self { module });
		} // end fn new

		pub fn call<T: serde::Serialize, O: serde::de::DeserializeOwned>(&mut self, name: &str, input: T) -> Result<O, WasmiumError> {
			return Ok(self.module.call::<T, O>(name, input)?);
		} // end fn call_function
	} // end impl WasmiumModule

	impl From<JsValue> for WasmiumError {
		fn from(value: JsValue) -> Self {
			WasmiumError::JsError(value)
		} // end fn from
	} // end impl From<JsValue> for WasmiumError

} // end mod web_wasmium_module

// Wasmium: Native
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
pub mod native_wasmium_module {
	use super::HostFunction;
	pub struct WasmiumModule {
		module: crate::native::NativeWasmiumModule,
	} // end struct NativeWasmiumModule
	impl WasmiumModule {
		pub fn new(wasm_bytes: &[u8], host_functions: Vec<HostFunction>) -> Result<Self, Box<dyn std::error::Error>> {
			let module = crate::native::NativeWasmiumModule::new(wasm_bytes, host_functions)?;
			return Ok(Self { module });
		} // end fn new

		pub fn call<T: serde::Serialize, O: serde::de::DeserializeOwned>(&mut self, name: &str, input: T) -> Result<O, Box<dyn std::error::Error>> {
			return self.module.call::<T, O>(name, input);
		} // end fn call_function
	} // end impl WasmiumModule
} // end mod native_wasmium_module

#[derive(Debug)]
pub enum WasmiumError {
	#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
	JsError(wasm_bindgen::JsValue),
}

pub struct HostFunction {
	pub name: String,
	pub function: Box<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>,
} // end struct HostFunction

impl HostFunction {
	pub fn new<I, O, F>(name: &str, func: F) -> Self
		where
			I: serde::de::DeserializeOwned + 'static,
			O: serde::Serialize + 'static,
			F: Fn(I) -> O + Send + Sync + 'static,
	{

		let name = name.to_string();
		return Self { name: name.clone(), function: Box::new(move |input| -> Vec<u8> {
			let input: I = rmp_serde::from_slice(input).expect(&format!("Failed to deserialize input for {}", name));
			let output: O = func(input);
			return rmp_serde::to_vec(&output).expect(&format!("Failed to serialize output for {}", name));
		})};

	} // end fn new
} // end impl HostFunction
