//! IPC commands (design §7.1).

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use mcleaner_core::detect::SUPPORTED_EXTENSIONS;
use mcleaner_core::report::Details;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tauri_plugin_dialog::DialogExt;
use ts_rs::TS;

use crate::AppState;
use crate::error::{ErrorCode, IpcError};
use crate::items::{self, AddResult};
use crate::jobs;
use crate::settings::{self, OutputDirLabel, Settings, SettingsInput};

/// Answer of [`get_about`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AboutInfo {
    pub version: String,
}

/// Returns the app version (design §7.1).
#[tauri::command]
pub fn get_about(app: AppHandle) -> AboutInfo {
    AboutInfo {
        version: app.package_info().version.to_string(),
    }
}

/// Returns the settings, with the output folder as a label (design §6.7, §7.1).
#[tauri::command]
pub fn get_settings(state: tauri::State<'_, AppState>) -> Settings {
    settings::get_settings_internal(&state)
}

/// Saves the language setting (design §6.7, §7.1).
#[tauri::command]
pub async fn save_settings(
    state: tauri::State<'_, AppState>,
    input: SettingsInput,
) -> Result<(), IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.settings.save(&input))
        .await
        .map_err(task_failed)?
}

/// Asks for a folder and makes it the output folder. Returns `None`
/// if the dialog was cancelled (design §6.7, §7.1).
#[tauri::command]
pub async fn pick_output_dir(
    window: tauri::Window,
    state: tauri::State<'_, AppState>,
) -> Result<Option<OutputDirLabel>, IpcError> {
    if state.is_running.load(Ordering::SeqCst) {
        return Err(IpcError::from_code(ErrorCode::JobRunning));
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(dir) = pick_folder(&window)? else {
            return Ok(None);
        };
        let (label, persisted) = settings::apply_picked_dir(&state, dir)?;
        #[cfg(debug_assertions)]
        if let Err(error) = &persisted {
            eprintln!("[debug] settings were not saved: {error}");
        }
        // The folder is in use even if the file could not be written.
        let _ = persisted;
        Ok(Some(label))
    })
    .await
    .map_err(task_failed)?
}

/// Where `add_files` takes files from (design §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum AddSource {
    Files,
    Folder,
}

/// Asks for files or a folder and adds them to the list (design §7.1).
/// Returns `None` if the dialog was cancelled.
#[tauri::command]
pub async fn add_files(
    window: tauri::Window,
    state: tauri::State<'_, AppState>,
    source: AddSource,
) -> Result<Option<AddResult>, IpcError> {
    if state.is_running.load(Ordering::SeqCst) {
        return Err(IpcError::from_code(ErrorCode::JobRunning));
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || match source {
        AddSource::Files => {
            let picked = pick_files(&window, "Supported files", &SUPPORTED_EXTENSIONS)?;
            picked
                .map(|paths| items::add_files(&state, &paths))
                .transpose()
        }
        AddSource::Folder => {
            let picked = pick_folder(&window)?;
            picked
                .map(|dir| items::add_folder(&state, &dir))
                .transpose()
        }
    })
    .await
    .map_err(task_failed)?
}

/// Removes items from the list (design §7.1). An ID not in the table is ignored.
#[tauri::command]
pub fn remove_items(state: tauri::State<'_, AppState>, ids: Vec<u64>) -> Result<(), IpcError> {
    items::remove_items(&state, &ids)
}

/// Gets the metadata details for an item (design §6.2, §7.1).
#[tauri::command]
pub async fn get_details(state: tauri::State<'_, AppState>, id: u64) -> Result<Details, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || items::details(&state, id))
        .await
        .map_err(task_failed)?
}

/// Cancels the currently executing cleaning job (design §6.5, §7.1).
#[tauri::command]
pub fn cancel_job(state: tauri::State<'_, AppState>) {
    state.cancel_flag.store(true, Ordering::SeqCst);
}

/// Starts batch cleaning of metadata in the background (design §6.3, §7.1).
#[tauri::command]
pub async fn start_clean(
    window: tauri::Window,
    state: tauri::State<'_, AppState>,
    ids: Vec<u64>,
) -> Result<(), IpcError> {
    let w1 = window.clone();
    let w2 = window.clone();
    let w3 = window.clone();

    let callbacks = jobs::JobCallbacks {
        on_progress: move |p| {
            let _ = w1.emit(jobs::JOB_PROGRESS_EVENT, &p);
        },
        on_item: move |item| {
            let _ = w2.emit(jobs::JOB_ITEM_EVENT, &item);
        },
        on_finished: move |fin| {
            let _ = w3.emit(jobs::JOB_FINISHED_EVENT, &fin);
        },
    };

    jobs::start_clean_internal(&state, &ids, callbacks)
}

/// Opens a dialog to pick several files with one of `extensions`.
fn pick_files(
    window: &tauri::Window,
    filter_name: &str,
    extensions: &[&str],
) -> Result<Option<Vec<PathBuf>>, IpcError> {
    let patterns = filter_extensions(extensions);
    let patterns: Vec<&str> = patterns.iter().map(String::as_str).collect();
    window
        .dialog()
        .file()
        .set_parent(window)
        .add_filter(filter_name, &patterns)
        .blocking_pick_files()
        .map(|files| files.into_iter().map(dialog_path).collect())
        .transpose()
}

/// The extensions to give a file dialog's filter: each one in lower and upper
/// case. On Linux the dialog is GTK3's, whose patterns are case sensitive, so
/// `*.png` alone would hide `IMG_0001.PNG`; the Windows dialog ignores case and
/// is not hurt by the extra entries.
fn filter_extensions(extensions: &[&str]) -> Vec<String> {
    extensions
        .iter()
        .flat_map(|extension| [extension.to_lowercase(), extension.to_uppercase()])
        .collect()
}

/// Opens a dialog to pick a folder.
fn pick_folder(window: &tauri::Window) -> Result<Option<PathBuf>, IpcError> {
    window
        .dialog()
        .file()
        .set_parent(window)
        .blocking_pick_folder()
        .map(dialog_path)
        .transpose()
}

fn dialog_path(picked: tauri_plugin_dialog::FilePath) -> Result<PathBuf, IpcError> {
    picked
        .into_path()
        .map_err(|_| IpcError::from_code(ErrorCode::ReadFailed))
}

fn task_failed(error: tauri::Error) -> IpcError {
    IpcError::new(ErrorCode::ReadFailed, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_filter_has_each_extension_in_both_cases() {
        assert_eq!(filter_extensions(&["pdf"]), ["pdf", "PDF"]);
        assert_eq!(
            filter_extensions(&SUPPORTED_EXTENSIONS),
            [
                "jpg", "JPG", "jpeg", "JPEG", "png", "PNG", "webp", "WEBP", "pdf", "PDF"
            ]
        );
    }
}
