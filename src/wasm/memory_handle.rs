use js_sys::{BigInt, Function, Object, Reflect, Uint8Array, WebAssembly};
use wasm_bindgen::prelude::*;

type Shared<T> = std::rc::Rc<std::cell::RefCell<Option<T>>>;

#[derive(Clone)]
pub struct MemoryHandle {
	memory: Shared<WebAssembly::Memory>,
	alloc: Shared<Function>,
} // end struct MemoryHandle

impl MemoryHandle {

	// Create a new MemoryHandle with uninitialized memory and alloc function
	pub fn new() -> Self { Self {
		memory: std::rc::Rc::new(std::cell::RefCell::new(None)),
		alloc: std::rc::Rc::new(std::cell::RefCell::new(None)),
	} } // end fn new

	// Setters for the WASM memory and allocation function, allowing them to be initialized after the MemoryHandle is created
	pub fn set_memory(&self, memory: WebAssembly::Memory) { *self.memory.borrow_mut() = Some(memory); }
	pub fn set_alloc(&self, alloc: Function) { *self.alloc.borrow_mut() = Some(alloc); }
	
	// Getter for the WASM memory, returning an error if it has not been initialized
	pub fn get_memory(&self) -> Result<WebAssembly::Memory, JsValue> {
		self.memory.borrow().clone().ok_or_else(|| JsValue::from_str("WASM memory is not initialized"))
	} // end fn get_memory

	// Call the allocation function to allocate memory in the WASM module, returning the pointer to the allocated memory
	pub fn call_alloc(&self, size: u32) -> Result<u32, JsValue> {
		// First try to use the provided alloc function if available
		if let Some(alloc) = self.alloc.borrow().clone() {
			let result = alloc.call1(&JsValue::NULL, &JsValue::from(size))?;
			return result.as_f64().map(|f| f as u32).ok_or_else(|| JsValue::from_str("Return value is not a number"));
		} // end primary allocation method
		Err(JsValue::from_str("No allocation function available"))
	} // end fn call_alloc
	
	// Read a slice of bytes from WASM memory at the given pointer and length, returning it as a Vec<u8>
	pub fn read_bytes(&self, ptr: u32, length: u32) -> Result<Vec<u8>, JsValue> {
		let memory = self.get_memory()?;
		let buffer = Uint8Array::new(&memory.buffer());
		// Check for pointer overflow and out-of-bounds access
		let end = ptr
			.checked_add(length)
			.ok_or_else(|| JsValue::from_str("Pointer overflow while reading memory"))?;
		if end > buffer.length() { return Err(JsValue::from_str("Out-of-bounds memory read")); }
		// Return the requested slice of memory as a Vec<u8>
		Ok(buffer.slice(ptr, end).to_vec())
	} // end fn read_bytes
	
	// Write a slice of bytes into WASM memory at the given pointer, ensuring it does not exceed memory bounds
	pub fn write_bytes(&self, ptr: u32, data: &[u8]) -> Result<(), JsValue> {
		let memory = self.get_memory()?;
		let buffer = Uint8Array::new(&memory.buffer());
		// Check for pointer overflow and out-of-bounds access
		let end = ptr
			.checked_add(data.len() as u32)
			.ok_or_else(|| JsValue::from_str("Pointer overflow while writing memory"))?;
		if end > buffer.length() { return Err(JsValue::from_str("Out-of-bounds memory write")); }
		// Write the provided data into the WASM memory at the specified pointer
		buffer.set(&Uint8Array::from(data), ptr);
		Ok(())
	} // end fn write_bytes
	// pub fn into_import(self, function: HostFunction) -> (String, Function) {
	// 	let callback = function.callback;
	// 	let import_name = function.name.clone();
	// 	let closure = Closure::wrap(Box::new(move |ptrlen: u64| -> u64 {
	// 		let ptr = (ptrlen >> 32) as u32;
	// 		let len = (ptrlen & 0xffffffff) as u32;
	// 		match self.read_bytes(ptr, len) {
	// 			Ok(bytes) => {
	// 				let result_bytes = (callback)(bytes.as_slice());
	// 				let alloc_ptr = self.call_alloc(result_bytes.len() as u32).expect("Failed to call WASM alloc for callback result");
	// 				self.write_bytes(alloc_ptr, result_bytes.as_slice()).expect("Failed to write callback result to WASM memory");
	// 				((alloc_ptr as u64) << 32) | (result_bytes.len() as u64)
	// 			}
	// 			Err(error) => { panic!("Import '{}' failed to read memory for ptr={}, len={}: {:?}", import_name, ptr, len, error); }
	// 		}
	// 	}) as Box<dyn FnMut(u64) -> u64>)
	// 	.into_js_value();
	// 	(function.name, closure.into())
	// } // end fn into_import
}