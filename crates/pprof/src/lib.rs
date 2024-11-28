#![doc = include_str!("../README.md")]

mod cpu;

#[derive(Debug, thiserror::Error)]
pub enum PprofError {
	#[error(transparent)]
	Io(#[from] std::io::Error),
	#[error(transparent)]
	Pprof(#[from] pprof::Error),
}

pub use cpu::Cpu;
