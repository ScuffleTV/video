use global::Global;
use management::ManagementSvc;
use scuffle_bootstrap_telemetry::TelemetrySvc;
use scuffle_signal::SignalSvc;
use worker::WorkerSvc;

mod config;
mod database;
mod drive;
mod event_queue;
pub mod events;
mod global;
mod management;
mod worker;

scuffle_bootstrap::main! {
	Global {
		SignalSvc,
		ManagementSvc,
		WorkerSvc,
		TelemetrySvc,
	}
}
