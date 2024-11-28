use wasm_bindgen::prelude::*;

mod tracing_wasm;

mod player;
mod thumbnail;

#[wasm_bindgen(start)]
pub fn main() {
	console_error_panic_hook::set_once();
	tracing_wasm::set_as_global_default();

	tracing::trace!("scuffle video player loaded");
}
