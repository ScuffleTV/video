/// Some of the code here is from the `ui_test` crate, and some is from
/// `trybuild`. Mainly related to how dependencies are found & included in the
/// build. Both of which are licensed under the MIT license.
use std::{
	collections::{HashMap, HashSet},
	ffi::OsString,
	path::PathBuf,
	process::{Command, Stdio},
};

use cargo_metadata::{BuildScript, DependencyKind, Edition};
use cargo_platform::Cfg;
use target_triple::TARGET;

use crate::{features, Config};

#[derive(Default, Debug)]
/// Describes where to find the binaries built for the dependencies
pub struct Dependencies {
	pub import_paths: Vec<PathBuf>,
	pub import_libs: Vec<PathBuf>,
	pub dependencies: Vec<(String, Vec<PathBuf>)>,
	pub edition: Edition,
	pub cfg: Vec<String>,
	pub env: Vec<(String, String)>,
}

impl Dependencies {
	pub fn new(config: &Config) -> Result<Self, Errored> {
		let mut build = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));

		let target_dir = if config.target_dir.ends_with(TARGET) {
			config.target_dir.parent().unwrap()
		} else {
			config.target_dir.as_ref()
		};

		build.arg("test");
		build.arg("--no-run");
		build.arg("--message-format=json");
		build.arg("--target-dir");
		build.arg(target_dir);
		build.arg("--manifest-path");
		build.arg(config.manifest.as_ref());
		build.arg("--locked");
		if let Some(rustflags) = std::env::var_os("RUSTFLAGS") {
			build.env("RUSTFLAGS", rustflags);
		}
		if let Some(llvm_profile_file) = std::env::var_os("LLVM_PROFILE_FILE") {
			build.env("LLVM_PROFILE_FILE", llvm_profile_file);
		}

		if let Some(features) = features::find() {
			build.arg(format!("--features={}", features.join(",")));
		}

		build.stderr(Stdio::piped());
		build.stdout(Stdio::piped());

		// This isnt trybuild but a bunch of libraries set this cfg flag to avoid
		// the runner from specifying a target.
		if !cfg!(trybuild_no_target) && !cfg!(postcompile_no_target) && config.target_dir.ends_with(TARGET) {
			build.arg(format!("--target={TARGET}"));
		}

		let output = match build.output() {
			Err(e) => {
				return Err(Errored {
					command: format!("{build:?}"),
					errors: vec![],
					stderr: e.to_string(),
					stdout: String::new(),
				});
			}
			Ok(o) => o,
		};

		if !output.status.success() {
			let stdout = output
				.stdout
				.split(|&b| b == b'\n')
				.flat_map(|line| match serde_json::from_slice::<cargo_metadata::Message>(line) {
					Ok(cargo_metadata::Message::CompilerArtifact(artifact)) => format!("{artifact:?}\n").into_bytes(),
					Ok(cargo_metadata::Message::BuildFinished(bf)) => format!("{bf:?}\n").into_bytes(),
					Ok(cargo_metadata::Message::BuildScriptExecuted(be)) => format!("{be:?}\n").into_bytes(),
					Ok(cargo_metadata::Message::TextLine(s)) => s.into_bytes(),
					Ok(cargo_metadata::Message::CompilerMessage(msg)) => msg
						.target
						.src_path
						.as_str()
						.as_bytes()
						.iter()
						.copied()
						.chain([b'\n'])
						.chain(msg.message.rendered.unwrap_or_default().into_bytes())
						.collect(),
					Ok(_) => vec![],
					Err(_) => line.iter().copied().chain([b'\n']).collect(),
				})
				.collect::<Vec<_>>();

			return Err(Errored {
				command: format!("{build:?}"),
				errors: vec![],
				stderr: String::from_utf8(output.stderr).unwrap(),
				stdout: String::from_utf8(stdout).unwrap(),
			});
		}

		// Collect all artifacts generated
		let artifact_output = output.stdout;
		let mut import_paths: HashSet<PathBuf> = HashSet::new();
		let mut import_libs: HashSet<PathBuf> = HashSet::new();
		let mut artifacts = HashMap::new();
		let mut all_cfgs = HashMap::new();
		let mut all_env = HashMap::new();

		for line in artifact_output.split(|&b| b == b'\n') {
			let Ok(message) = serde_json::from_slice::<cargo_metadata::Message>(line) else {
				continue;
			};
			match message {
				cargo_metadata::Message::CompilerArtifact(artifact) if artifact.executable.is_none() => {
					if artifact.target.crate_types.iter().all(|ctype| {
						!matches!(
							ctype,
							cargo_metadata::CrateType::ProcMacro
								| cargo_metadata::CrateType::Lib
								| cargo_metadata::CrateType::RLib
						)
					}) {
						continue;
					}

					for filename in &artifact.filenames {
						import_paths.insert(filename.parent().unwrap().into());
					}

					let package_id = artifact.package_id;

					if let Some(prev) = artifacts.insert(package_id.clone(), Ok((artifact.target.name, artifact.filenames)))
					{
						artifacts.insert(
							package_id.clone(),
							Err(format!(
								"{prev:#?} vs {:#?} ({:?})",
								artifacts[&package_id], artifact.target.crate_types
							)),
						);
					}
				}
				cargo_metadata::Message::BuildScriptExecuted(BuildScript {
					linked_libs,
					linked_paths,
					cfgs,
					env,
					package_id,
					..
				}) => {
					import_paths.extend(linked_paths.into_iter().map(Into::into));
					import_libs.extend(linked_libs.into_iter().map(Into::into));

					all_cfgs.entry(package_id.clone()).or_insert_with(Vec::new).extend(cfgs);
					all_env.entry(package_id.clone()).or_insert_with(Vec::new).extend(env);
				}
				_ => {}
			}
		}

		// Check which crates are mentioned in the crate itself
		let mut metadata = cargo_metadata::MetadataCommand::new().cargo_command();
		metadata.arg("--manifest-path").arg(config.manifest.as_ref());
		metadata.arg("--locked");
		if let Some(features) = features::find() {
			metadata.arg(format!("--features={}", features.join(",")));
		}

		let output = match metadata.output() {
			Err(e) => {
				eprintln!("failed to run cargo metadata: \n{:#}", e);
				std::process::exit(1);
			}
			Ok(output) => output,
		};

		if !output.status.success() {
			eprintln!("cargo metadata failed: \n{}", String::from_utf8_lossy(&output.stderr));
			std::process::exit(1);
		}

		let output = output.stdout;

		if let Some(line) = output.split(|&b| b == b'\n').find(|line| line.starts_with(b"{")) {
			let rustc_cfg = rustc_cfg();

			let metadata: cargo_metadata::Metadata = serde_json::from_slice(line).map_err(|err| Errored {
				command: "decoding cargo metadata json".into(),
				errors: vec![],
				stderr: err.to_string(),
				stdout: String::new(),
			})?;

			let root = metadata
				.packages
				.iter()
				.find(|package| {
					package.manifest_path.as_std_path().canonicalize().unwrap()
						== config.manifest.as_ref().canonicalize().unwrap()
				})
				.unwrap();

			let rustc_cfg = rustc_cfg
				.iter()
				.chain(all_cfgs.get(&root.id).into_iter().flatten())
				.map(|cfg| {
					let mut splits = cfg.splitn(2, '=');
					let key = splits.next().unwrap();
					let value = splits.next();
					if let Some(value) = value {
						Cfg::KeyPair(key.to_string(), value.to_string())
					} else {
						Cfg::Name(key.to_string())
					}
				})
				.collect::<Vec<_>>();

			let dependencies = root
                .dependencies
                .iter()
                .filter(|dep| matches!(dep.kind, DependencyKind::Normal | DependencyKind::Development))
                // Only consider dependencies that are enabled on the current target
                .filter(|dep| match &dep.target {
                    Some(platform) => platform.matches(TARGET, &rustc_cfg),
                    None => true,
                })
                .map(|dep| {
                    for p in &metadata.packages {
                        if p.name != dep.name {
                            continue;
                        }
                        if dep
                            .path
                            .as_ref()
                            .is_some_and(|path| p.manifest_path.parent().unwrap() == path)
                            || dep.req.matches(&p.version)
                        {
                            return (p, dep.rename.clone().unwrap_or_else(|| p.name.clone()));
                        }
                    }
                    panic!("dep not found: {dep:#?}")
                })
                // Also expose the root crate
                .chain(std::iter::once((root, root.name.clone())))
                .filter_map(|(package, name)| {
                    // Get the id for the package matching the version requirement of the dep
                    let id = &package.id;
                    // Return the name chosen in `Cargo.toml` and the path to the corresponding artifact
                    match artifacts.remove(id) {
                        Some(Ok((_, artifacts))) => Some(Ok((name.replace('-', "_"), artifacts.into_iter().map(Into::into).collect()))),
                        Some(Err(what)) => Some(Err(Errored {
                            command: what,
                            errors: vec![],
                            stderr: id.to_string(),
                            stdout: "`postcompile` does not support crates that appear as both build-dependencies and core dependencies".into(),
                        })),
                        None => {
                            if name == root.name {
                                // If there are no artifacts, this is the root crate and it is being built as a binary/test
                                // instead of a library. We simply add no artifacts, meaning you can't depend on functions
                                // and types declared in the root crate.
                                None
                            } else {
                                panic!("no artifact found for `{name}`(`{id}`):`\n{}", String::from_utf8_lossy(&artifact_output))
                            }
                        }
                    }
                })
                .collect::<Result<Vec<_>, Errored>>()?;
			let import_paths = import_paths.into_iter().collect();
			let import_libs = import_libs.into_iter().collect();

			return Ok(Dependencies {
				dependencies,
				import_paths,
				import_libs,
				edition: root.edition,
				cfg: all_cfgs.get(&root.id).cloned().unwrap_or_default(),
				env: all_env.get(&root.id).cloned().unwrap_or_default(),
			});
		}

		Err(Errored {
			command: "looking for json in cargo-metadata output".into(),
			errors: vec![],
			stderr: String::new(),
			stdout: String::new(),
		})
	}

	pub fn apply(&self, command: &mut Command) {
		for (name, artifacts) in &self.dependencies {
			for dependency in artifacts {
				command.arg("--extern");
				let mut dep = OsString::from(&name);
				dep.push("=");
				dep.push(dependency);
				command.arg(dep);
			}
		}
		for import_path in &self.import_paths {
			command.arg("-L");
			command.arg(import_path);
		}

		for import_path in &self.import_libs {
			command.arg("-l");
			command.arg(import_path);
		}

		command.arg("--edition");
		command.arg(self.edition.as_str());

		for cfg in &self.cfg {
			command.arg("--cfg");
			command.arg(cfg);
		}

		for (key, value) in &self.env {
			command.env(key, value);
		}
	}
}

#[derive(Debug)]
pub struct Errored {
	pub command: String,
	pub errors: Vec<String>,
	pub stderr: String,
	pub stdout: String,
}

fn rustc_cfg() -> Vec<String> {
	Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
		.arg("--print")
		.arg("cfg")
		.output()
		.unwrap()
		.stdout
		.split(|&b| b == b'\n')
		.map(|line| String::from_utf8_lossy(line).to_string())
		.filter(|line| !line.is_empty())
		.collect()
}
