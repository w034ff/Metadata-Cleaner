//! Integration tests for metadata cleaning execution, jobs, and atomic saving
//! (work-plan T09, design §6.3–§6.5, §7).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use mcleaner_core::image_file;
use mcleaner_worker::WORKER_FLAG;
use mcleaner_worker::pdf;
#[cfg(feature = "test-hooks")]
use mcleaner_worker::server::CRASH_ON_OPEN_FILE_NAME;
use metadata_cleaner_lib::AppState;
use metadata_cleaner_lib::error::ErrorCode;
use metadata_cleaner_lib::items::{add_files, details, remove_items};
#[cfg(feature = "test-hooks")]
use metadata_cleaner_lib::jobs::BREAK_VERIFY_FILE_NAME;
use metadata_cleaner_lib::jobs::{
    JobCallbacks, JobFinishedPayload, JobItemPayload, JobItemStatus, JobProgressPayload,
    list_existing_files, run_clean, save_atomic, start_clean_internal,
};
use metadata_cleaner_lib::worker_pool::{WorkerPool, WorkerPoolConfig};
use tempfile::TempDir;

fn state() -> AppState {
    let config = WorkerPoolConfig::new(env!("CARGO_BIN_EXE_metadata-cleaner"), [WORKER_FLAG])
        .with_max_workers(2)
        .with_inspect_timeout(Duration::from_secs(30))
        .with_clean_timeout(Duration::from_secs(60));
    AppState::new(WorkerPool::new(config))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/core/tests/fixtures")
        .join(name)
}

fn copy_fixture(dir: &Path, name: &str, as_name: &str) -> PathBuf {
    let dest = dir.join(as_name);
    fs::copy(fixture(name), &dest).expect("copying fixture");
    dest
}

/// Collects callback notifications into thread-safe vectors for inspection.
struct TestRecorder {
    progress: Arc<Mutex<Vec<JobProgressPayload>>>,
    items: Arc<Mutex<Vec<JobItemPayload>>>,
    finished: Arc<Mutex<Option<JobFinishedPayload>>>,
    is_running_at_finished: Arc<Mutex<Option<bool>>>,
}

impl TestRecorder {
    fn new() -> Self {
        Self {
            progress: Arc::new(Mutex::new(Vec::new())),
            items: Arc::new(Mutex::new(Vec::new())),
            finished: Arc::new(Mutex::new(None)),
            is_running_at_finished: Arc::new(Mutex::new(None)),
        }
    }

    fn callbacks(
        &self,
    ) -> JobCallbacks<
        impl Fn(JobProgressPayload) + Send + Sync + 'static,
        impl Fn(JobItemPayload) + Send + Sync + 'static,
        impl FnOnce(JobFinishedPayload) + Send + Sync + 'static,
    > {
        let p = Arc::clone(&self.progress);
        let i = Arc::clone(&self.items);
        let f = Arc::clone(&self.finished);
        JobCallbacks {
            on_progress: move |prog| p.lock().unwrap().push(prog),
            on_item: move |item| i.lock().unwrap().push(item),
            on_finished: move |fin| *f.lock().unwrap() = Some(fin),
        }
    }

    fn callbacks_recording_running(
        &self,
        is_running: Arc<AtomicBool>,
    ) -> JobCallbacks<
        impl Fn(JobProgressPayload) + Send + Sync + 'static,
        impl Fn(JobItemPayload) + Send + Sync + 'static,
        impl FnOnce(JobFinishedPayload) + Send + Sync + 'static,
    > {
        let p = Arc::clone(&self.progress);
        let i = Arc::clone(&self.items);
        let f = Arc::clone(&self.finished);
        let r = Arc::clone(&self.is_running_at_finished);
        JobCallbacks {
            on_progress: move |prog| p.lock().unwrap().push(prog),
            on_item: move |item| i.lock().unwrap().push(item),
            on_finished: move |fin| {
                *r.lock().unwrap() = Some(is_running.load(Ordering::SeqCst));
                *f.lock().unwrap() = Some(fin);
            },
        }
    }

    fn items(&self) -> Vec<JobItemPayload> {
        self.items.lock().unwrap().clone()
    }

    fn finished(&self) -> JobFinishedPayload {
        self.finished
            .lock()
            .unwrap()
            .clone()
            .expect("finished event received")
    }

    fn is_running_at_finished(&self) -> Option<bool> {
        *self.is_running_at_finished.lock().unwrap()
    }

    fn wait_finished(&self, timeout: Duration) -> JobFinishedPayload {
        let start = std::time::Instant::now();
        loop {
            if let Some(f) = self.finished.lock().unwrap().clone() {
                return f;
            }
            if start.elapsed() > timeout {
                panic!("timed out waiting for finished payload");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Asserts that no temporary files starting with '.' remain in `dir`.
fn assert_no_temp_files(dir: &Path) {
    for entry in fs::read_dir(dir).expect("reading dir").flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        assert!(
            !name_str.starts_with('.'),
            "unexpected temporary file found: {name_str}"
        );
    }
}

#[test]
fn mixed_images_and_pdf_with_corrupt_file() {
    let app_state = state();
    let temp_in1 = TempDir::new().unwrap();
    let temp_in2 = TempDir::new().unwrap();
    let temp_out = TempDir::new().unwrap();

    let jpg_path = copy_fixture(temp_in1.path(), "full.jpg", "full.jpg");
    let png_path = copy_fixture(temp_in1.path(), "full.png", "full.png");
    let webp_path = copy_fixture(temp_in2.path(), "full.webp", "full.webp");
    let pdf_path = copy_fixture(temp_in2.path(), "full.pdf", "full.pdf");
    let corrupt_path = copy_fixture(temp_in2.path(), "corrupt.jpg", "corrupt.jpg");

    let add_res = add_files(
        &app_state,
        &[jpg_path, png_path, webp_path, pdf_path, corrupt_path],
    )
    .unwrap();
    assert_eq!(add_res.added.len(), 5);

    let corrupt_item = add_res
        .added
        .iter()
        .find(|i| i.name == "corrupt.jpg")
        .unwrap();
    assert!(corrupt_item.error.is_some());

    let all_ids: Vec<u64> = add_res.added.iter().map(|item| item.id).collect();

    let recorder = TestRecorder::new();
    run_clean(&app_state, &all_ids, temp_out.path(), recorder.callbacks())
        .expect("run_clean should succeed");

    let finished = recorder.finished();
    assert_eq!(finished.succeeded, 4);
    assert_eq!(finished.failed, 0);
    assert_eq!(finished.unprocessed, 0);
    assert!(!finished.cancelled);

    let items = recorder.items();
    assert_eq!(items.len(), 4);

    for item in &items {
        assert_eq!(item.status, JobItemStatus::Ok);
        assert_eq!(item.saved_name, None);
        assert!(item.error.is_none());

        // Verify that removed matches original inspection kinds
        let original = add_res.added.iter().find(|i| i.id == item.id).unwrap();
        assert_eq!(item.removed, original.kinds);
    }

    // Verify cleaned files have empty kinds when inspected
    let saved_jpg = fs::read(temp_out.path().join("full.jpg")).unwrap();
    assert!(image_file::inspect(&saved_jpg).unwrap().kinds.is_empty());

    let saved_png = fs::read(temp_out.path().join("full.png")).unwrap();
    assert!(image_file::inspect(&saved_png).unwrap().kinds.is_empty());

    let saved_webp = fs::read(temp_out.path().join("full.webp")).unwrap();
    assert!(image_file::inspect(&saved_webp).unwrap().kinds.is_empty());

    let saved_pdf = fs::read(temp_out.path().join("full.pdf")).unwrap();
    assert!(pdf::inspect(&saved_pdf).unwrap().kinds.is_empty());

    assert_no_temp_files(temp_out.path());
}

#[test]
fn same_folder_as_source_rejects() {
    let app_state = state();
    let temp_dir = TempDir::new().unwrap();

    let path = copy_fixture(temp_dir.path(), "full.jpg", "full.jpg");
    let add_res = add_files(&app_state, &[path]).unwrap();
    let id = add_res.added[0].id;

    *app_state.output_dir.lock().unwrap() = Some(temp_dir.path().to_path_buf());

    let recorder = TestRecorder::new();
    let err = start_clean_internal(&app_state, &[id], recorder.callbacks()).unwrap_err();
    assert_eq!(err.code, ErrorCode::SameFolderAsSource);
    assert!(!app_state.is_running.load(Ordering::SeqCst));
}

#[cfg(windows)]
#[test]
fn same_folder_as_source_case_insensitive_windows() {
    let app_state = state();
    let temp_dir = TempDir::new().unwrap();

    let path = copy_fixture(temp_dir.path(), "full.jpg", "full.jpg");
    let add_res = add_files(&app_state, &[path]).unwrap();
    let id = add_res.added[0].id;

    let path_str = temp_dir.path().to_string_lossy();
    let alt_case_path = PathBuf::from(path_str.to_uppercase());
    *app_state.output_dir.lock().unwrap() = Some(alt_case_path);

    let recorder = TestRecorder::new();
    let err = start_clean_internal(&app_state, &[id], recorder.callbacks()).unwrap_err();
    assert_eq!(err.code, ErrorCode::SameFolderAsSource);
    assert!(!app_state.is_running.load(Ordering::SeqCst));
}

#[cfg(feature = "test-hooks")]
#[test]
fn verify_failed_breaks_and_cleans_up() {
    let app_state = state();
    let temp_in = TempDir::new().unwrap();
    let temp_out = TempDir::new().unwrap();

    let path = copy_fixture(temp_in.path(), "full.jpg", BREAK_VERIFY_FILE_NAME);
    let add_res = add_files(&app_state, &[path]).unwrap();
    let id = add_res.added[0].id;

    let recorder = TestRecorder::new();
    run_clean(&app_state, &[id], temp_out.path(), recorder.callbacks()).unwrap();

    let finished = recorder.finished();
    assert_eq!(finished.succeeded, 0);
    assert_eq!(finished.failed, 1);

    let items = recorder.items();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].status, JobItemStatus::Failed);
    assert_eq!(
        items[0].error.as_ref().map(|e| e.code),
        Some(ErrorCode::VerifyFailed)
    );

    // Verify neither output file nor temp file exists
    assert!(!temp_out.path().join(BREAK_VERIFY_FILE_NAME).exists());
    assert_no_temp_files(temp_out.path());
}

#[test]
fn naming_collision_and_noclobber() {
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();

    // Pre-create file in output directory
    let existing_file = out_dir.join("photo.jpg");
    fs::write(&existing_file, b"existing content").unwrap();

    let used_names = Mutex::new(HashSet::new());

    // First save: photo.jpg exists on disk -> should save as photo (1).jpg
    let saved1 = save_atomic(out_dir, "photo.jpg", b"image 1", &used_names).unwrap();
    assert_eq!(saved1, "photo (1).jpg");
    assert_eq!(fs::read(out_dir.join("photo (1).jpg")).unwrap(), b"image 1");
    // Existing file was not clobbered
    assert_eq!(fs::read(&existing_file).unwrap(), b"existing content");

    // Second save: both photo.jpg and photo (1).jpg are taken -> should save as photo (2).jpg
    let saved2 = save_atomic(out_dir, "photo.jpg", b"image 2", &used_names).unwrap();
    assert_eq!(saved2, "photo (2).jpg");
    assert_eq!(fs::read(out_dir.join("photo (2).jpg")).unwrap(), b"image 2");

    // Collision at the moment of persist: pre-create photo (3).jpg behind our back
    fs::write(out_dir.join("photo (3).jpg"), b"third party").unwrap();
    let saved3 = save_atomic(out_dir, "photo (3).jpg", b"image 3", &used_names).unwrap();
    assert_eq!(saved3, "photo (3) (1).jpg");
    assert_eq!(
        fs::read(out_dir.join("photo (3).jpg")).unwrap(),
        b"third party"
    );
    assert_eq!(
        fs::read(out_dir.join("photo (3) (1).jpg")).unwrap(),
        b"image 3"
    );

    assert_no_temp_files(out_dir);
}

#[test]
fn saved_name_only_reported_when_different() {
    let app_state = state();
    let temp_in = TempDir::new().unwrap();
    let temp_out = TempDir::new().unwrap();

    // Pre-create full.jpg in output directory
    fs::write(temp_out.path().join("full.jpg"), b"existing").unwrap();

    let path_jpg = copy_fixture(temp_in.path(), "full.jpg", "full.jpg");
    let path_png = copy_fixture(temp_in.path(), "full.png", "full.png");

    let add_res = add_files(&app_state, &[path_jpg, path_png]).unwrap();
    let ids: Vec<u64> = add_res.added.iter().map(|i| i.id).collect();

    let recorder = TestRecorder::new();
    run_clean(&app_state, &ids, temp_out.path(), recorder.callbacks()).unwrap();

    let items = recorder.items();
    let jpg_item = items.iter().find(|i| i.id == ids[0]).unwrap();
    let png_item = items.iter().find(|i| i.id == ids[1]).unwrap();

    // full.jpg had collision -> saved as full (1).jpg, saved_name is Some
    assert_eq!(jpg_item.saved_name, Some("full (1).jpg".to_string()));
    // full.png had no collision -> saved_name is None
    assert_eq!(png_item.saved_name, None);
}

#[test]
fn cancellation_stops_next_and_leaves_no_temp_files() {
    let app_state = state();
    let temp_in = TempDir::new().unwrap();
    let temp_out = TempDir::new().unwrap();

    let mut paths = Vec::new();
    for i in 0..8 {
        paths.push(copy_fixture(
            temp_in.path(),
            "full.jpg",
            &format!("img_{i}.jpg"),
        ));
    }

    let add_res = add_files(&app_state, &paths).unwrap();
    let ids: Vec<u64> = add_res.added.iter().map(|i| i.id).collect();

    // Pre-set cancel flag before running
    app_state.cancel_flag.store(true, Ordering::SeqCst);

    let recorder = TestRecorder::new();
    run_clean(&app_state, &ids, temp_out.path(), recorder.callbacks()).unwrap();

    let finished = recorder.finished();
    assert!(finished.cancelled);
    assert_eq!(finished.succeeded, 0);
    assert_eq!(finished.unprocessed, 8);

    let items = recorder.items();
    assert_eq!(items.len(), 8);
    for item in items {
        assert_eq!(item.status, JobItemStatus::Cancelled);
    }

    assert_no_temp_files(temp_out.path());
    let files = list_existing_files(temp_out.path()).unwrap();
    assert!(files.is_empty());
}

#[test]
fn job_running_rejects_commands_and_allows_details() {
    let app_state = state();
    let temp_in = TempDir::new().unwrap();
    let temp_out = TempDir::new().unwrap();

    let path = copy_fixture(temp_in.path(), "full.jpg", "full.jpg");
    let add_res = add_files(&app_state, std::slice::from_ref(&path)).unwrap();
    let id = add_res.added[0].id;

    *app_state.output_dir.lock().unwrap() = Some(temp_out.path().to_path_buf());

    // Flag job as running
    app_state.is_running.store(true, Ordering::SeqCst);

    // add_files rejected
    let err_add = add_files(&app_state, &[path]).unwrap_err();
    assert_eq!(err_add.code, ErrorCode::JobRunning);

    // remove_items rejected
    let err_remove = remove_items(&app_state, &[id]).unwrap_err();
    assert_eq!(err_remove.code, ErrorCode::JobRunning);

    // start_clean_internal rejected
    let recorder = TestRecorder::new();
    let err_start = start_clean_internal(&app_state, &[id], recorder.callbacks()).unwrap_err();
    assert_eq!(err_start.code, ErrorCode::JobRunning);

    // details is allowed even while job is running
    let det = details(&app_state, id).expect("details should succeed while job is running");
    assert!(!det.groups.is_empty());

    // JobRunning takes precedence even if output_dir is None
    *app_state.output_dir.lock().unwrap() = None;
    let recorder = TestRecorder::new();
    let err_start_none = start_clean_internal(&app_state, &[id], recorder.callbacks()).unwrap_err();
    assert_eq!(err_start_none.code, ErrorCode::JobRunning);
}

#[cfg(feature = "test-hooks")]
#[test]
fn worker_crashed_isolated_and_continues_batch() {
    let app_state = state();
    let temp_in = TempDir::new().unwrap();
    let temp_out = TempDir::new().unwrap();

    let crash_path = copy_fixture(temp_in.path(), "full.pdf", CRASH_ON_OPEN_FILE_NAME);
    let valid_path = copy_fixture(temp_in.path(), "full.png", "full.png");

    let add_res = add_files(&app_state, &[valid_path]).unwrap();
    let valid_id = add_res.added[0].id;

    let canonical = fs::canonicalize(&crash_path).unwrap();
    let bytes = fs::metadata(&crash_path).unwrap().len();
    let crash_id = app_state
        .items
        .insert(metadata_cleaner_lib::items::Entry {
            path: crash_path.clone(),
            canonical,
            name: CRASH_ON_OPEN_FILE_NAME.to_string(),
            bytes,
            format: Some(mcleaner_core::detect::Format::Pdf),
            error: None,
        })
        .unwrap();

    let recorder = TestRecorder::new();
    run_clean(
        &app_state,
        &[crash_id, valid_id],
        temp_out.path(),
        recorder.callbacks(),
    )
    .unwrap();

    let finished = recorder.finished();
    assert_eq!(finished.succeeded, 1);
    assert_eq!(finished.failed, 1);

    let items = recorder.items();
    let crash_item = items.iter().find(|i| i.id == crash_id).unwrap();
    assert_eq!(crash_item.status, JobItemStatus::Failed);
    assert_eq!(
        crash_item.error.as_ref().map(|e| e.code),
        Some(ErrorCode::WorkerCrashed)
    );

    let ok_item = items.iter().find(|i| i.id == valid_id).unwrap();
    assert_eq!(ok_item.status, JobItemStatus::Ok);
    assert!(temp_out.path().join("full.png").exists());

    assert_no_temp_files(temp_out.path());
}

#[test]
fn file_modified_time_not_copied() {
    let app_state = state();
    let temp_in = TempDir::new().unwrap();
    let temp_out = TempDir::new().unwrap();

    let path = copy_fixture(temp_in.path(), "full.jpg", "full.jpg");

    // Set input file modified time to 1 year ago
    let one_year_ago = SystemTime::now() - Duration::from_secs(365 * 24 * 3600);
    let file = fs::File::open(&path).unwrap();
    file.set_modified(one_year_ago).unwrap();
    drop(file);

    let add_res = add_files(&app_state, &[path]).unwrap();
    let id = add_res.added[0].id;

    let recorder = TestRecorder::new();
    run_clean(&app_state, &[id], temp_out.path(), recorder.callbacks()).unwrap();

    let saved_path = temp_out.path().join("full.jpg");
    assert!(saved_path.exists());

    let saved_modified = fs::metadata(&saved_path).unwrap().modified().unwrap();

    // Verify saved file modified time is NOT one year ago, but close to now
    let diff_from_orig = saved_modified
        .duration_since(one_year_ago)
        .unwrap_or_default();
    assert!(diff_from_orig.as_secs() > 300 * 24 * 3600);

    let elapsed_since_saved = SystemTime::now()
        .duration_since(saved_modified)
        .unwrap_or_default();
    assert!(elapsed_since_saved.as_secs() < 60);
}

#[test]
fn start_clean_internal_full_lifecycle() {
    let app_state = state();
    let temp_in = TempDir::new().unwrap();
    let temp_out = TempDir::new().unwrap();

    let path = copy_fixture(temp_in.path(), "full.jpg", "full.jpg");
    let add_res = add_files(&app_state, &[path]).unwrap();
    let id = add_res.added[0].id;

    *app_state.output_dir.lock().unwrap() = Some(temp_out.path().to_path_buf());

    let recorder = TestRecorder::new();
    start_clean_internal(
        &app_state,
        &[id],
        recorder.callbacks_recording_running(Arc::clone(&app_state.is_running)),
    )
    .expect("start_clean_internal should succeed");

    let finished = recorder.wait_finished(Duration::from_secs(10));
    assert_eq!(finished.succeeded, 1);
    assert_eq!(finished.failed, 0);

    // Verify is_running was reset to false before on_finished
    assert_eq!(recorder.is_running_at_finished(), Some(false));
    assert!(!app_state.is_running.load(Ordering::SeqCst));
}
