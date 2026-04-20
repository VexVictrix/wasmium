use js_sys::{BigInt, Function, Object, Reflect, Uint8Array, WebAssembly};
use wasm_bindgen::prelude::*;
use crate::MemoryHandle;
use crate::HostFunction;
use crate::sys_functions;

/// Represents an instantiated WebAssembly module, holding the instance
/// and a handle to its memory for managing interactions between host
/// functions and WASM functions
pub struct WasmModule {
	pub instance: WebAssembly::Instance,
	pub memory: MemoryHandle,
	pub functions: std::collections::HashMap<String, Function>,
} // end struct WasmModule

impl WasmModule {

	/// Create a new WasmModule by instantiating the provided WASM bytes
	/// with the given host functions, setting up the necessary imports
	/// and memory handling
	pub fn new(bytes: &[u8], functions: Vec<HostFunction>) -> Result<Self, JsValue> {
		
		let memory = MemoryHandle::new();

		// Create the sys module with built-in system functions that can be called from WASM, such as logging; these are separate from the user-provided host functions in the env module
		let sys_module = Object::new();
		let log = HostFunction::new("log", sys_functions::log).into_import(memory.clone());
		Reflect::set(&sys_module, &JsValue::from_str(&log.0), &log.1)?;

		// Create the main import object for the WASM module
		let env_module = Object::new();
		for func in functions {
			let (name, import_fn) = func.into_import(memory.clone());
			Reflect::set(&env_module, &JsValue::from_str(&name), &import_fn)?;
		} // end loop over host functions to create imports
		
		// Combine the env and sys modules into a single imports object for instantiating the WASM module
		let imports = Object::new();
		Reflect::set(&imports, &JsValue::from_str("env"), &env_module)?;
		Reflect::set(&imports, &JsValue::from_str("sys"), &sys_module)?;

		// Instantiate the WASM module from the provided bytes and imports, then extract the exported memory and alloc function to initialize the MemoryHandle
		let view = unsafe { Uint8Array::view(bytes) };
		let module = WebAssembly::Module::new(&view)?;
		let instance = WebAssembly::Instance::new(&module, &imports)?;
		let memory_js = Reflect::get(&instance.exports(), &JsValue::from_str("memory"))?
			.dyn_into::<WebAssembly::Memory>()
			.map_err(|_| JsValue::from_str("Exported memory is not WebAssembly.Memory"))?;
		memory.set_memory(memory_js);
		let alloc_fn = Reflect::get(&instance.exports(), &JsValue::from_str("alloc"))?
			.dyn_into::<Function>()
			.map_err(|_| JsValue::from_str("Exported alloc is not a function"))?;
		memory.set_alloc(alloc_fn);
		let free_fn = Reflect::get(&instance.exports(), &JsValue::from_str("free"))?
			.dyn_into::<Function>()
			.map_err(|_| JsValue::from_str("Exported free is not a function"))?;
		memory.set_free(free_fn);
		let mut module = Self { instance, memory, functions: std::collections::HashMap::new() };

		// Extract all exported functions from the WASM module and store
		// them in the WasmModule struct for easy access
		for export in Object::keys(&module.instance.exports()).iter() {
			let export_name = export.as_string().unwrap_or_default();
			let export_value = Reflect::get(&module.instance.exports(), &export)?;
			if let Ok(function) = export_value.dyn_into::<Function>() {
				if export_name != "alloc" && export_name != "free" {
					module.functions.insert(export_name, function);
				} // end skip alloc and free in exported functions
			}
		} // end loop to log exported functions for debugging

		module.call::<(), ()>("__sys_init", ())?;
		Ok(module)
	} // end fn new

	/// Call the exported alloc function of the WASM module to allocate memory
	pub fn call_alloc(&self, size: u32) -> Result<u32, JsValue> {
		self.memory.call_alloc(size)
	} // end fn call_alloc

	/// Call the exported free function of the WASM module to free memory
	/// at the given pointer and size
	pub fn call_free(&self, ptr: u32, size: u32) -> Result<(), JsValue> {
		self.memory.call_free(ptr, size)
	} // end fn call_free

	/// Call an exported WASM function that takes a pointer and length
	/// as input and returns a pointer and length as output, handling
	/// the memory management and byte conversion
	pub fn call_ptr(&self, func_name: &str, ptr: u32, len: u32) -> Result<Vec<u8>, JsValue> {
		
		// Get the exported function from the WASM module by name, ensuring it exists and is a function
		let func = self.functions
			.get(func_name)
			.ok_or_else(|| JsValue::from_str(&format!("Function '{}' not found", func_name)))?;
		
		// Call the function with the pointer and length as arguments, expecting a BigInt return value that encodes the result pointer and length
		let result = func.call2(&JsValue::NULL, &JsValue::from(ptr), &JsValue::from(len))?;
		
		// WebAssembly i64 exports come back to JS as BigInt; convert via js_sys::BigInt
		let bigint = result.dyn_into::<BigInt>().map_err(|_| JsValue::from_str("Return value is not a BigInt"))?;
		let integer = u64::try_from(bigint).map_err(|e| JsValue::from(e))?;
		
		// Extract the result pointer and length from the returned integer,
		// then read the bytes from WASM memory and free the allocated memory
		let ptr = (integer >> 32) as u32;
		let len = (integer & 0xffffffff) as u32;
		let bytes = self.memory.read_bytes(ptr, len)?;
		
		// Free the memory allocated by the WASM function
		self.call_free(ptr, len)?;
		Ok(bytes)
	
	} // end fn call_ptr

	/// Call an exported WASM function with a typed input and output, automatically
	/// handling serialization and deserialization using rmp_serde
	pub fn call<T: serde::Serialize, O: serde::de::DeserializeOwned>(&self, func_name: &str, input: T) -> Result<O, JsValue> {
		// 1. Serialize the input using rmp_serde
		let bytes = rmp_serde::to_vec(&input).map_err(|e| JsValue::from_str(&format!("Failed to serialize input: {}", e)))?;
		// 2. Allocate memory in the WASM module
		let alloc_ptr = self.call_alloc(bytes.len() as u32)?;
		// 3. Write the serialized bytes into WASM memory
		self.memory.write_bytes(alloc_ptr, &bytes)?;
		// 4. Call the WASM function with the pointer and length of the input data
		let result = self.call_ptr(func_name, alloc_ptr, bytes.len() as u32)?;
		// 5. Deserialize the output bytes into the expected output type
		let output = rmp_serde::from_slice(&result).map_err(|e| JsValue::from_str(&format!("Failed to deserialize output: {}", e)))?;
		Ok(output)
	} // end fn call

} // end impl WasmModule