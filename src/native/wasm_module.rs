use serde::{Serialize, de::DeserializeOwned};
pub use crate::HostFunction;
use crate::native;

/// Represents an instantiated WebAssembly module
pub struct WasmModule {
	module: wasmtime::Module,
	instance: wasmtime::Instance,
	store: wasmtime::Store<()>,
	memory: wasmtime::Memory,
	alloc: std::sync::Arc<std::sync::Mutex<Option<wasmtime::Func>>>,
} // end struct WasmModule

pub type AllocFunc = std::sync::Arc<std::sync::Mutex<Option<wasmtime::Func>>>;

/// Call the alloc function exported by the WASM module to allocate memory for input or output data, returning the pointer to the allocated memory
pub fn call_alloc(module: &mut WasmModule, size: u32) -> Result<u32, Box<dyn std::error::Error>> {
	let binding = module.alloc.lock().unwrap();
	let alloc_func = binding.as_ref().ok_or("Allocation function not set")?;
	let mut results = [wasmtime::Val::I32(0)];
	alloc_func.call(&mut module.store, &[wasmtime::Val::I32(size as i32)], &mut results)?;
	if let wasmtime::Val::I32(ptr) = results[0] { Ok(ptr as u32)
	} else { Err("Alloc function did not return an i32".into()) }
} // end fn call_alloc

/// Generic wrapper for HostFunction that handles serialization and deserialization
/// of input and output data, allowing Rust closures to be easily exposed as WASM
/// imports without needing to manually manage memory or data formats
pub fn func_wrap<I: DeserializeOwned, O: Serialize>(linker: &mut wasmtime::Linker<()>, alloc: AllocFunc, module_name: &str, func_name: &str, func: impl Fn(I) -> O + Send + Sync + 'static) -> Result<(), Box<dyn std::error::Error>> {
	let module_name = module_name.to_string();
	let out_module_name = module_name.clone();
	let func_name = func_name.to_string();
	let out_func_name = func_name.clone();
	let func = move |mut caller: wasmtime::Caller<'_, ()>, ptrlen: u64| -> u64 {
		let ptr = (ptrlen >> 32) as u32;
		let len = (ptrlen & 0xffffffff) as u32;
		let mut memory_read = vec![0; len as usize];
		let memory = caller
			.get_export("memory")
			.and_then(|export| export.into_memory())
			.expect(&format!("Host function '{}.{}' could not access exported memory", module_name, func_name));
		let _ = memory.read(&mut caller, ptr as usize, &mut memory_read)
			.expect(&format!("Host function '{}.{}' failed to read memory for ptr={}, len={}", module_name, func_name, ptr, len));
		let input_data = rmp_serde::from_slice(&memory_read).expect(&format!("Host function '{}.{}' failed to deserialize input", module_name, func_name));
		let output_bytes = rmp_serde::to_vec(&func(input_data)).expect(&format!("Host function '{}.{}' failed to serialize output", module_name, func_name));
		if let Some(alloc_func) = alloc.lock().unwrap().as_ref() {
			let mut results = [wasmtime::Val::I32(0)];
			alloc_func.call(&mut caller, &[wasmtime::Val::I32(output_bytes.len() as i32)], &mut results)
				.expect(&format!("Host function '{}.{}' failed to call alloc for output size {}", module_name, func_name, output_bytes.len()));
			if let wasmtime::Val::I32(output_ptr) = results[0] {
				memory.write(&mut caller, output_ptr as usize, &output_bytes)
					.expect(&format!("Host function '{}.{}' failed to write output to memory at ptr={}, len={}", module_name, func_name, output_ptr, output_bytes.len()));
				// Return the pointer and length packed into a single u64
				let result_ptrlen = ((output_ptr as u64) << 32) | (output_bytes.len() as u64);
				result_ptrlen
			} else {
				panic!("Host function '{}.{}' alloc did not return an i32", module_name, func_name);
			}
		} else {
			panic!("Host function '{}.{}' called before alloc function was set", module_name, func_name);
		}
	};
	linker.func_wrap(&out_module_name.clone(), &out_func_name.clone(), func)?;
	Ok(())
} // end fn func_wrap

/// Wrapper for HostFunction that operates on raw byte slices, allowing for maximum flexibility in how the function processes input and produces output, without any assumptions about data formats or serialization
pub fn func_wrap_bytes(linker: &mut wasmtime::Linker<()>, alloc: AllocFunc, module_name: &str, func_name: &str, func: impl Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static) -> Result<(), Box<dyn std::error::Error>> {
	let module_name = module_name.to_string();
	let out_module_name = module_name.clone();
	let func_name = func_name.to_string();
	let out_func_name = func_name.clone();
	let func = move |mut caller: wasmtime::Caller<'_, ()>, ptrlen: u64| -> u64 {
		let ptr = (ptrlen >> 32) as u32;
		let len = (ptrlen & 0xffffffff) as u32;
		let mut memory_read = vec![0; len as usize];
		let memory = caller
			.get_export("memory")
			.and_then(|export| export.into_memory())
			.expect(&format!("Host function '{}.{}' could not access exported memory", module_name, func_name));
		let _ = memory.read(&mut caller, ptr as usize, &mut memory_read)
			.expect(&format!("Host function '{}.{}' failed to read memory for ptr={}, len={}", module_name, func_name, ptr, len));
		let output_bytes = func(&memory_read);
		if let Some(alloc_func) = alloc.lock().unwrap().as_ref() {
			let mut results = [wasmtime::Val::I32(0)];
			alloc_func.call(&mut caller, &[wasmtime::Val::I32(output_bytes.len() as i32)], &mut results)
				.expect(&format!("Host function '{}.{}' failed to call alloc for output size {}", module_name, func_name, output_bytes.len()));
			if let wasmtime::Val::I32(output_ptr) = results[0] {
				memory.write(&mut caller, output_ptr as usize, &output_bytes)
					.expect(&format!("Host function '{}.{}' failed to write output to memory at ptr={}, len={}", module_name, func_name, output_ptr, output_bytes.len()));
				// Return the pointer and length packed into a single u64
				let result_ptrlen = ((output_ptr as u64) << 32) | (output_bytes.len() as u64);
				result_ptrlen
			} else {
				panic!("Host function '{}.{}' alloc did not return an i32", module_name, func_name);
			}
		} else {
			panic!("Host function '{}.{}' called before alloc function was set", module_name, func_name);
		}
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
		let alloc: AllocFunc = std::sync::Arc::new(std::sync::Mutex::new(None));
		
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
		*alloc.lock().unwrap() = Some(instance.get_func(&mut store, "alloc").ok_or("Alloc function not found")?);
		
		let mut module = Self { module, instance, store, memory, alloc };
		
		// Call the initialization function to set up panic hooks and any other necessary runtime state
		module.call::<(), ()>("__sys_init", ()).ok();

		Ok(module)
	} // end fn new

	/// Allocate memory in the WASM module by calling the exported alloc function with the given size
	pub fn call_alloc(&mut self, size: u32) -> Result<u32, Box<dyn std::error::Error>> {
		call_alloc(self, size)
	} // end fn call_alloc

	/// Free memory in the WASM module at the given pointer and size
	pub fn call_free(&mut self, ptr: u32, size: u32) -> Result<(), Box<dyn std::error::Error>> {
		let free_func = self.instance.get_func(&mut self.store, "free").ok_or("Free function not found")?;
		free_func.call(&mut self.store, &[wasmtime::Val::I32(ptr as i32), wasmtime::Val::I32(size as i32)], &mut [])?;
		Ok(())
	} // end fn call_free

	/// Call an exported WASM function that takes a pointer and length as input and returns a pointer and length as output
	pub fn call_ptr(&mut self, func_name: &str, ptr: u32, len: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
		let func = self.instance.get_func(&mut self.store, func_name).ok_or(format!("Function '{}' not found", func_name))?;
		let mut results = [wasmtime::Val::I64(0)];
		func.call(&mut self.store, &[wasmtime::Val::I32(ptr as i32), wasmtime::Val::I32(len as i32)], &mut results)?;
		if let wasmtime::Val::I64(result_ptrlen) = results[0] {
			let result_ptr = ((result_ptrlen as u64) >> 32) as u32;
			let result_len = ((result_ptrlen as u64) & 0xffffffff) as u32;
			let mut result_data = vec![0; result_len as usize];
			self.memory.read(&self.store, result_ptr as usize, &mut result_data)?;
			self.call_free(result_ptr, result_len)?;
			Ok(result_data)
		} else { Err("Function did not return an i64 ptr/len".into()) }
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
