mod wasm_module; pub use wasm_module::NativeWasmiumModule;
mod host_function; pub use host_function::NativeHostFunction;
mod linker_handle; pub use linker_handle::LinkerHandle;

use wasmtime::Engine;
static ENGINE: std::sync::LazyLock<Engine> = std::sync::LazyLock::new(|| { Engine::default() });
