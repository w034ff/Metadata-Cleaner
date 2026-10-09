//! IPC commands (design §7.1).

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use mcleaner_worker::WORKER_FLAG;
use mcleaner_worker::client::{INSPECT_TIMEOUT, WorkerError, WorkerProcess};
use mcleaner_worker::protocol::{Request, Response};

/// The worker started by [`check_worker`], kept running so that the owner
/// can end the app from the task manager and see the worker go with it
/// (work-plan T01). Dropped with the app, which ends the worker.
#[derive(Default)]
pub struct CheckedWorker(Mutex<Option<WorkerProcess>>);

/// Answer of [`check_worker`].
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerCheck {
    /// The version the worker answered with, when it answered.
    pub worker_version: Option<String>,
    /// Why the worker could not be started or did not answer.
    pub error: Option<String>,
}

/// Starts a worker from the app's own executable and pings it.
///
/// Until the screens exist (T11, T12), the window shows this result so that
/// an installed build can be checked (work-plan T01).
#[tauri::command]
pub async fn check_worker(app: AppHandle) -> WorkerCheck {
    let result = tauri::async_runtime::spawn_blocking(ping_new_worker).await;
    match result {
        Ok(Ok((worker, version))) => {
            if let Ok(mut slot) = app.state::<CheckedWorker>().0.lock() {
                *slot = Some(worker);
            }
            WorkerCheck {
                worker_version: Some(version),
                error: None,
            }
        }
        Ok(Err(error)) => WorkerCheck {
            worker_version: None,
            error: Some(error),
        },
        Err(join_error) => WorkerCheck {
            worker_version: None,
            error: Some(join_error.to_string()),
        },
    }
}

fn ping_new_worker() -> Result<(WorkerProcess, String), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut worker =
        WorkerProcess::spawn(exe.as_os_str(), [WORKER_FLAG]).map_err(|e| format!("{e:?}"))?;
    match worker.request(&Request::Ping, INSPECT_TIMEOUT) {
        Ok((Response::Pong { version }, _)) => Ok((worker, version)),
        Ok((other, _)) => Err(format!("unexpected answer: {other:?}")),
        Err(WorkerError::Remote { code, detail }) => {
            Err(format!("{code}: {}", detail.unwrap_or_default()))
        }
        Err(e) => Err(format!("{e:?}")),
    }
}
