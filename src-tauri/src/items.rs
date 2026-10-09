//! The list of files: the table of IDs, adding files and folders,
//! sorting what is dropped, and details (design §6.1, §6.2, §7.1).
//!
//! Paths stay in this table; the frontend only sees IDs (design §1).

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use mcleaner_core::detect::{self, DETECT_HEAD_BYTES, Format, SUPPORTED_EXTENSIONS};
use mcleaner_core::image_file;
use mcleaner_core::report::{Details, MetadataKind};
use mcleaner_worker::protocol::{Request, Response};
use serde::Serialize;
use ts_rs::TS;

use crate::AppState;
use crate::error::{ErrorCode, IpcError};

/// The first character of the name of a hidden file (design §6.1).
const HIDDEN_PREFIX: char = '.';

/// Whether `path` has a supported extension (case-insensitive).
pub fn is_supported_extension(path: &Path) -> bool {
    path.extension().and_then(OsStr::to_str).is_some_and(|ext| {
        let lower = ext.to_lowercase();
        SUPPORTED_EXTENSIONS.contains(&lower.as_str())
    })
}

/// What the table keeps for one item (design §6.1).
#[derive(Debug, Clone)]
pub struct Entry {
    /// The path as it was chosen; files are read through it.
    pub path: PathBuf,
    /// The normalized path, which decides whether two items are the same file.
    pub canonical: PathBuf,
    /// The file name without folder.
    pub name: String,
    /// Size of the file in bytes.
    pub bytes: u64,
    /// Detected format of the file, if detected successfully.
    pub format: Option<Format>,
    /// Why the file could not be read, if it could not.
    pub error: Option<IpcError>,
}

#[derive(Debug, Default)]
struct Table {
    entries: HashMap<u64, Entry>,
    by_canonical: HashMap<PathBuf, u64>,
}

/// The table that maps IDs to files (design §1, §6.1). An ID is a number that
/// only grows, so the ID of a removed item is never given to another file.
#[derive(Debug, Default)]
pub struct ItemTable {
    table: Mutex<Table>,
    last_id: AtomicU64,
}

impl ItemTable {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Table> {
        self.table
            .lock()
            .expect("the item table is never locked across a panic")
    }

    /// Whether a file with this normalized path is in the list already.
    pub fn contains(&self, canonical: &Path) -> bool {
        self.lock().by_canonical.contains_key(canonical)
    }

    /// Adds `entry` and returns its ID, or `None` if the same file is in the
    /// list already (so two additions at once cannot add a file twice).
    pub fn insert(&self, entry: Entry) -> Option<u64> {
        let mut table = self.lock();
        if table.by_canonical.contains_key(&entry.canonical) {
            return None;
        }
        let id = self.last_id.fetch_add(1, Ordering::Relaxed) + 1;
        table.by_canonical.insert(entry.canonical.clone(), id);
        table.entries.insert(id, entry);
        Some(id)
    }

    pub fn get(&self, id: u64) -> Option<Entry> {
        self.lock().entries.get(&id).cloned()
    }

    /// Removes the items with these IDs; an ID that is not in the table is ignored.
    pub fn remove(&self, ids: &[u64]) {
        let mut table = self.lock();
        for id in ids {
            if let Some(entry) = table.entries.remove(id) {
                table.by_canonical.remove(&entry.canonical);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.lock().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// An item in the list (design §6.1, §7.1). When `error` is set, the file could not
/// be read: `format` is null and `kinds` is empty.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileItem {
    #[ts(type = "number")]
    pub id: u64,
    /// The file name, without the folder.
    pub name: String,
    pub format: Option<Format>,
    /// Size of the file in bytes.
    #[ts(type = "number")]
    pub bytes: u64,
    pub kinds: Vec<MetadataKind>,
    pub error: Option<IpcError>,
}

/// What was left out of an addition (design §6.1, §7.1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    /// Files whose extension the list does not take.
    pub unsupported: u32,
    /// Subfolders of a folder that was added.
    pub folders: u32,
    /// Files that are in the list already.
    pub duplicates: u32,
}

impl Skipped {
    pub fn merged(self, other: Self) -> Self {
        Self {
            unsupported: self.unsupported + other.unsupported,
            folders: self.folders + other.folders,
            duplicates: self.duplicates + other.duplicates,
        }
    }
}

/// The answer of `add_files` (design §7.1).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AddResult {
    pub added: Vec<FileItem>,
    pub skipped: Skipped,
}

/// The payload of the `items-dropped` event (design §7.2).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ItemsDropped {
    pub added: Vec<FileItem>,
    pub skipped: Skipped,
    pub error: Option<IpcError>,
}

/// What a folder holds (design §6.1).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FolderScan {
    pub files: Vec<PathBuf>,
    /// Files with an unsupported extension.
    pub unsupported: u32,
    /// Subfolders.
    pub folders: u32,
}

/// Looks at the entries directly in `dir` (design §6.1): hidden entries and
/// symbolic links are left out without being counted, subfolders and files of
/// an unsupported extension are counted, and the files that are left are sorted by
/// name so that the order of a list does not depend on the file system.
///
/// # Errors
///
/// Returns the I/O error when `dir` cannot be read.
pub fn scan_folder(dir: &Path) -> io::Result<FolderScan> {
    let mut scan = FolderScan::default();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if is_hidden(&entry.file_name()) {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            scan.folders += 1;
        } else if file_type.is_file() {
            if is_supported_extension(&entry.path()) {
                scan.files.push(entry.path());
            } else {
                scan.unsupported += 1;
            }
        }
    }
    scan.files.sort_by_key(|path| sort_key(path));
    Ok(scan)
}

fn is_hidden(name: &OsStr) -> bool {
    name.to_string_lossy().starts_with(HIDDEN_PREFIX)
}

fn sort_key(path: &Path) -> (String, PathBuf) {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    (name.to_lowercase(), path.to_path_buf())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Inspects a file in the order defined in design §4.1, §4.5 and work-plan T08.
/// Returns `(format, bytes, kinds, error)`.
fn inspect_file(
    state: &AppState,
    path: &Path,
) -> (Option<Format>, u64, Vec<MetadataKind>, Option<IpcError>) {
    // 1. File metadata and size
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => {
            return (
                None,
                0,
                Vec::new(),
                Some(IpcError::from_code(ErrorCode::ReadFailed)),
            );
        }
    };
    let bytes = metadata.len();

    // 2. Format detection from header
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => {
            return (
                None,
                bytes,
                Vec::new(),
                Some(IpcError::from_code(ErrorCode::ReadFailed)),
            );
        }
    };
    // A single read may return fewer bytes than asked for; read_to_end through
    // take() fills the head unless the file is shorter.
    let mut head = Vec::with_capacity(DETECT_HEAD_BYTES);
    match (&mut file)
        .take(DETECT_HEAD_BYTES as u64)
        .read_to_end(&mut head)
    {
        Ok(_) => {}
        Err(_) => {
            return (
                None,
                bytes,
                Vec::new(),
                Some(IpcError::from_code(ErrorCode::ReadFailed)),
            );
        }
    };
    let format = match detect::detect(&head) {
        Ok(f) => f,
        Err(e) => return (None, bytes, Vec::new(), Some(IpcError::from(e))),
    };

    // 3. Size limit check
    if let Err(e) = detect::check_size(format, bytes) {
        return (None, bytes, Vec::new(), Some(IpcError::from(e)));
    }

    // 4 & 5. Inspect metadata (images in-process, PDF in worker)
    match format {
        Format::Jpeg | Format::Png | Format::Webp => {
            let content = match fs::read(path) {
                Ok(c) => c,
                Err(_) => {
                    return (
                        None,
                        bytes,
                        Vec::new(),
                        Some(IpcError::from_code(ErrorCode::ReadFailed)),
                    );
                }
            };
            match image_file::inspect(&content) {
                Ok(inspection) => (Some(format), bytes, inspection.kinds, None),
                Err(e) => (None, bytes, Vec::new(), Some(IpcError::from(e))),
            }
        }
        Format::Pdf => {
            let worker_result = (|| -> Result<Vec<MetadataKind>, IpcError> {
                let mut worker = state.pool.acquire()?;
                let (response, _) = worker.send(&Request::Inspect {
                    path: path.to_path_buf(),
                    details: false,
                })?;
                match response {
                    Response::Inspected { inspection, .. } => Ok(inspection.kinds),
                    _ => Err(IpcError::new(
                        ErrorCode::WorkerCrashed,
                        "unexpected worker response",
                    )),
                }
            })();
            match worker_result {
                Ok(kinds) => (Some(Format::Pdf), bytes, kinds, None),
                Err(e) => (None, bytes, Vec::new(), Some(e)),
            }
        }
    }
}

/// Adds files from `paths` to the table.
fn add_files_internal(state: &AppState, paths: &[PathBuf]) -> AddResult {
    let mut added = Vec::new();
    let mut skipped = Skipped::default();

    for path in paths {
        if !is_supported_extension(path) {
            skipped.unsupported += 1;
            continue;
        }
        let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        if state.items.contains(&canonical) {
            skipped.duplicates += 1;
            continue;
        }

        let (format, bytes, kinds, error) = inspect_file(state, path);
        let name = file_name(path);
        let entry = Entry {
            path: path.clone(),
            canonical,
            name: name.clone(),
            bytes,
            format,
            error: error.clone(),
        };

        match state.items.insert(entry) {
            Some(id) => added.push(FileItem {
                id,
                name,
                format,
                bytes,
                kinds,
                error,
            }),
            None => skipped.duplicates += 1,
        }
    }

    AddResult { added, skipped }
}

/// Adds files directly in `paths`. Rejects with `JobRunning` if a job is in progress.
pub fn add_files(state: &AppState, paths: &[PathBuf]) -> Result<AddResult, IpcError> {
    if state.is_running.load(Ordering::SeqCst) {
        return Err(IpcError::from_code(ErrorCode::JobRunning));
    }
    Ok(add_files_internal(state, paths))
}

/// Adds files directly in folder `dir`. Subfolders are counted as skipped.
/// Rejects with `JobRunning` if a job is in progress.
///
/// # Errors
///
/// Returns `ReadFailed` if the directory cannot be read.
pub fn add_folder(state: &AppState, dir: &Path) -> Result<AddResult, IpcError> {
    if state.is_running.load(Ordering::SeqCst) {
        return Err(IpcError::from_code(ErrorCode::JobRunning));
    }
    let scan = scan_folder(dir).map_err(|_| IpcError::from_code(ErrorCode::ReadFailed))?;
    let mut result = add_files_internal(state, &scan.files);
    result.skipped = result.skipped.merged(Skipped {
        unsupported: scan.unsupported,
        folders: scan.folders,
        duplicates: 0,
    });
    Ok(result)
}

/// What a drop holds (design §6.1).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct DropSorting {
    pub files: Vec<PathBuf>,
    pub unsupported: u32,
    pub folders: u32,
}

/// Sorts dropped paths by extension. A dropped folder is looked into as in
/// [`scan_folder`]; a file that was dropped itself is kept even if its name is
/// hidden.
pub fn sort_dropped(paths: &[PathBuf]) -> DropSorting {
    let mut sorting = DropSorting::default();
    for path in paths {
        if path.is_dir() {
            match scan_folder(path) {
                Ok(scan) => {
                    sorting.files.extend(scan.files);
                    sorting.unsupported += scan.unsupported;
                    sorting.folders += scan.folders;
                }
                Err(_) => sorting.unsupported += 1,
            }
            continue;
        }
        if is_supported_extension(path) {
            sorting.files.push(path.clone());
        } else {
            sorting.unsupported += 1;
        }
    }
    sorting
}

/// Adds what was dropped to the list (design §6.1, §7.2).
pub fn add_dropped(state: &AppState, paths: &[PathBuf]) -> ItemsDropped {
    if state.is_running.load(Ordering::SeqCst) {
        return ItemsDropped {
            added: Vec::new(),
            skipped: Skipped::default(),
            error: Some(IpcError::from_code(ErrorCode::JobRunning)),
        };
    }
    let sorting = sort_dropped(paths);
    let result = add_files_internal(state, &sorting.files);
    let left_out = Skipped {
        unsupported: sorting.unsupported,
        folders: sorting.folders,
        duplicates: 0,
    };
    ItemsDropped {
        added: result.added,
        skipped: result.skipped.merged(left_out),
        error: None,
    }
}

/// Removes items from the table, rejecting with `JobRunning` during active processing (design §6.1, §7.1).
pub fn remove_items(state: &AppState, ids: &[u64]) -> Result<(), IpcError> {
    if state.is_running.load(Ordering::SeqCst) {
        return Err(IpcError::from_code(ErrorCode::JobRunning));
    }
    state.items.remove(ids);
    Ok(())
}

/// Reads details for an item by re-reading the file (design §6.2, §7.1).
///
/// Accepts requests even while a job is running.
///
/// # Errors
///
/// Returns `UnknownHandle` for an ID that is not in the table, or the item's own
/// error if it was in an error state.
pub fn details(state: &AppState, id: u64) -> Result<Details, IpcError> {
    let entry = state
        .items
        .get(id)
        .ok_or_else(|| IpcError::from_code(ErrorCode::UnknownHandle))?;
    if let Some(error) = entry.error {
        return Err(error);
    }

    let metadata =
        fs::metadata(&entry.path).map_err(|_| IpcError::from_code(ErrorCode::ReadFailed))?;
    let bytes = metadata.len();

    let format = entry
        .format
        .ok_or_else(|| IpcError::from_code(ErrorCode::ReadFailed))?;
    detect::check_size(format, bytes)?;

    match format {
        Format::Jpeg | Format::Png | Format::Webp => {
            let content =
                fs::read(&entry.path).map_err(|_| IpcError::from_code(ErrorCode::ReadFailed))?;
            image_file::details(&content).map_err(IpcError::from)
        }
        Format::Pdf => {
            let mut worker = state.pool.acquire()?;
            let (response, _) = worker.send(&Request::Inspect {
                path: entry.path.clone(),
                details: true,
            })?;
            match response {
                Response::Inspected {
                    details: Some(details),
                    ..
                } => Ok(details),
                _ => Err(IpcError::new(
                    ErrorCode::WorkerCrashed,
                    "missing details in worker response",
                )),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(canonical: &str) -> Entry {
        Entry {
            path: PathBuf::from(canonical),
            canonical: PathBuf::from(canonical),
            name: "test".to_string(),
            bytes: 100,
            format: Some(Format::Jpeg),
            error: None,
        }
    }

    #[test]
    fn ids_only_grow() {
        let table = ItemTable::new();
        let first = table.insert(entry("/a.jpg")).expect("a new file");
        let second = table.insert(entry("/b.jpg")).expect("a new file");
        table.remove(&[second]);
        let third = table.insert(entry("/c.jpg")).expect("a new file");
        assert!(first < second && second < third);
    }

    #[test]
    fn the_same_file_is_added_once_until_it_is_removed() {
        let table = ItemTable::new();
        let id = table.insert(entry("/a.jpg")).expect("a new file");
        assert!(table.contains(Path::new("/a.jpg")));
        assert_eq!(table.insert(entry("/a.jpg")), None);
        table.remove(&[id]);
        assert!(!table.contains(Path::new("/a.jpg")));
        assert!(table.insert(entry("/a.jpg")).is_some());
    }

    #[test]
    fn removing_ignores_ids_that_are_not_in_the_table() {
        let table = ItemTable::new();
        let id = table.insert(entry("/a.jpg")).expect("a new file");
        table.remove(&[id + 100, 0]);
        assert_eq!(table.len(), 1);
        table.remove(&[id, id]);
        assert!(table.is_empty());
        assert!(table.get(id).is_none());
    }

    #[test]
    fn the_extension_decides_support_ignoring_case() {
        assert!(is_supported_extension(Path::new("a.PNG")));
        assert!(is_supported_extension(Path::new("dir/a.JpEg")));
        assert!(is_supported_extension(Path::new("a.Pdf")));
        assert!(is_supported_extension(Path::new("photo.webp")));
        assert!(!is_supported_extension(Path::new("a.txt")));
        assert!(!is_supported_extension(Path::new("pdf")));
        assert!(!is_supported_extension(Path::new("a.png.bak")));
    }
}
