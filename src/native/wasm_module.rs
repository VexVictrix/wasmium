use serde::{Serialize, de::DeserializeOwned};
pub use crate::HostFunction;
use crate::native;

/// Represents an instantiated WebAssembly module
pub struct WasmModule {
	module: wasmtime::Module,
	instance: wasmtime::Instance,
	store: wasmtime::Store<()>,
	memory: wasmtime::Memory,
	alloc: std::sync::Arc<std::sync::OnceLock<wasmtime::TypedFunc<u32, u32>>>,
	free: std::sync::Arc<std::sync::OnceLock<wasmtime::TypedFunc<(u32, u32), ()>>>,
	functions: std::collections::HashMap<String, wasmtime::TypedFunc<(u32, u32), u64>>,
} // end struct WasmModule

/// Generic wrapper for HostFunction that handles serialization and deserialization
/// of input and output data, allowing Rust closures to be easily exposed as WASM
/// imports without needing to manually manage memory or data formats
pub fn func_wrap<I: DeserializeOwned, O: Serialize>(
	linker: &mut wasmtime::Linker<()>,
	alloc: std::sync::Arc<std::sync::OnceLock<wasmtime::TypedFunc<u32, u32>>>,
	module_name: &str,
	func_name: &str,
	func: impl Fn(I) -> O + Send + Sync + 'static
) -> Result<(), Box<dyn std::error::Error>> {

	// Clone the module and function name strings to ensure they are owned and can be moved into the closure
	let module_name = module_name.to_string();
	let out_module_name = module_name.clone();
	let func_name = func_name.to_string();
	let out_func_name = func_name.clone();
	
	// Define the host function that will be called by the WASM module,
	// which reads input data from WASM memory, deserializes it, calls
	// the provided Rust closure, serializes the output, allocates memory
	// for the output in WASM, writes the output back to WASM memory, and
	// returns a pointer and length to the output data
	let func = move |mut caller: wasmtime::Caller<'_, ()>, ptrlen: u64| -> u64 {
		
		// Extract the pointer and length from the combined u64 argument
		let ptr = (ptrlen >> 32) as u32;
		let len = (ptrlen & 0xffffffff) as u32;
		
		// Read the input data from WASM memory into a Rust byte vector
		let mut memory_read = vec![0; len as usize];
		let memory = caller
			.get_export("memory")
			.and_then(|export| export.into_memory())
			.expect(&format!("Host function '{}.{}' could not access exported memory", module_name, func_name));
		let _ = memory.read(&mut caller, ptr as usize, &mut memory_read)
			.expect(&format!("Host function '{}.{}' failed to read memory for ptr={}, len={}", module_name, func_name, ptr, len));
		
		// Deserialize the input data using rmp_serde, call the provided Rust closure with
		// the deserialized input, serialize the output using rmp_serde, allocate memory
		// for the output in WASM, write the output back to WASM memory, and return a pointer
		// and length to the output data
		let input_data = rmp_serde::from_slice(&memory_read).expect(&format!("Host function '{}.{}' failed to deserialize input", module_name, func_name));
		let output_bytes = rmp_serde::to_vec(&func(input_data)).expect(&format!("Host function '{}.{}' failed to serialize output", module_name, func_name));
		let alloc_func = alloc.get().expect(&format!("Host function '{}.{}' called before alloc function was set", module_name, func_name));
		let output_ptr = alloc_func.call(&mut caller, output_bytes.len() as u32)
			.expect(&format!("Host function '{}.{}' failed to call alloc for output size {}", module_name, func_name, output_bytes.len()));
		memory.write(&mut caller, output_ptr as usize, &output_bytes)
			.expect(&format!("Host function '{}.{}' failed to write output to memory at ptr={}, len={}", module_name, func_name, output_ptr, output_bytes.len()));
		// Return the pointer and length packed into a single u64
		let result_ptrlen = ((output_ptr as u64) << 32) | (output_bytes.len() as u64);
		result_ptrlen
	};
	linker.func_wrap(&out_module_name.clone(), &out_func_name.clone(), func)?;
	Ok(())
} // end fn func_wrap

/// Wrapper for HostFunction that operates on raw byte slices, allowing for maximum
/// flexibility in how the function processes input and produces output, without any
/// assumptions about data formats or serialization
pub fn func_wrap_bytes(
	linker: &mut wasmtime::Linker<()>,
	alloc: std::sync::Arc<std::sync::OnceLock<wasmtime::TypedFunc<u32, u32>>>,
	module_name: &str,
	func_name: &str,
	func: impl Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static
) -> Result<(), Box<dyn std::error::Error>> {

	// Clone the module and function name strings to ensure they are owned and can be moved into the closure
	let module_name = module_name.to_string();
	let out_module_name = module_name.clone();
	let func_name = func_name.to_string();
	let out_func_name = func_name.clone();
	
	// Define the host function that will be called by the WASM module,
	// which reads input data from WASM memory, deserializes it, calls
	// the provided Rust closure, serializes the output, allocates memory
	// for the output in WASM, writes the output back to WASM memory, and
	// returns a pointer and length to the output data
	let func = move |mut caller: wasmtime::Caller<'_, ()>, ptrlen: u64| -> u64 {
		
		// Extract the pointer and length from the combined u64 argument
		let ptr = (ptrlen >> 32) as u32;
		let len = (ptrlen & 0xffffffff) as u32;
		
		// Read the input data from WASM memory into a Rust byte vector
		let mut memory_read = vec![0; len as usize];
		let memory = caller
			.get_export("memory")
			.and_then(|export| export.into_memory())
			.expect(&format!("Host function '{}.{}' could not access exported memory", module_name, func_name));
		let _ = memory.read(&mut caller, ptr as usize, &mut memory_read)
			.expect(&format!("Host function '{}.{}' failed to read memory for ptr={}, len={}", module_name, func_name, ptr, len));
		
		// For the bytes version, we just pass the raw bytes
		// to the closure without deserialization
		let input_data = memory_read;
		let output_bytes = func(&input_data);
		let alloc_func = alloc.get().expect(&format!("Host function '{}.{}' called before alloc function was set", module_name, func_name));
		let output_ptr = alloc_func.call(&mut caller, output_bytes.len() as u32)
			.expect(&format!("Host function '{}.{}' failed to call alloc for output size {}", module_name, func_name, output_bytes.len()));
		memory.write(&mut caller, output_ptr as usize, &output_bytes)
			.expect(&format!("Host function '{}.{}' failed to write output to memory at ptr={}, len={}", module_name, func_name, output_ptr, output_bytes.len()));
		// Return the pointer and length packed into a single u64
		let result_ptrlen = ((output_ptr as u64) << 32) | (output_bytes.len() as u64);
		result_ptrlen
	};
	linker.func_wrap(&out_module_name.clone(), &out_func_name.clone(), func)?;
	Ok(())
} // end fn func_wrap_bytes

impl WasmModule {

	/// Create a new WasmModule by instantiating the provided WASM bytes
	/// with the given host functions, setting up the necessary imports
	/// and memory handling
	pub fn new(bytes: &[u8], functions: Vec<HostFunction>) -> Result<Self, Box<dyn std::error::Error>> {
		let module = wasmtime::Module::new(&native::ENGINE, bytes)?;
		let mut store = wasmtime::Store::new(&native::ENGINE, ());
		let mut linker = wasmtime::Linker::new(&native::ENGINE);
		let alloc = std::sync::Arc::new(std::sync::OnceLock::new());
		
		func_wrap(&mut linker, alloc.clone(), "sys", "log", crate::sys_functions::log)?;
		
		for func in functions {
			let name = func.name.clone();
			let alloc_clone = alloc.clone();
			func_wrap_bytes(&mut linker, alloc_clone, "env", &name,
				move |input| { (func.func)(input) }
			)?;
		}

		let instance = linker.instantiate(&mut store, &module)?;
		let memory = instance
			.get_memory(&mut store, "memory")
			.ok_or("Exported memory not found")?;
		let alloc_func = instance.get_func(&mut store, "alloc").ok_or("Alloc function not found")?;
		let _ = alloc.set(alloc_func.typed::<u32, u32>(&store)?);
		let free_func = instance.get_func(&mut store, "free").ok_or("Free function not found")?;
		let free = std::sync::Arc::new(std::sync::OnceLock::new());
		let _ = free.set(free_func.typed::<(u32, u32), ()>(&store)?);

		let mut exports = std::collections::HashMap::new();
		
		// Iterate over all exports in the module and store any functions that match the expected signature for exported functions
		for export in module.exports() {
			if let wasmtime::ExternType::Func(func_ty) = export.ty() {
				// We expect exported functions to have the signature (i32, i32) -> i64, where the input is a pointer and length to serialized input data, and the output is a pointer and length packed into an i64
				let params: Vec<_> = func_ty.params().collect();
				let results: Vec<_> = func_ty.results().collect();
				if params.len() == 2 && results.len() == 1 {
					if let (wasmtime::ValType::I32, wasmtime::ValType::I32) = (params[0].clone(), params[1].clone()) {
						if let wasmtime::ValType::I64 = results[0] {
							let func = instance.get_func(&mut store, export.name()).ok_or(format!("Exported function '{}' not found", export.name()))?;
							exports.insert(export.name().to_string(), func.typed::<(u32, u32), u64>(&store)?);
						}
					}
				}
			}
		}
		
		let mut module = Self { module, instance, store, memory, alloc, free, functions: exports };
		
		// Call the initialization function to set up panic hooks and any other necessary runtime state
		module.call::<(), ()>("__sys_init", ()).ok();

		Ok(module)
	} // end fn new

	/// Call the alloc function exported by the WASM module to allocate memory for input or
	/// output data, returning the pointer to the allocated memory
	pub fn call_alloc(&mut self, size: u32) -> Result<u32, Box<dyn std::error::Error>> {
		let alloc_func = self.alloc.get().ok_or("Allocation function not set")?;
		let ptr = alloc_func.call(&mut self.store, size)?;
		Ok(ptr)
	} // end fn call_alloc

	/// Call the free function exported by the WASM module to free memory at the
	/// given pointer and size, ensuring that any allocated resources are properly released
	pub fn call_free(&mut self, ptr: u32, size: u32) -> Result<(), Box<dyn std::error::Error>> {
		let free_func = self.free.get().ok_or("Free function not set")?;
		free_func.call(&mut self.store, (ptr, size))?;
		Ok(())
	} // end fn call_free

	/// Call an exported WASM function that takes a pointer and length as input and returns a pointer and length as output
	pub fn call_ptr(&mut self, func_name: &str, ptr: u32, len: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
		let func = self.functions.get(func_name).ok_or(format!("Function '{}' not found in exports", func_name))?;
		let results = func.call(&mut self.store, (ptr, len))?;
		let result_ptr = ((results as u64) >> 32) as u32;
		let result_len = ((results as u64) & 0xffffffff) as u32;
		let mut result_data = vec![0; result_len as usize];
		self.memory.read(&self.store, result_ptr as usize, &mut result_data)?;
		self.call_free(result_ptr, result_len)?;
		Ok(result_data)
	} // end fn call_ptr

	/// Call an exported WASM function that takes a serialized input and returns a serialized output,
	pub fn call<T: serde::Serialize, O: serde::de::DeserializeOwned>(&mut self, func_name: &str, input: T) -> Result<O, Box<dyn std::error::Error>> {
		// 1. Serialize the input using rmp_serde
		let bytes = rmp_serde::to_vec(&input)?;
		// 2. Allocate memory in the WASM module
		let alloc_ptr = self.call_alloc(bytes.len() as u32)?;
		// 3. Write the serialized bytes into WASM memory
		self.memory.write(&mut self.store, alloc_ptr as usize, &bytes)?;
		// 4. Call the WASM function with the pointer and length of the input data
		let result = self.call_ptr(func_name, alloc_ptr, bytes.len() as u32)?;
		// 5. Deserialize the output bytes into the expected output type
		let output: Result<O, rmp_serde::decode::Error> = rmp_serde::from_slice(&result);
		Ok(output?)
	} // end fn call

} // end impl WasmModule
