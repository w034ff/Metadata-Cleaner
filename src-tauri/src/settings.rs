//! Settings kept in `settings.json` and the output folder (design §6.5, §6.7).
//!
//! Loading, validating and writing take the settings folder as an argument so
//! they can run without a Tauri app.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::AppState;
use crate::error::{ErrorCode, IpcError};

/// The `schemaVersion` this build reads and writes (design §6.7).
pub const SCHEMA_VERSION: u32 = 1;

/// Name of the settings file inside the settings folder.
pub const SETTINGS_FILE_NAME: &str = "settings.json";

/// The UI language the user chose (design §6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Ja,
    En,
}

/// What the frontend may know of an output folder: its name, never its path
/// (design §7.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OutputDirLabel {
    pub dir_label: String,
}

/// Argument of `save_settings`: [`Settings`] without the folder, which only
/// `pick_output_dir` changes (design §6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    pub language: Option<Language>,
}

/// Answer of `get_settings` (design §6.7). It holds no path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub language: Option<Language>,
    pub output_dir: Option<OutputDirLabel>,
}

/// What `settings.json` holds (design §6.7). Unlike [`Settings`] it has the
/// folder's path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsFile {
    pub schema_version: u32,
    pub language: Option<Language>,
    #[serde(serialize_with = "serialize_dir")]
    pub output_dir: Option<PathBuf>,
}

impl Default for SettingsFile {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            language: None,
            output_dir: None,
        }
    }
}

/// Writes a folder as a string. A path that is not valid Unicode cannot be
/// written as one, so it becomes `null`: the folder is then used in this
/// session but not remembered, and the file stays writable.
fn serialize_dir<S: Serializer>(dir: &Option<PathBuf>, serializer: S) -> Result<S::Ok, S::Error> {
    dir.as_deref().and_then(Path::to_str).serialize(serializer)
}

/// Reads `key` of `object` as a `T`; `None` if it is missing or not a `T`.
fn field<T: DeserializeOwned>(object: Option<&Map<String, Value>>, key: &str) -> Option<T> {
    T::deserialize(object?.get(key)?).ok()
}

/// Reads `settings.json` text. Every item that is missing, of the wrong type
/// or unknown goes back to its default and the others stay (design §6.7).
/// Text that is not JSON, or whose `schemaVersion` is not [`SCHEMA_VERSION`],
/// gives the defaults for everything.
///
/// Folders are returned as written; [`load_settings`] checks they exist.
pub fn parse_settings_json(text: &str) -> SettingsFile {
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(text) else {
        return SettingsFile::default();
    };
    if root.get("schemaVersion").and_then(Value::as_u64) != Some(u64::from(SCHEMA_VERSION)) {
        return SettingsFile::default();
    }
    SettingsFile {
        schema_version: SCHEMA_VERSION,
        language: field(Some(&root), "language"),
        output_dir: field(Some(&root), "outputDir"),
    }
}

/// Reads the settings in `config_dir`. A missing or unreadable file gives the
/// defaults, and a saved folder that is not an absolute path to a folder
/// becomes `None` (design §6.7): a relative one would resolve against
/// wherever the app was started from.
pub fn load_settings(config_dir: &Path) -> SettingsFile {
    let mut file = std::fs::read_to_string(config_dir.join(SETTINGS_FILE_NAME))
        .map(|text| parse_settings_json(&text))
        .unwrap_or_default();
    if file
        .output_dir
        .as_deref()
        .is_some_and(|dir| !dir.is_absolute() || !dir.is_dir())
    {
        file.output_dir = None;
    }
    file
}

fn write_failed(error: impl std::fmt::Display) -> IpcError {
    IpcError::new(ErrorCode::WriteFailed, error.to_string())
}

/// Writes `file` as `settings.json` in `config_dir`, creating the folder if
/// needed.
///
/// The content goes to a temporary file in the same folder first and is then
/// renamed over `settings.json`, so a crash in between leaves the old file,
/// not a half-written one. A failure leaves no temporary file.
///
/// # Errors
///
/// `WriteFailed` if the folder or the file cannot be written.
pub fn save_settings_to_dir(config_dir: &Path, file: &SettingsFile) -> Result<(), IpcError> {
    std::fs::create_dir_all(config_dir).map_err(write_failed)?;
    let json = serde_json::to_vec_pretty(file).map_err(write_failed)?;
    let mut temp = tempfile::NamedTempFile::new_in(config_dir).map_err(write_failed)?;
    temp.write_all(&json).map_err(write_failed)?;
    temp.as_file().sync_all().map_err(write_failed)?;
    temp.persist(config_dir.join(SETTINGS_FILE_NAME))
        .map_err(|e| write_failed(e.error))?;
    Ok(())
}

#[derive(Debug, Default)]
struct Stored {
    /// Where the file is written; `None` before [`restore_settings`] ran, in
    /// which case nothing is written.
    config_dir: Option<PathBuf>,
    file: SettingsFile,
}

/// The one copy of the settings in memory. Every change goes through it and
/// rewrites the whole file while the lock is held, so two changes cannot
/// overwrite each other with stale content.
#[derive(Debug, Default)]
pub struct SettingsStore {
    inner: Mutex<Stored>,
}

impl SettingsStore {
    fn lock(&self) -> std::sync::MutexGuard<'_, Stored> {
        self.inner.lock().expect("settings lock")
    }

    /// A copy of what is held now.
    pub fn file(&self) -> SettingsFile {
        self.lock().file.clone()
    }

    /// Replaces the held settings with `file`, read from `config_dir`, where
    /// later changes are written.
    fn restore(&self, config_dir: PathBuf, file: SettingsFile) {
        *self.lock() = Stored {
            config_dir: Some(config_dir),
            file,
        };
    }

    /// Takes `input` over, folders excluded, and writes the file. Nothing
    /// changes, in memory or on disk, if this fails.
    ///
    /// # Errors
    ///
    /// `WriteFailed` if the file cannot be written.
    pub fn save(&self, input: &SettingsInput) -> Result<(), IpcError> {
        let mut stored = self.lock();
        let mut next = stored.file.clone();
        next.language = input.language;
        write(stored.config_dir.as_deref(), &next)?;
        stored.file = next;
        Ok(())
    }

    /// Puts `dir` in the held settings and writes the file. The folder stays
    /// in memory if the write fails.
    ///
    /// # Errors
    ///
    /// `WriteFailed` if the file cannot be written.
    pub fn record_output_dir(&self, dir: &Path) -> Result<(), IpcError> {
        self.set_output_dir(Some(dir.to_path_buf()))
    }

    /// Forgets the output folder in the held settings and writes the file.
    /// The folder stays forgotten in memory if the write fails.
    ///
    /// # Errors
    ///
    /// `WriteFailed` if the file cannot be written.
    pub fn clear_output_dir(&self) -> Result<(), IpcError> {
        self.set_output_dir(None)
    }

    fn set_output_dir(&self, dir: Option<PathBuf>) -> Result<(), IpcError> {
        let mut stored = self.lock();
        stored.file.output_dir = dir;
        write(stored.config_dir.as_deref(), &stored.file)
    }
}

fn write(config_dir: Option<&Path>, file: &SettingsFile) -> Result<(), IpcError> {
    config_dir.map_or(Ok(()), |dir| save_settings_to_dir(dir, file))
}

/// Loads the settings in `config_dir` into `state`: the held settings, and the
/// output folder if it still exists. Called once at startup, before the state
/// is shared (design §6.7).
pub fn restore_settings(state: &AppState, config_dir: &Path) {
    let file = load_settings(config_dir);
    *state.output_dir.lock().expect("output_dir lock") = file.output_dir.clone();
    state.settings.restore(config_dir.to_path_buf(), file);
}

/// The name of the last component of `dir`, or the whole path for one that has
/// none (a drive or the root).
pub fn dir_label(dir: &Path) -> String {
    dir.file_name().map_or_else(
        || dir.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn label_of(slot: &Mutex<Option<PathBuf>>) -> Option<OutputDirLabel> {
    slot.lock()
        .expect("output_dir lock")
        .as_deref()
        .map(|dir| OutputDirLabel {
            dir_label: dir_label(dir),
        })
}

/// The settings as `get_settings` returns them. The folder is the one
/// cleaning will use, taken from the state.
pub fn get_settings_internal(state: &AppState) -> Settings {
    let file = state.settings.file();
    Settings {
        language: file.language,
        output_dir: label_of(&state.output_dir),
    }
}

/// Makes `dir`, which the user picked, the output folder and writes the settings.
///
/// Returns the label to show, and whether the settings file could be written.
/// The folder is in the state either way, so a settings file that cannot be
/// written does not stop clean jobs from starting (design §6.7).
///
/// # Errors
///
/// `JobRunning` if a clean job is currently executing (design §7.1).
pub fn apply_picked_dir(
    state: &AppState,
    dir: PathBuf,
) -> Result<(OutputDirLabel, Result<(), IpcError>), IpcError> {
    if state.is_running.load(Ordering::SeqCst) {
        return Err(IpcError::from_code(ErrorCode::JobRunning));
    }
    let label = OutputDirLabel {
        dir_label: dir_label(&dir),
    };
    let persisted = state.settings.record_output_dir(&dir);
    *state.output_dir.lock().expect("output_dir lock") = Some(dir);
    Ok((label, persisted))
}

/// Puts the output folder back to "not chosen", in the state and in
/// the settings file (design §6.5).
///
/// The folder is forgotten in the state even if the file cannot be written.
///
/// # Errors
///
/// `JobRunning` if a clean job is currently executing (design §7.1).
/// `WriteFailed` if the settings file cannot be written.
pub fn clear_output_dir(state: &AppState) -> Result<(), IpcError> {
    if state.is_running.load(Ordering::SeqCst) {
        return Err(IpcError::from_code(ErrorCode::JobRunning));
    }
    let persisted = state.settings.clear_output_dir();
    *state.output_dir.lock().expect("output_dir lock") = None;
    persisted
}
