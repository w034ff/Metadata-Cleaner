//! Batch metadata cleaning execution, state management, and atomic file saving
//! per design §6.3, §6.4, §6.5, §7.

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use mcleaner_core::detect::{self, Format};
use mcleaner_core::image_file;
use mcleaner_core::naming::{resolve_output_names, split_stem_and_ext};
use mcleaner_core::report::MetadataKind;
use mcleaner_worker::protocol::{Request, Response};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::AppState;
use crate::error::{ErrorCode, IpcError};
use crate::worker_pool::default_worker_limit;

/// Name of the event emitted for job progress updates (design §7.2).
pub const JOB_PROGRESS_EVENT: &str = "job-progress";

/// Name of the event emitted when an item completes (design §7.2).
pub const JOB_ITEM_EVENT: &str = "job-item";

/// Name of the event emitted when a job finishes (design §7.2).
pub const JOB_FINISHED_EVENT: &str = "job-finished";

/// Name of a JPEG image that triggers verification failure for testing.
#[cfg(feature = "test-hooks")]
pub const BREAK_VERIFY_FILE_NAME: &str = "break-verify-for-test.jpg";

/// Status of an individual item in a cleaning job (design §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum JobItemStatus {
    Ok,
    Failed,
    Cancelled,
}

/// Progress notification emitted via `job-progress` event (design §7.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct JobProgressPayload {
    /// Total completed items so far.
    #[ts(type = "number")]
    pub done: u32,
    /// Total items to be processed.
    #[ts(type = "number")]
    pub total: u32,
    /// Name of the current file being processed, or None.
    pub current: Option<String>,
}

/// Item completion notification emitted via `job-item` event (design §7.2).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct JobItemPayload {
    #[ts(type = "number")]
    pub id: u64,
    pub status: JobItemStatus,
    /// Output file name if it differs from the original name (design §6.2, §7.2).
    pub saved_name: Option<String>,
    /// Kinds of metadata removed on success; empty otherwise.
    pub removed: Vec<MetadataKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<IpcError>,
}

/// Final summary emitted via `job-finished` event (design §7.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct JobFinishedPayload {
    #[ts(type = "number")]
    pub succeeded: u32,
    #[ts(type = "number")]
    pub failed: u32,
    #[ts(type = "number")]
    pub unprocessed: u32,
    pub cancelled: bool,
}

/// Callbacks for receiving job progress, item results, and completion events.
pub struct JobCallbacks<FProg, FItem, FFin> {
    pub on_progress: FProg,
    pub on_item: FItem,
    pub on_finished: FFin,
}

/// RAII guard to reset the running flag when dropped if not already reset.
pub struct RunningGuard {
    flag: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
}

impl RunningGuard {
    /// Creates a new guard for the given running flag.
    pub fn new(flag: Arc<AtomicBool>) -> Self {
        Self {
            flag,
            active: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Wraps an `on_finished` callback so that `is_running` is reset to `false`
    /// before `on_finished` is invoked, and disarms this guard so that
    /// dropping it later does not reset the flag again.
    pub fn wrap_on_finished<FFin>(
        &self,
        on_finished: FFin,
    ) -> impl FnOnce(JobFinishedPayload) + Send + Sync + 'static
    where
        FFin: FnOnce(JobFinishedPayload) + Send + Sync + 'static,
    {
        let flag = Arc::clone(&self.flag);
        let active = Arc::clone(&self.active);
        move |payload| {
            if active.swap(false, Ordering::SeqCst) {
                flag.store(false, Ordering::SeqCst);
            }
            on_finished(payload);
        }
    }
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        if self.active.swap(false, Ordering::SeqCst) {
            self.flag.store(false, Ordering::SeqCst);
        }
    }
}

/// Lists existing file and directory names directly under `dir` (design §6.4).
pub fn list_existing_files(dir: &Path) -> Result<HashSet<String>, IpcError> {
    let mut set = HashSet::new();
    if !dir.exists() {
        return Ok(set);
    }
    let entries = fs::read_dir(dir).map_err(|_| IpcError::from_code(ErrorCode::WriteFailed))?;
    for entry in entries.flatten() {
        set.insert(entry.file_name().to_string_lossy().to_string());
    }
    Ok(set)
}

/// Mode a converted file is created with before the umask is applied, so a
/// saved file gets the permissions any other new file in the folder gets.
#[cfg(unix)]
const OUTPUT_FILE_MODE: u32 = 0o666;

/// Creates the dot-prefixed temporary file a conversion writes into before
/// renaming it to its final name (design §6.5).
fn create_output_temp(output_dir: &Path) -> std::io::Result<tempfile::NamedTempFile> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(".");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(OUTPUT_FILE_MODE));
    }
    builder.tempfile_in(output_dir)
}

/// Checks that `dir` is a directory and that a temporary file can be created in it (design §6.5).
pub fn check_output_dir(dir: &Path) -> Result<(), IpcError> {
    if !dir.is_dir() || fs::canonicalize(dir).is_err() {
        return Err(IpcError::from_code(ErrorCode::OutputDirMissing));
    }
    create_output_temp(dir)
        .map(drop)
        .map_err(|_| IpcError::from_code(ErrorCode::OutputDirNotWritable))
}

/// Raises `write_failed`, which stops a conversion from taking up its next
/// item, if `err` is a failure to write the output (design §6.5).
fn stop_after_write_failure(err: &IpcError, write_failed: &AtomicBool) -> bool {
    let is_write_failure = err.code == ErrorCode::WriteFailed;
    if is_write_failure {
        write_failed.store(true, Ordering::SeqCst);
    }
    is_write_failure
}

/// Compares two canonical paths for equality. On Windows, comparison is case-insensitive.
fn is_same_path(p1: &Path, p2: &Path) -> bool {
    if cfg!(windows) {
        p1.to_string_lossy().to_lowercase() == p2.to_string_lossy().to_lowercase()
    } else {
        p1 == p2
    }
}

/// Write errors carry no detail: the I/O and tempfile messages name the
/// folder, and the frontend never sees paths (design §1).
///
/// Saves file bytes atomically into `output_dir` using a dot-prefixed temporary file
/// and `persist_noclobber` (design §6.4, §6.5).
///
/// If a collision occurs (`AlreadyExists`), increments the numeric suffix
/// (`stem (1).ext`, `stem (2).ext`, ...) until an unoccupied name is found.
pub fn save_atomic(
    output_dir: &Path,
    initial_output_name: &str,
    bytes: &[u8],
    used_names_lower: &Mutex<HashSet<String>>,
) -> Result<String, IpcError> {
    let mut temp =
        create_output_temp(output_dir).map_err(|_| IpcError::from_code(ErrorCode::WriteFailed))?;

    temp.write_all(bytes)
        .map_err(|_| IpcError::from_code(ErrorCode::WriteFailed))?;
    temp.flush()
        .map_err(|_| IpcError::from_code(ErrorCode::WriteFailed))?;

    let (stem, ext) = split_stem_and_ext(initial_output_name);
    let mut candidate = initial_output_name.to_string();
    let mut suffix_num = 0usize;

    loop {
        let dest_path = output_dir.join(&candidate);
        match temp.persist_noclobber(&dest_path) {
            Ok(_) => {
                let mut used = used_names_lower
                    .lock()
                    .expect("the used names are never locked across a panic");
                used.insert(candidate.to_lowercase());
                return Ok(candidate);
            }
            Err(persist_err) => {
                if persist_err.error.kind() == std::io::ErrorKind::AlreadyExists {
                    temp = persist_err.file;
                    let mut used = used_names_lower
                        .lock()
                        .expect("the used names are never locked across a panic");
                    loop {
                        suffix_num += 1;
                        let next_candidate = if ext.is_empty() {
                            format!("{stem} ({suffix_num})")
                        } else {
                            format!("{stem} ({suffix_num}).{ext}")
                        };
                        let next_lower = next_candidate.to_lowercase();
                        if !used.contains(&next_lower) && !output_dir.join(&next_candidate).exists()
                        {
                            used.insert(next_lower);
                            candidate = next_candidate;
                            break;
                        }
                    }
                } else {
                    return Err(IpcError::from_code(ErrorCode::WriteFailed));
                }
            }
        }
    }
}

struct CleanTask {
    id: u64,
    filename: String,
    path: PathBuf,
    format: Format,
    output_name: String,
}

struct CleanResult {
    saved_name: String,
    removed: Vec<MetadataKind>,
}

fn process_image(
    task: &CleanTask,
    output_dir: &Path,
    used_names: &Mutex<HashSet<String>>,
    write_failed: &AtomicBool,
) -> Result<CleanResult, IpcError> {
    let input_bytes =
        fs::read(&task.path).map_err(|_| IpcError::from_code(ErrorCode::ReadFailed))?;
    detect::check_size(task.format, input_bytes.len() as u64)?;

    let removed = image_file::inspect(&input_bytes)
        .map(|insp| insp.kinds)
        .unwrap_or_default();

    #[cfg(feature = "test-hooks")]
    let cleaned_bytes = if task.filename == BREAK_VERIFY_FILE_NAME {
        input_bytes.clone()
    } else {
        image_file::clean(&input_bytes)?
    };
    #[cfg(not(feature = "test-hooks"))]
    let cleaned_bytes = image_file::clean(&input_bytes)?;

    if !image_file::verify(&input_bytes, &cleaned_bytes) {
        return Err(IpcError::from_code(ErrorCode::VerifyFailed));
    }

    let saved_name = save_atomic(output_dir, &task.output_name, &cleaned_bytes, used_names)
        .inspect_err(|err| {
            stop_after_write_failure(err, write_failed);
        })?;

    let saved_path = output_dir.join(&saved_name);
    let verify_saved =
        matches!(fs::read(&saved_path), Ok(read_bytes) if read_bytes == cleaned_bytes);
    if !verify_saved {
        let _ = fs::remove_file(&saved_path);
        return Err(IpcError::from_code(ErrorCode::VerifyFailed));
    }

    Ok(CleanResult {
        saved_name,
        removed,
    })
}

fn process_pdf(
    task: &CleanTask,
    state: &AppState,
    output_dir: &Path,
    used_names: &Mutex<HashSet<String>>,
    write_failed: &AtomicBool,
) -> Result<CleanResult, IpcError> {
    let (response, cleaned_bytes) = {
        let mut worker = state.pool.acquire()?;
        worker.send(&Request::Clean {
            path: task.path.clone(),
        })?
    };

    let removed = match response {
        Response::Cleaned { removed } => removed,
        Response::Error { code, detail } => {
            return Err(IpcError::from_worker(&code, detail.as_deref()));
        }
        _ => {
            return Err(IpcError::new(
                ErrorCode::WorkerCrashed,
                "unexpected worker response",
            ));
        }
    };

    let saved_name = save_atomic(output_dir, &task.output_name, &cleaned_bytes, used_names)
        .inspect_err(|err| {
            stop_after_write_failure(err, write_failed);
        })?;

    let saved_path = output_dir.join(&saved_name);
    let verify_saved =
        matches!(fs::read(&saved_path), Ok(read_bytes) if read_bytes == cleaned_bytes);
    if !verify_saved {
        let _ = fs::remove_file(&saved_path);
        return Err(IpcError::from_code(ErrorCode::VerifyFailed));
    }

    Ok(CleanResult {
        saved_name,
        removed,
    })
}

fn process_single_item(
    task: &CleanTask,
    state: &AppState,
    output_dir: &Path,
    used_names: &Mutex<HashSet<String>>,
    write_failed: &AtomicBool,
) -> Result<CleanResult, IpcError> {
    match task.format {
        Format::Jpeg | Format::Png | Format::Webp => {
            process_image(task, output_dir, used_names, write_failed)
        }
        Format::Pdf => process_pdf(task, state, output_dir, used_names, write_failed),
    }
}

/// Executes batch cleaning of items and emits events via callbacks (design §6.3–§6.5).
pub fn run_clean<FProg, FItem, FFin>(
    state: &AppState,
    ids: &[u64],
    output_dir: &Path,
    callbacks: JobCallbacks<FProg, FItem, FFin>,
) -> Result<(), IpcError>
where
    FProg: Fn(JobProgressPayload) + Send + Sync + 'static,
    FItem: Fn(JobItemPayload) + Send + Sync + 'static,
    FFin: FnOnce(JobFinishedPayload) + Send + Sync + 'static,
{
    let mut valid_tasks_info = Vec::new();
    for &id in ids {
        let entry = state
            .items
            .get(id)
            .ok_or_else(|| IpcError::from_code(ErrorCode::UnknownHandle))?;
        if entry.error.is_none()
            && let Some(format) = entry.format
        {
            valid_tasks_info.push((id, entry, format));
        }
    }
    if valid_tasks_info.is_empty() {
        return Err(IpcError::from_code(ErrorCode::InvalidParams));
    }

    let output_canonical = fs::canonicalize(output_dir)
        .map_err(|_| IpcError::from_code(ErrorCode::OutputDirMissing))?;

    for (_, entry, _) in &valid_tasks_info {
        if let Some(parent) = entry.path.parent()
            && let Ok(parent_canonical) = fs::canonicalize(parent)
            && is_same_path(&output_canonical, &parent_canonical)
        {
            return Err(IpcError::from_code(ErrorCode::SameFolderAsSource));
        }
    }

    let existing = list_existing_files(output_dir)?;
    let target_names: Vec<String> = valid_tasks_info
        .iter()
        .map(|(_, e, _)| e.name.clone())
        .collect();
    let resolved_names = resolve_output_names(&target_names, &existing);

    let mut initial_used: HashSet<String> = existing.iter().map(|s| s.to_lowercase()).collect();
    for name in &resolved_names {
        initial_used.insert(name.to_lowercase());
    }
    let used_names_lower = Arc::new(Mutex::new(initial_used));

    let tasks: VecDeque<CleanTask> = valid_tasks_info
        .into_iter()
        .zip(resolved_names)
        .map(|((id, entry, format), output_name)| CleanTask {
            id,
            filename: entry.name,
            path: entry.path,
            format,
            output_name,
        })
        .collect();

    let total = tasks.len() as u32;
    (callbacks.on_progress)(JobProgressPayload {
        done: 0,
        total,
        current: None,
    });

    let on_progress = Arc::new(callbacks.on_progress);
    let on_item = Arc::new(callbacks.on_item);
    let on_finished = callbacks.on_finished;

    let queue = Arc::new(Mutex::new(tasks));
    let done = Arc::new(AtomicU32::new(0));
    let succeeded = Arc::new(AtomicU32::new(0));
    let failed = Arc::new(AtomicU32::new(0));
    let unprocessed = Arc::new(AtomicU32::new(0));
    let write_failed = Arc::new(AtomicBool::new(false));

    let num_threads = default_worker_limit().min(total as usize).max(1);
    let mut handles = Vec::with_capacity(num_threads);
    let output_dir_buf = output_dir.to_path_buf();

    for _ in 0..num_threads {
        let queue = Arc::clone(&queue);
        let done = Arc::clone(&done);
        let succeeded = Arc::clone(&succeeded);
        let failed = Arc::clone(&failed);
        let unprocessed = Arc::clone(&unprocessed);
        let cancel_flag = Arc::clone(&state.cancel_flag);
        let write_failed = Arc::clone(&write_failed);
        let used_names = Arc::clone(&used_names_lower);
        let output_dir = output_dir_buf.clone();
        let on_progress = Arc::clone(&on_progress);
        let on_item = Arc::clone(&on_item);
        let state_clone = state.clone();

        handles.push(std::thread::spawn(move || {
            loop {
                if cancel_flag.load(Ordering::SeqCst) {
                    let mut q = match queue.lock() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    let remaining: Vec<CleanTask> = q.drain(..).collect();
                    drop(q);
                    for task in remaining {
                        unprocessed.fetch_add(1, Ordering::SeqCst);
                        on_item(JobItemPayload {
                            id: task.id,
                            status: JobItemStatus::Cancelled,
                            saved_name: None,
                            removed: Vec::new(),
                            error: None,
                        });
                    }
                    break;
                }

                if write_failed.load(Ordering::SeqCst) {
                    let mut q = match queue.lock() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    unprocessed.fetch_add(q.len() as u32, Ordering::SeqCst);
                    q.clear();
                    break;
                }

                let task = {
                    let mut q = match queue.lock() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    q.pop_front()
                };

                let Some(task) = task else {
                    break;
                };

                on_progress(JobProgressPayload {
                    done: done.load(Ordering::SeqCst),
                    total,
                    current: Some(task.filename.clone()),
                });

                let result = process_single_item(
                    &task,
                    &state_clone,
                    &output_dir,
                    &used_names,
                    &write_failed,
                );

                match result {
                    Ok(clean_result) => {
                        succeeded.fetch_add(1, Ordering::SeqCst);
                        let current_done = done.fetch_add(1, Ordering::SeqCst) + 1;
                        let saved_name = if clean_result.saved_name != task.filename {
                            Some(clean_result.saved_name)
                        } else {
                            None
                        };
                        on_item(JobItemPayload {
                            id: task.id,
                            status: JobItemStatus::Ok,
                            saved_name,
                            removed: clean_result.removed,
                            error: None,
                        });
                        on_progress(JobProgressPayload {
                            done: current_done,
                            total,
                            current: None,
                        });
                    }
                    Err(err) => {
                        failed.fetch_add(1, Ordering::SeqCst);
                        let current_done = done.fetch_add(1, Ordering::SeqCst) + 1;
                        on_item(JobItemPayload {
                            id: task.id,
                            status: JobItemStatus::Failed,
                            saved_name: None,
                            removed: Vec::new(),
                            error: Some(err),
                        });
                        on_progress(JobProgressPayload {
                            done: current_done,
                            total,
                            current: None,
                        });
                    }
                }
            }
        }));
    }

    for handle in handles {
        let _ = handle.join();
    }

    let cur_succeeded = succeeded.load(Ordering::SeqCst);
    let cur_failed = failed.load(Ordering::SeqCst);
    let cur_unprocessed = unprocessed.load(Ordering::SeqCst);
    let accounted = cur_succeeded + cur_failed + cur_unprocessed;
    if accounted < total {
        failed.fetch_add(total - accounted, Ordering::SeqCst);
    }

    on_finished(JobFinishedPayload {
        succeeded: succeeded.load(Ordering::SeqCst),
        failed: failed.load(Ordering::SeqCst),
        unprocessed: unprocessed.load(Ordering::SeqCst),
        cancelled: state.cancel_flag.load(Ordering::SeqCst),
    });

    Ok(())
}

/// Prepares and starts batch cleaning in the background, validating state and parameters.
pub fn start_clean_internal<FProg, FItem, FFin>(
    state: &AppState,
    ids: &[u64],
    callbacks: JobCallbacks<FProg, FItem, FFin>,
) -> Result<(), IpcError>
where
    FProg: Fn(JobProgressPayload) + Send + Sync + 'static,
    FItem: Fn(JobItemPayload) + Send + Sync + 'static,
    FFin: FnOnce(JobFinishedPayload) + Send + Sync + 'static,
{
    // The checks run in the order of design §6.3 and §6.5; any failure
    // lowers the running flag again and starts nothing.
    if state
        .is_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(IpcError::from_code(ErrorCode::JobRunning));
    }
    state.cancel_flag.store(false, Ordering::SeqCst);

    let output_dir = {
        let lock = state
            .output_dir
            .lock()
            .expect("the output folder is never locked across a panic");
        match lock.clone() {
            Some(dir) => dir,
            None => {
                state.is_running.store(false, Ordering::SeqCst);
                return Err(IpcError::from_code(ErrorCode::InvalidParams));
            }
        }
    };

    if let Err(err) = check_output_dir(&output_dir) {
        state.is_running.store(false, Ordering::SeqCst);
        return Err(err);
    }

    for &id in ids {
        if state.items.get(id).is_none() {
            state.is_running.store(false, Ordering::SeqCst);
            return Err(IpcError::from_code(ErrorCode::UnknownHandle));
        }
    }

    // Items that failed when added are not cleaned (design §6.3).
    let valid_count = ids
        .iter()
        .filter_map(|&id| state.items.get(id))
        .filter(|entry| entry.error.is_none())
        .count();
    if valid_count == 0 {
        state.is_running.store(false, Ordering::SeqCst);
        return Err(IpcError::from_code(ErrorCode::InvalidParams));
    }

    let output_canonical = match fs::canonicalize(&output_dir) {
        Ok(c) => c,
        Err(_) => {
            state.is_running.store(false, Ordering::SeqCst);
            return Err(IpcError::from_code(ErrorCode::WriteFailed));
        }
    };
    for &id in ids {
        if let Some(entry) = state.items.get(id)
            && entry.error.is_none()
            && let Some(parent) = entry.path.parent()
            && let Ok(parent_canonical) = fs::canonicalize(parent)
            && is_same_path(&output_canonical, &parent_canonical)
        {
            state.is_running.store(false, Ordering::SeqCst);
            return Err(IpcError::from_code(ErrorCode::SameFolderAsSource));
        }
    }

    if let Err(err) = list_existing_files(&output_dir) {
        state.is_running.store(false, Ordering::SeqCst);
        return Err(err);
    }

    let state_clone = state.clone();
    let ids_owned = ids.to_vec();

    std::thread::spawn(move || {
        let running_guard = RunningGuard::new(Arc::clone(&state_clone.is_running));
        let on_finished_cell = Arc::new(Mutex::new(Some(
            running_guard.wrap_on_finished(callbacks.on_finished),
        )));
        let on_finished_for_run = {
            let cell = Arc::clone(&on_finished_cell);
            move |finished: JobFinishedPayload| {
                if let Ok(mut lock) = cell.lock()
                    && let Some(cb) = lock.take()
                {
                    cb(finished);
                }
            }
        };
        let wrapped_callbacks = JobCallbacks {
            on_progress: callbacks.on_progress,
            on_item: callbacks.on_item,
            on_finished: on_finished_for_run,
        };
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_clean(&state_clone, &ids_owned, &output_dir, wrapped_callbacks)
        }));
        if (res.is_err() || matches!(res, Ok(Err(_))))
            && let Ok(mut lock) = on_finished_cell.lock()
            && let Some(cb) = lock.take()
        {
            cb(JobFinishedPayload {
                succeeded: 0,
                failed: ids_owned.len() as u32,
                unprocessed: 0,
                cancelled: state_clone.cancel_flag.load(Ordering::SeqCst),
            });
        }
    });

    Ok(())
}
