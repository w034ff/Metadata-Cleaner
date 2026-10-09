//! Writes the TypeScript types of the IPC to `src/ipc/generated/`. CI runs
//! this test and fails if the committed files differ from what it writes.

use std::path::Path;

use mcleaner_core::detect::Format;
use mcleaner_core::report::{
    DetailEntry, DetailGroup, DetailValue, Details, Field, KeptInfo, MetadataKind, ResolutionUnit,
};
use metadata_cleaner_lib::commands::{AboutInfo, AddSource};
use metadata_cleaner_lib::error::{ErrorCode, IpcError};
use metadata_cleaner_lib::items::{AddResult, FileItem, ItemsDropped, Skipped};
use metadata_cleaner_lib::settings::{Language, OutputDirLabel, Settings, SettingsInput};
use ts_rs::{Config, TS};

#[test]
fn export_typescript_bindings() {
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/ipc/generated");
    std::fs::create_dir_all(&out_dir).expect("creating the export directory");
    let cfg = Config::new().with_out_dir(&out_dir);

    ErrorCode::export_all(&cfg).expect("exporting ErrorCode");
    IpcError::export_all(&cfg).expect("exporting IpcError");
    AddSource::export_all(&cfg).expect("exporting AddSource");
    Format::export_all(&cfg).expect("exporting Format");
    MetadataKind::export_all(&cfg).expect("exporting MetadataKind");
    Field::export_all(&cfg).expect("exporting Field");
    DetailValue::export_all(&cfg).expect("exporting DetailValue");
    DetailEntry::export_all(&cfg).expect("exporting DetailEntry");
    DetailGroup::export_all(&cfg).expect("exporting DetailGroup");
    ResolutionUnit::export_all(&cfg).expect("exporting ResolutionUnit");
    KeptInfo::export_all(&cfg).expect("exporting KeptInfo");
    Details::export_all(&cfg).expect("exporting Details");
    FileItem::export_all(&cfg).expect("exporting FileItem");
    Skipped::export_all(&cfg).expect("exporting Skipped");
    AddResult::export_all(&cfg).expect("exporting AddResult");
    ItemsDropped::export_all(&cfg).expect("exporting ItemsDropped");
    AboutInfo::export_all(&cfg).expect("exporting AboutInfo");
    Language::export_all(&cfg).expect("exporting Language");
    OutputDirLabel::export_all(&cfg).expect("exporting OutputDirLabel");
    Settings::export_all(&cfg).expect("exporting Settings");
    SettingsInput::export_all(&cfg).expect("exporting SettingsInput");
}
