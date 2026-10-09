// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    // The app starts itself with this flag to get a worker process
    // (design §5.4), so the worker must be handled before Tauri starts.
    if std::env::args_os()
        .nth(1)
        .is_some_and(|flag| flag == mcleaner_worker::WORKER_FLAG)
    {
        return match mcleaner_worker::server::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    metadata_cleaner_lib::run();
    ExitCode::SUCCESS
}
