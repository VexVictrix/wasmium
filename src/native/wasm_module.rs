use super::LinkerHandle;
use crate::{HostFunction, native::NativeHostFunction};

pub struct NativeWasmiumModule {
	pub store: wasmtime::Store<()>,
	pub linker: super::LinkerHandle,
	pub exports: std::collections::HashMap<String, wasmtime::TypedFunc<u64, u64>>,
}

impl NativeWasmiumModule {
	pub fn new(bytes: &[u8], functions: Vec<HostFunction>) -> Result<Self, Box<dyn std::error::Error>> {
		let mut linker = LinkerHandle::new();

		let module = wasmtime::Module::new(&super::ENGINE, bytes)?;
		let mut store = wasmtime::Store::new(&super::ENGINE, ());

		let log_fn = HostFunction::new("log", crate::sys_functions::log).into_linked(&mut linker, "wasmium_sys");
		
		for func in functions {
			let alloc_clone = linker.alloc.clone();
			func.into_linked(&mut linker, "env");
		}

		let instance = linker.linker.instantiate(&mut store, &module)?;
		let memory = instance
			.get_memory(&mut store, "memory")
			.ok_or("Exported memory not found")?;
		let _ = linker.memory.set(memory.clone());
		let alloc_func = instance
			.get_func(&mut store, "wasmium_alloc")
			.ok_or("Alloc function not found")?;
		let _ = linker.alloc.set(alloc_func.typed::<u64, u64>(&store)?);
		let free_func = instance
			.get_func(&mut store, "wasmium_free")
			.ok_or("Free function not found")?;
		let _ = linker.free.set(free_func.typed::<(u64, u64), ()>(&store)?);;

		let mut exports = std::collections::HashMap::new();
		
		// Iterate over all exports in the module and store any functions that match the expected signature for exported functions
		for export in module.exports() {
			if let wasmtime::ExternType::Func(func_ty) = export.ty() {
				// We expect exported functions to have the signature (i64, i64) -> i64, where the input is a pointer and length to serialized input data, and the output is a pointer and length packed into an i64
				// if func name is wasmium_alloc or wasmium_free, skip it since we already handled those
				if export.name() != "wasmium_alloc" && export.name() != "wasmium_free" {
					let func = instance.get_func(&mut store, export.name()).ok_or(format!("Exported function '{}' not found", export.name()))?;
					exports.insert(export.name().to_string(), func.typed::<u64, u64>(&store)?);
				}
			}
		}
		
		let mut module = Self { store, linker, exports };
		
		// Call the initialization function to set up panic hooks and any other necessary runtime state
		module.call::<(), ()>("__sys_init", ()).ok();

		Ok(module)

	} // end fn new
	pub fn call<T: serde::Serialize, O: serde::de::DeserializeOwned>(&mut self, func_name: &str, input: T) -> Result<O, Box<dyn std::error::Error>> {
		// 1. Serialize the input using rmp_serde
		let bytes = rmp_serde::to_vec(&input)?;
		// 2. Allocate memory in the WASM module
		let alloc_ptr = self.linker.alloc.get().ok_or("Alloc function not found")?.call(&mut self.store, (bytes.len() as u64 + 8))?;
		// 3. Write the length of the serialized bytes into the first 8 bytes at the allocated pointer
		self.linker.memory.get().ok_or("Memory not found")?.write(&mut self.store, alloc_ptr as usize, &(bytes.len() as u64).to_le_bytes())?;
		// 4. Write the serialized bytes into WASM memory starting at the allocated pointer + 8
		self.linker.memory.get().ok_or("Memory not found")?.write(&mut self.store, alloc_ptr as usize + 8, &bytes)?;
		// 5. Call the specified function with the pointer and length of the input data, and get back a pointer to the output data
		let func = self.exports.get(func_name).ok_or(format!("Function '{}' not found in exports", func_name))?;
		let result_ptr = func.call(&mut self.store, alloc_ptr)?;
		// 6. Read the length of the output data from the first 8 bytes at the result pointer
		let mut length = vec![0; 8];
		self.linker.memory.get().ok_or("Memory not found")?.read(&self.store, result_ptr as usize, &mut length)?;
		let result_len = u64::from_le_bytes(length.try_into().unwrap());
		// 7. Read the output data from WASM memory starting at the result pointer + 8
		let mut result_data = vec![0; result_len as usize];
		self.linker.memory.get().ok_or("Memory not found")?.read(&self.store, result_ptr as usize + 8, &mut result_data)?;
		// 8. Free the allocated memory for the input and output data in the WASM module
		self.linker.free.get().ok_or("Free function not found")?.call(&mut self.store, (result_ptr, result_len + 8))?;
		// 9. Deserialize the output bytes into the expected output type
		let output: Result<O, rmp_serde::decode::Error> = rmp_serde::from_slice(&result_data);
		Ok(output?)
	} // end fn call
}

