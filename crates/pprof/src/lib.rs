//! A crate designed to provide a more ergonomic interface to the `pprof` crate.
//!
//! ## Example
//!
//! ```rust,no_run
//! // Create a new CPU profiler with a sampling frequency of 1000 Hz and an empty ignore list.
//! let cpu = scuffle_pprof::Cpu::new::<String>(1000, &[]);
//!
//! // Capture a pprof profile for 10 seconds.
//! // This call is blocking. It is recommended to run it in a separate thread.
//! let capture = cpu.capture(std::time::Duration::from_secs(10)).unwrap();
//!
//! // Write the profile to a file.
//! std::fs::write("capture.pprof", capture).unwrap();
//! ```
//!
//! ## Analyzing the profile
//!
//! The resulting profile can be analyzed using the [`pprof`](https://github.com/google/pprof) tool.
//!
//! For example, to generate a flamegraph:
//! ```sh
//! pprof -svg capture.pprof
//! ```
//!
//! ## Status
//!
//! This crate is currently under development and is not yet stable.
//!
//! Unit tests are not yet fully implemented. Use at your own risk.
//!
//! ## License
//!
//! This project is licensed under the [MIT](./LICENSE.MIT) or [Apache-2.0](./LICENSE.Apache-2.0) license.
//! You can choose between one of them if you use this work.
//!
//! `SPDX-License-Identifier: MIT OR Apache-2.0`
#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]

mod cpu;

pub use cpu::Cpu;

/// An error that can occur while profiling.
#[derive(Debug, thiserror::Error)]
pub enum PprofError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Pprof(#[from] pprof::Error),
}

#[cfg(test)]
#[cfg_attr(all(coverage_nightly, test), coverage(off))]
mod tests {
    use std::io::Read;
    use std::time::SystemTime;

    use flate2::read::GzDecoder;
    use pprof::protos::Message;

    use crate::Cpu;

    #[test]
    fn empty_profile() {
        let before_nanos = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos() as i64;

        let cpu = Cpu::new::<String>(1000, &[]);
        let profile = cpu.capture(std::time::Duration::from_secs(1)).unwrap();

        // Decode the profile
        let mut reader = GzDecoder::new(profile.as_slice());
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();
        let profile = pprof::protos::Profile::decode(buf.as_slice()).unwrap();

        assert!(profile.duration_nanos > 1_000_000_000);
        assert!(profile.time_nanos > before_nanos);
        let now_nanos = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos() as i64;
        assert!(profile.time_nanos < now_nanos);

        assert_eq!(profile.string_table[profile.drop_frames as usize], "");
        assert_eq!(profile.string_table[profile.keep_frames as usize], "");

        let Some(period_type) = profile.period_type else {
            panic!("missing period type");
        };
        assert_eq!(profile.string_table[period_type.ty as usize], "cpu");
        assert_eq!(profile.string_table[period_type.unit as usize], "nanoseconds");

        assert_eq!(profile.period, 1_000_000);
    }
}
