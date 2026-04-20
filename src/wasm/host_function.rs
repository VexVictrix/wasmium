use js_sys::Function;
use wasm_bindgen::prelude::*;
use crate::MemoryHandle;

/// Represents a host function that can be imported by a WASM module, allowing Rust closures to be called from WASM with automatic serialization and deserialization of input and output data
pub struct HostFunction {
	name: String,
	callback: Box<dyn Fn(&[u8]) -> Vec<u8>>,
} // end struct HostFunction

impl HostFunction {
	
	// Create a new HostFunction with a raw byte callback, allowing for maximum flexibility in how the function processes input and produces output
	pub fn new_bytes(name: &str, callback: impl Fn(&[u8]) -> Vec<u8> + 'static) -> Self {
		Self { name: name.to_string(), callback: Box::new(callback) }
	} // end fn new_bytes

	// Create a new HostFunction with typed input and output, automatically handling serialization and deserialization using rmp_serde
	pub fn new<I, O, F>(name: &str, func: F) -> Self
	where
		I: serde::de::DeserializeOwned + 'static,
		O: serde::Serialize + 'static,
		F: Fn(I) -> O + 'static,
	{
		let name = name.to_string();
		Self::new_bytes(&name.clone(), move |input| -> Vec<u8> {
			let input: I = rmp_serde::from_slice(input).expect(&format!("Failed to deserialize input for {}", name));
			let output: O = func(input);
			rmp_serde::to_vec(&output).expect(&format!("Failed to serialize output for {}", name))
		})
	} // end fn new

	// Convert the HostFunction into a JavaScript function that can be imported by the WASM module, using the provided MemoryHandle to read input and write output
	pub fn into_import(self, memory: MemoryHandle) -> (String, Function) {
		let callback = self.callback;
		let import_name = self.name.clone();
		let closure = Closure::wrap(Box::new(move |ptrlen: u64| -> u64 {
			let ptr = (ptrlen >> 32) as u32;
			let len = (ptrlen & 0xffffffff) as u32;
			match memory.read_bytes(ptr, len) {
				Ok(bytes) => {
					let result_bytes = (callback)(bytes.as_slice());
					let alloc_ptr = memory.call_alloc(result_bytes.len() as u32).expect("Failed to call WASM alloc for callback result");
					memory.write_bytes(alloc_ptr, result_bytes.as_slice()).expect("Failed to write callback result to WASM memory");
					((alloc_ptr as u64) << 32) | (result_bytes.len() as u64)
				}
				Err(error) => { panic!("Import '{}' failed to read memory for ptr={}, len={}: {:?}", import_name, ptr, len, error); }
			}
		}) as Box<dyn FnMut(u64) -> u64>)
		.into_js_value();
		(self.name, closure.into())
	} // end fn into_import
	
} // end impl HostFunction