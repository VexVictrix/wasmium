use js_sys::Function;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use super::MemoryHandle;

pub trait WebHostFunction {
	fn into_import(self, memory: MemoryHandle) -> (String, Function);
} // end trait WebHostFunction

impl WebHostFunction for crate::HostFunction {
	fn into_import(self, memory: MemoryHandle) -> (String, Function) {
		let callback = self.function;
		let import_name = self.name.clone();
		let closure = Closure::wrap(Box::new(move |ptr: u64| -> u64 {
			let input_bytes = memory.read_bytes(ptr).expect(&format!("Failed to read input bytes for import '{}'", import_name));
			let result_bytes = (callback)(input_bytes.as_slice());
			let alloc_ptr = memory.alloc_and_write(&result_bytes).expect(&format!("Failed to allocate memory for result of '{}'", import_name));
			return alloc_ptr;
		}) as Box<dyn FnMut(u64) -> u64>);
		let function = closure.as_ref()
			.unchecked_ref::<Function>()
			.clone();
		closure.forget();
		(self.name.clone(), function)
	} // end fn into_import
} // end impl WebHostFunction for HostFunction