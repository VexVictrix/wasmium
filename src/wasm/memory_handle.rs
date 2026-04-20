use js_sys::{Function, Uint8Array, WebAssembly};
use wasm_bindgen::prelude::*;

type Shared<T> = std::rc::Rc<std::cell::OnceCell<T>>;

#[derive(Clone)]
pub struct MemoryHandle {
	memory: Shared<WebAssembly::Memory>,
	alloc: Shared<Function>,
	free: Shared<Function>,
} // end struct MemoryHandle

impl MemoryHandle {

	/// Create a new MemoryHandle with uninitialized memory and alloc function
	pub fn new() -> Self { Self {
		memory: std::rc::Rc::new(std::cell::OnceCell::new()),
		alloc: std::rc::Rc::new(std::cell::OnceCell::new()),
		free: std::rc::Rc::new(std::cell::OnceCell::new()),
	} } // end fn new

	/// Setters for the WASM memory and allocation function, allowing them to be initialized after the MemoryHandle is created
	pub fn set_memory(&self, memory: WebAssembly::Memory) { let _ = self.memory.set(memory); }
	pub fn set_alloc(&self, alloc: Function) { let _ = self.alloc.set(alloc); }
	pub fn set_free(&self, free: Function) { let _ = self.free.set(free); }
	
	/// Getter for the WASM memory, returning an error if it has not been initialized
	pub fn get_memory(&self) -> Result<WebAssembly::Memory, JsValue> {
		self.memory.get().cloned().ok_or_else(|| JsValue::from_str("WASM memory is not initialized"))
	} // end fn get_memory

	/// Call the allocation function to allocate memory in the WASM module, returning the pointer to the allocated memory
	pub fn call_alloc(&self, size: u32) -> Result<u32, JsValue> {
		// First try to use the provided alloc function if available
		if let Some(alloc) = self.alloc.get().cloned() {
			let result = alloc.call1(&JsValue::NULL, &JsValue::from(size))?;
			return result.as_f64().map(|f| f as u32).ok_or_else(|| JsValue::from_str("Return value is not a number"));
		} // end primary allocation method
		Err(JsValue::from_str("No allocation function available"))
	} // end fn call_alloc

	/// Call the free function to deallocate memory in the WASM module, given a pointer and size
	pub fn call_free(&self, ptr: u32, size: u32) -> Result<(), JsValue> {
		if let Some(free) = self.free.get().cloned() {
			free.call2(&JsValue::NULL, &JsValue::from(ptr), &JsValue::from(size))?;
			return Ok(());
		} // end primary free method
		Err(JsValue::from_str("No free function available"))
	} // end fn call_free
	
	/// Read a slice of bytes from WASM memory at the given pointer and length, returning it as a Vec<u8>
	pub fn read_bytes(&self, ptr: u32, length: u32) -> Result<Vec<u8>, JsValue> {
		let memory = self.get_memory()?;
		let buffer = Uint8Array::new(&memory.buffer());
		// Check for pointer overflow and out-of-bounds access
		let end = ptr
			.checked_add(length)
			.ok_or_else(|| JsValue::from_str("Pointer overflow while reading memory"))?;
		if end > buffer.length() { return Err(JsValue::from_str("Out-of-bounds memory read")); }
		// Copy requested bytes directly into a Rust Vec without creating an intermediate JS typed array
		let mut out = vec![0u8; length as usize];
		buffer.subarray(ptr, end).copy_to(&mut out);
		Ok(out)
	} // end fn read_bytes
	
	/// Write a slice of bytes into WASM memory at the given pointer, ensuring it does not exceed memory bounds
	pub fn write_bytes(&self, ptr: u32, data: &[u8]) -> Result<(), JsValue> {
		let memory = self.get_memory()?;
		let buffer = Uint8Array::new(&memory.buffer());
		// Check for pointer overflow and out-of-bounds access
		let end = ptr
			.checked_add(data.len() as u32)
			.ok_or_else(|| JsValue::from_str("Pointer overflow while writing memory"))?;
		if end > buffer.length() { return Err(JsValue::from_str("Out-of-bounds memory write")); }
		// Write directly from Rust slice to the target memory range without allocating a temporary JS typed array
		buffer.subarray(ptr, end).copy_from(data);
		Ok(())
	} // end fn write_bytes

} // end impl MemoryHandle