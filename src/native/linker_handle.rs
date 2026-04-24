use std::sync::{Arc, OnceLock};


pub struct LinkerHandle {
	pub linker: wasmtime::Linker<()>,
	pub memory: Arc<OnceLock<wasmtime::Memory>>,
	pub alloc: Arc<OnceLock<wasmtime::TypedFunc<u64, u64>>>,
	pub free: Arc<OnceLock<wasmtime::TypedFunc<(u64, u64), ()>>>,
} // end struct LinkerHandle

impl LinkerHandle {
	pub fn new() -> Self {
		Self {
			linker: wasmtime::Linker::new(&super::ENGINE),
			memory: Arc::new(OnceLock::new()),
			alloc: Arc::new(OnceLock::new()),
			free: Arc::new(OnceLock::new()),
		}
	} // end fn new
} // end impl LinkerHandle