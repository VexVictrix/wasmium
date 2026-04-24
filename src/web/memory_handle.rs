use js_sys::{BigInt, Function, Uint8Array, WebAssembly};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

type Shared<T> = std::rc::Rc<std::cell::OnceCell<T>>;

#[derive(Clone)]
pub struct MemoryHandle {
	memory: Shared<WebAssembly::Memory>,
	alloc: Shared<Function>,
	free: Shared<Function>,
} // end struct MemoryHandle

impl MemoryHandle {

	/// Create a new MemoryHandle with uninitialized memory and alloc/free functions
	pub fn new() -> Self { Self {
		memory: std::rc::Rc::new(std::cell::OnceCell::new()),
		alloc: std::rc::Rc::new(std::cell::OnceCell::new()),
		free: std::rc::Rc::new(std::cell::OnceCell::new()),
	} } // end fn new

	/// Setters for the WASM memory and alloc/free functions
	pub fn set_memory(&self, memory: WebAssembly::Memory) { let _ = self.memory.set(memory); }
	pub fn set_alloc(&self, alloc: Function) { let _ = self.alloc.set(alloc); }
	pub fn set_free(&self, free: Function) { let _ = self.free.set(free); }
	
	/// Getter for the WASM memory
	pub fn get_memory(&self) -> Result<WebAssembly::Memory, JsValue> {
		self.memory.get().cloned().ok_or_else(|| JsValue::from_str("WASM memory is not initialized"))
	} // end fn get_memory

	/// Call the allocation function to allocate memory in the WASM module, returning the pointer to the allocated memory
	pub fn call_alloc(&self, size: u64) -> Result<u64, JsValue> {
		// First try to use the provided alloc function if available
		if let Some(alloc) = self.alloc.get().cloned() {
			let result = alloc.call1(&JsValue::NULL, &JsValue::from(BigInt::from(size)))?;
			return super::js_to_u64(&JsValue::from(result));
		} // end primary allocation method
		Err(JsValue::from_str("No allocation function available"))
	} // end fn call_alloc

	/// Call the free function to deallocate memory in the WASM module, given a pointer and size
	pub fn call_free(&self, ptr: u64, size: u64) -> Result<(), JsValue> {
		if let Some(free) = self.free.get().cloned() {
			free.call2(&JsValue::NULL, &JsValue::from(BigInt::from(ptr)), &JsValue::from(BigInt::from(size)))?;
			return Ok(());
		} // end primary free method
		Err(JsValue::from_str("No free function available"))
	} // end fn call_free
	
	/// Read a slice of bytes from WASM memory at the given pointer and length, returning it as a Vec<u8>
	pub fn read_bytes(&self, ptr: u64) -> Result<Vec<u8>, JsValue> {
		let memory = self.get_memory()?;
		let buffer = Uint8Array::new(&memory.buffer());
		let byte_len = buffer.length() as u64;

		let length_end = ptr
			.checked_add(8)
			.ok_or_else(|| JsValue::from_str("Pointer overflow while reading length"))?;
		if length_end > byte_len {
			return Err(JsValue::from_str("Pointer out of bounds while reading length"));
		}

		let ptr_u32 = u32::try_from(ptr).map_err(|_| JsValue::from_str("Pointer does not fit in wasm32 address space"))?;
		let length_end_u32 = u32::try_from(length_end).map_err(|_| JsValue::from_str("Length pointer does not fit in wasm32 address space"))?;
		let length_bytes = buffer.subarray(ptr_u32, length_end_u32).to_vec();
		let len = u64::from_le_bytes(length_bytes.try_into().map_err(|_| JsValue::from_str("Failed to decode length header"))?);

		let data_start = length_end;
		let data_end = data_start
			.checked_add(len)
			.ok_or_else(|| JsValue::from_str("Pointer overflow while reading data"))?;
		if data_end > byte_len {
			return Err(JsValue::from_str("Pointer out of bounds while reading data"));
		}

		let data_start_u32 = u32::try_from(data_start).map_err(|_| JsValue::from_str("Data start does not fit in wasm32 address space"))?;
		let data_end_u32 = u32::try_from(data_end).map_err(|_| JsValue::from_str("Data end does not fit in wasm32 address space"))?;
		Ok(buffer.subarray(data_start_u32, data_end_u32).to_vec())
	} // end fn read_bytes
	
	/// Write a slice of bytes into WASM memory at the given pointer, ensuring it does not exceed memory bounds
	pub fn write_bytes(&self, ptr: u64, data: &[u8]) -> Result<(), JsValue> {
		let memory = self.get_memory()?;
		let buffer = Uint8Array::new(&memory.buffer());
		let end = ptr + (data.len() as u64);
		buffer.subarray(ptr as u32, end as u32).copy_from(data);
		Ok(())
	} // end fn write_bytes

	pub fn alloc_and_write(&self, data: &[u8]) -> Result<u64, JsValue> {
		let ptr = self.call_alloc(data.len() as u64 + 8)?;
		self.write_bytes(ptr, &data.len().to_le_bytes())?;
		self.write_bytes(ptr+8, data)?;
		Ok(ptr)
	} // end fn alloc_and_write

} // end impl MemoryHandle