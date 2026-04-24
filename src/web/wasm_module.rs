use js_sys::{Function, Object, Reflect, Uint8Array, WebAssembly};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use js_sys::BigInt;
use super::MemoryHandle;
use crate::HostFunction;
use super::host_function::WebHostFunction;
use crate::sys_functions;

pub struct WebWasmiumModule {
	pub instance: WebAssembly::Instance,
	pub memory: MemoryHandle,
	pub functions: std::collections::HashMap<String, Function>,
} // end struct WebWasmiumModule

impl WebWasmiumModule {

	pub fn new(bytes: &[u8], functions: Vec<HostFunction>) -> Result<Self, JsValue> {

		let memory_handle = MemoryHandle::new();

		// Create the sys module with built-in system functions that can be called from WASM, such as logging; these are separate from the user-provided host functions in the env module
		let sys_module = Object::new();
		let log = HostFunction::new("log", sys_functions::log).into_import(memory_handle.clone());
		Reflect::set(&sys_module, &JsValue::from_str(&log.0), &log.1)?;

		// Create the main import object for the WASM module
		let env_module = Object::new();
		for func in functions {
			let (name, import_fn) = func.into_import(memory_handle.clone());
			Reflect::set(&env_module, &JsValue::from_str(&name), &import_fn)?;
		} // end loop over host functions to create imports
		
		// Combine the env and sys modules into a single imports object for instantiating the WASM module
		let imports = Object::new();
		Reflect::set(&imports, &JsValue::from_str("env"), &env_module)?;
		Reflect::set(&imports, &JsValue::from_str("wasmium_sys"), &sys_module)?;

		// Instantiate the WASM module from the provided bytes and imports, then extract the exported memory and alloc function to initialize the MemoryHandle
		let view = unsafe { Uint8Array::view(bytes) };
		let module = WebAssembly::Module::new(&view)?;
		let instance = WebAssembly::Instance::new(&module, &imports)?;
		let memory_js = Reflect::get(&instance.exports(), &JsValue::from_str("memory"))?
			.dyn_into::<WebAssembly::Memory>()
			.map_err(|_| JsValue::from_str("Exported memory is not WebAssembly.Memory"))?;
		memory_handle.set_memory(memory_js);
		let alloc_fn = Reflect::get(&instance.exports(), &JsValue::from_str("wasmium_alloc"))?
			.dyn_into::<Function>()
			.map_err(|_| JsValue::from_str("Exported alloc is not a function"))?;
		memory_handle.set_alloc(alloc_fn);
		let free_fn = Reflect::get(&instance.exports(), &JsValue::from_str("wasmium_free"))?
			.dyn_into::<Function>()
			.map_err(|_| JsValue::from_str("Exported free is not a function"))?;
		memory_handle.set_free(free_fn);
		let mut module = Self { instance, memory: memory_handle, functions: std::collections::HashMap::new() };

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
		} // end loop

		// Call the __sys_init function to perform Wasmium initialization
		module.call::<(), ()>("__sys_init", ())?;


		Ok(module)
	
	} // end fn new

	pub fn call<T: serde::Serialize, O: serde::de::DeserializeOwned>(&self, func_name: &str, input: T) -> Result<O, JsValue> {
		// 1. Serialize the input using rmp_serde
		let bytes = rmp_serde::to_vec(&input).map_err(|e| JsValue::from_str(&format!("Failed to serialize input: {}", e)))?;
		// 2. Write the serialized input bytes into the WASM module's memory using the MemoryHandle
		let alloc_ptr = self.memory.alloc_and_write(&bytes)?;
		// 3. Call the specified exported function of the WASM module, passing the pointer to the input data in WASM memory
		let func = self.functions.get(func_name).ok_or_else(|| JsValue::from_str(&format!("Function '{}' not found in WASM module exports", func_name)))?;
		let result = func.call1(&JsValue::NULL, &JsValue::from(BigInt::from(alloc_ptr)))?;
		let result = super::js_to_u64(&result).map_err(|_| JsValue::from_str("Function did not return a valid pointer"))?;
		// 4. Read the output bytes from WASM memory at the pointer returned by the function call
		let bytes = self.memory.read_bytes(result)?;
		// 5. Deserialize the output bytes into the expected output type
		let output = rmp_serde::from_slice(&bytes).map_err(|e| JsValue::from_str(&format!("Failed to deserialize output: {}", e)))?;
		Ok(output)
	} // end fn call

} // end impl WebWasmiumModule
