use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context;

use crate::utils::{cargo_cmd, comma_delimited, parse_features, test_package_features, XTaskMetadata};

#[derive(Debug, Clone, clap::Parser)]
pub struct PowerSet {
	#[clap(long, value_delimiter = ',')]
	#[clap(alias = "feature")]
	/// Features to test
	features: Vec<String>,
	#[clap(long, value_delimiter = ',')]
	#[clap(alias = "exclude-feature")]
	/// Features to exclude from testing
	exclude_features: Vec<String>,
	#[clap(long, short, value_delimiter = ',')]
	#[clap(alias = "package")]
	/// Packages to test
	packages: Vec<String>,
	#[clap(long, short, value_delimiter = ',')]
	#[clap(alias = "exclude-package")]
	/// Packages to exclude from testing
	exclude_packages: Vec<String>,
	#[clap(long, default_value = "0")]
	/// Number of tests to skip
	skip: usize,
	#[clap(long, default_value = "true")]
	/// Fail fast
	fail_fast: bool,
	#[clap(long, default_value = "target/power-set")]
	/// Target directory
	target_dir: String,
	#[clap(long, action = clap::ArgAction::SetTrue)]
	/// Override target directory
	no_override_target_dir: bool,
	#[clap(name = "command", default_value = "clippy")]
	/// Command to run
	command: String,
	#[clap(last = true)]
	/// Additional arguments to pass to the command
	args: Vec<String>,
}

const IGNORED_PACKAGES: &[&str] = &["scuffle-workspace-hack", "xtask"];

impl PowerSet {
	pub fn run(self) -> anyhow::Result<()> {
		let start = std::time::Instant::now();

		let metadata = crate::utils::metadata()?;

		let mut tests = BTreeMap::new();

		let features = self.features.into_iter().map(|f| f.to_lowercase()).collect::<BTreeSet<_>>();

		let (added_global_features, added_package_features) = parse_features(features.iter().map(|f| f.as_str()));
		let (excluded_global_features, excluded_package_features) =
			parse_features(self.exclude_features.iter().map(|f| f.as_str()));

		let ignored_packages = self
			.exclude_packages
			.into_iter()
			.chain(IGNORED_PACKAGES.iter().map(|p| p.to_string()))
			.map(|p| p.to_lowercase())
			.collect::<BTreeSet<_>>();
		let packages = self.packages.into_iter().map(|p| p.to_lowercase()).collect::<BTreeSet<_>>();

		let xtask_metadata = metadata
			.workspace_packages()
			.iter()
			.map(|p| {
				XTaskMetadata::from_package(p).with_context(|| format!("failed to get metadata for package {}", p.name))
			})
			.collect::<anyhow::Result<Vec<_>>>()?;

		// For each package in the workspace, run tests
		for (package, xtask_metadata) in metadata.workspace_packages().iter().zip(xtask_metadata.iter()) {
			if ignored_packages.contains(&package.name.to_lowercase())
				|| !(packages.is_empty() || packages.contains(&package.name.to_lowercase()))
			{
				continue;
			}

			let added_features = added_package_features
				.get(package.name.as_str())
				.into_iter()
				.flatten()
				.chain(added_global_features.iter())
				.copied()
				.filter(|s| package.features.contains_key(*s));
			let excluded_features = excluded_package_features
				.get(package.name.as_str())
				.into_iter()
				.flatten()
				.chain(excluded_global_features.iter())
				.copied()
				.filter(|s| package.features.contains_key(*s));

			let features = test_package_features(package, added_features, excluded_features, xtask_metadata)
				.with_context(|| package.name.clone())?;

			tests.insert(package.name.as_str(), features);
		}

		let mut i = 0;
		let total = tests.values().map(|s| s.len()).sum::<usize>();

		let mut failed = Vec::new();

		for (package, power_set) in tests.iter() {
			for features in power_set.iter() {
				if i < self.skip {
					i += 1;
					continue;
				}

				let mut cmd = cargo_cmd();
				cmd.arg(&self.command);
				cmd.args(&self.args);
				cmd.arg("--no-default-features");
				if !features.is_empty() {
					cmd.arg("--features").arg(comma_delimited(features.iter()));
				}
				cmd.arg("--package").arg(package);

				if !self.no_override_target_dir {
					cmd.arg("--target-dir").arg(&self.target_dir);
				}

				println!("executing {:?} ({}/{})", cmd, i, total);

				if !cmd.status()?.success() {
					failed.push((*package, features));
					if self.fail_fast {
						anyhow::bail!(
							"failed to execute command for package {} with features {:?} after {:?}",
							package,
							features,
							start.elapsed()
						);
					}
				}

				i += 1;
			}
		}

		if !failed.is_empty() {
			eprintln!("failed to execute command for the following:");
			for (package, features) in failed {
				eprintln!("  {} with features {:?}", package, features);
			}

			anyhow::bail!("failed to execute command for some packages after {:?}", start.elapsed());
		}

		println!("all commands executed successfully after {:?}", start.elapsed());

		Ok(())
	}
}
