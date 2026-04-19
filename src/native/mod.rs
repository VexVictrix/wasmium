mod wasm_module; pub use wasm_module::WasmModule;
mod host_function; pub use host_function::HostFunction;

use wasmtime::Engine;
static ENGINE: std::sync::LazyLock<Engine> = std::sync::LazyLock::new(|| { Engine::default() });
