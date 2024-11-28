#[cfg(feature = "serde")]
use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
	#[cfg(feature = "serde")]
	let descriptor_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("proto_descriptor.bin");

	let config = tonic_build::configure()
		.compile_well_known_types(true)
		.build_server(cfg!(feature = "server"))
		.build_client(cfg!(feature = "client"));

	#[cfg(feature = "serde")]
	let config = config.file_descriptor_set_path(&descriptor_path);

	config.compile_protos(
		&[
			"scuffle/image_processor/service.proto",
			"scuffle/image_processor/types.proto",
			"scuffle/image_processor/events.proto",
		],
		&["./"],
	)?;

	#[cfg(feature = "serde")]
	let descriptor_set = std::fs::read(&descriptor_path)?;

	#[cfg(feature = "serde")]
	pbjson_build::Builder::new()
		.register_descriptors(&descriptor_set)?
		.build(&[".scuffle.image_processor"])?;

	Ok(())
}
