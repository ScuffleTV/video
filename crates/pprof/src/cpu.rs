use std::io::Write;

use flate2::write::GzEncoder;
use flate2::Compression;
use pprof::protos::Message;

use crate::PprofError;

/// A CPU profiler.
pub struct Cpu(pprof::ProfilerGuardBuilder);

impl Cpu {
	/// Create a new CPU profiler.
	/// - `frequency` is the sampling frequency in Hz.
	/// - `blocklist` is a list of functions to exclude from the profile.
	pub fn new<S: AsRef<str>>(frequency: i32, blocklist: &[S]) -> Self {
		Self(
			pprof::ProfilerGuardBuilder::default()
				.frequency(frequency)
				.blocklist(blocklist),
		)
	}

	/// Capture a pprof profile for the given duration.
	/// The profile is compressed using gzip.
	/// The profile can be analyzed using the `pprof` tool.
	/// Warning: This method is blocking and may take a long time to complete.
	/// It is recommended to run it in a separate thread.
	pub fn capture(&self, duration: std::time::Duration) -> Result<Vec<u8>, PprofError> {
		let profiler = self.0.clone().build()?;

		std::thread::sleep(duration);

		let report = profiler.report().build()?;

		let pprof = report.pprof()?;

		let mut gz = GzEncoder::new(Vec::new(), Compression::default());
		gz.write_all(&pprof.encode_to_vec())?;
		Ok(gz.finish()?)
	}
}
