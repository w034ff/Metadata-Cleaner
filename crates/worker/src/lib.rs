//! Inspects and cleans PDFs in a separate process (design §3, §5).
//!
//! The app starts its own executable with [`WORKER_FLAG`] to run
//! [`server::run`]; [`client::WorkerProcess`] is the main-process side.

pub mod client;
pub mod protocol;
pub mod server;
#[cfg(windows)]
mod windows_job;

/// The command-line argument that makes the app executable run as a worker
/// (design §5.4).
pub const WORKER_FLAG: &str = "--pdf-worker";

/// The worker's version, returned by [`protocol::Request::Ping`].
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
