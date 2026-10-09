//! A standalone worker, used by the tests to start a worker without the app.

use std::process::ExitCode;

fn main() -> ExitCode {
    match mcleaner_worker::server::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("worker failed: {e}");
            ExitCode::FAILURE
        }
    }
}
