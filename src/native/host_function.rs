use crate::native::LinkerHandle;


pub trait NativeHostFunction {
	fn into_linked(self, linker: &mut LinkerHandle, module_name: &str);
} // end trait NativeHostFunction

impl NativeHostFunction for crate::HostFunction {

	fn into_linked(self, linker: &mut LinkerHandle, module_name: &str) {
		let name = self.name.clone();
		let func = self.function;
		let memory = linker.memory.clone();
		let alloc = linker.alloc.clone();
		let module_name = module_name.to_string();
		let module_name_clone = module_name.clone();

		let func = move |mut caller: wasmtime::Caller<'_, ()>, ptr: u64| -> u64 {
			let memory = memory.get().expect(&format!("Memory not set for host function '{}.{}'", module_name, name));
			let mut length = [0u8; 8];
			memory.read(&mut caller, ptr as usize, &mut length).unwrap();
			let len = u64::from_le_bytes(length);
			let mut input_bytes = vec![0; len as usize];
			memory.read(&mut caller, ptr as usize + 8, &mut input_bytes).unwrap();
			let output_bytes = (func)(input_bytes.as_slice());
			let alloc = alloc.get().expect(&format!("Alloc function not set for host function '{}.{}'", module_name, name));
			let output_len = output_bytes.len() as u64;
			let output_ptr = alloc.call(&mut caller, output_len + 8).expect(&format!("Host function '{}.{}' failed to allocate memory for output of size {}", module_name, name, output_len + 8));
			memory.write(&mut caller, output_ptr as usize, &output_len.to_le_bytes()).expect(&format!("Host function '{}.{}' failed to write output length at ptr={}", module_name, name, output_ptr));
			memory.write(&mut caller, output_ptr as usize + 8, &output_bytes).expect(&format!("Host function '{}.{}' failed to write output to memory at ptr={}, len={}", module_name, name, output_ptr + 8, output_bytes.len()));
			output_ptr
		}; // end let func

		linker.linker.func_wrap(&module_name_clone, &self.name, func).unwrap();
		
	} // end fn into_linked
	
} // end impl NativeHostFunction for HostFunction
