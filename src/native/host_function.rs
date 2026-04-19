
/// Represents a host function that can be imported by a WASM module, allowing Rust closures to be called from WASM with automatic serialization and deserialization of input and output data
pub struct HostFunction {
	pub name: String,
	pub func: Box<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>,
} // end struct HostFunction

impl HostFunction {

	/// Create a new HostFunction with a raw byte callback, allowing for maximum flexibility in how the function processes input and produces output
	pub fn new_bytes(name: impl Into<String>, func: impl Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static) -> Self {
		Self { name: name.into(), func: Box::new(func) }
	} // end fn new

	/// Create a new HostFunction with typed input and output, automatically handling serialization and deserialization using rmp_serde
	pub fn new<I, O, F>(name: &str, func: F) -> Self
	where
		I: serde::de::DeserializeOwned + 'static,
		O: serde::Serialize + 'static,
		F: Fn(I) -> O + Send + Sync + 'static,
	{
		let name = name.to_string();
		Self::new_bytes(&name.clone(), move |input| -> Vec<u8> {
			let input: I = rmp_serde::from_slice(input).expect(&format!("Failed to deserialize input for {}", name));
			let output: O = func(input);
			rmp_serde::to_vec(&output).expect(&format!("Failed to serialize output for {}", name))
		})
	} // end fn new

} // end impl HostFunction