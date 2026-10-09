//! Integration tests for the lists of items (design §6.1, §6.2, §7.1):
//! adding files and folders, inspection, details, and drop sorting.
//! Uses the real worker pool.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

use mcleaner_core::detect::Format;
use mcleaner_core::image_file;
use mcleaner_worker::WORKER_FLAG;
use metadata_cleaner_lib::AppState;
use metadata_cleaner_lib::error::ErrorCode;
use metadata_cleaner_lib::items::{
    AddResult, Skipped, add_dropped, add_files, add_folder, details, remove_items,
};
use metadata_cleaner_lib::worker_pool::{WorkerPool, WorkerPoolConfig};

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

fn put(dir: &Path, name: &str, as_name: &str) -> PathBuf {
    let dest = dir.join(as_name);
    fs::copy(fixture(name), &dest).expect("copying a fixture");
    dest
}

fn item_names(result: &AddResult) -> Vec<String> {
    result.added.iter().map(|item| item.name.clone()).collect()
}

#[test]
fn a_folder_is_read_one_level_deep_and_counts_skipped() {
    let dir = tempfile::tempdir().expect("creating a temporary folder");
    put(dir.path(), "full.jpg", "b.JPG");
    put(dir.path(), "clean.png", "a.png");
    put(dir.path(), "full.pdf", "doc.pdf");
    fs::write(dir.path().join("notes.txt"), "text file").expect("writing a text file");
    put(dir.path(), "clean.png", ".hidden.png");
    fs::create_dir(dir.path().join("sub")).expect("creating a subfolder");
    put(&dir.path().join("sub"), "clean.png", "nested.png");

    let state = state();

    let result = add_folder(&state, dir.path()).expect("adding folder");

    // Files directly in the folder sorted by name case-insensitively
    assert_eq!(item_names(&result), ["a.png", "b.JPG", "doc.pdf"]);
    // notes.txt is unsupported (1), sub is folder (1), .hidden.png is ignored without count
    assert_eq!(
        result.skipped,
        Skipped {
            unsupported: 1,
            folders: 1,
            duplicates: 0,
        }
    );
    assert_eq!(state.items.len(), 3);

    // Adding the same folder again reports all 3 as duplicates
    let second_result = add_folder(&state, dir.path()).expect("adding same folder again");
    assert_eq!(second_result.added.len(), 0);
    assert_eq!(
        second_result.skipped,
        Skipped {
            unsupported: 1,
            folders: 1,
            duplicates: 3,
        }
    );
}

#[test]
fn error_items_are_added_with_error_code_and_null_format() {
    let dir = tempfile::tempdir().expect("creating a temporary folder");
    let corrupt_jpg = put(dir.path(), "corrupt.jpg", "corrupt.jpg");
    let encrypted_pdf = put(dir.path(), "encrypted.pdf", "encrypted.pdf");
    let signed_pdf = put(dir.path(), "signed.pdf", "signed.pdf");
    let fake_jpg = dir.path().join("fake.jpg");
    fs::write(&fake_jpg, "not a jpeg file").expect("writing fake.jpg");

    let state = state();
    let result = add_files(&state, &[corrupt_jpg, encrypted_pdf, signed_pdf, fake_jpg])
        .expect("adding files");

    assert_eq!(result.added.len(), 4);

    let corrupt = &result.added[0];
    assert_eq!(corrupt.name, "corrupt.jpg");
    assert_eq!(corrupt.format, None);
    assert!(corrupt.kinds.is_empty());
    assert_eq!(
        corrupt.error.as_ref().map(|e| e.code),
        Some(ErrorCode::DecodeFailed)
    );

    let encrypted = &result.added[1];
    assert_eq!(encrypted.name, "encrypted.pdf");
    assert_eq!(encrypted.format, None);
    assert!(encrypted.kinds.is_empty());
    assert_eq!(
        encrypted.error.as_ref().map(|e| e.code),
        Some(ErrorCode::PdfEncrypted)
    );

    let signed = &result.added[2];
    assert_eq!(signed.name, "signed.pdf");
    assert_eq!(signed.format, None);
    assert!(signed.kinds.is_empty());
    assert_eq!(
        signed.error.as_ref().map(|e| e.code),
        Some(ErrorCode::PdfSigned)
    );

    let fake = &result.added[3];
    assert_eq!(fake.name, "fake.jpg");
    assert_eq!(fake.format, None);
    assert!(fake.kinds.is_empty());
    assert_eq!(
        fake.error.as_ref().map(|e| e.code),
        Some(ErrorCode::UnsupportedFormat)
    );
}

#[test]
fn inspects_kinds_and_formats_correctly() {
    let dir = tempfile::tempdir().expect("creating a temporary folder");
    let full_jpg_path = put(dir.path(), "full.jpg", "full.jpg");
    let full_pdf_path = put(dir.path(), "full.pdf", "full.pdf");
    let clean_png_path = put(dir.path(), "clean.png", "clean.png");
    let webp_named_jpg_path = put(dir.path(), "webp_named.jpg", "webp_named.jpg");

    let state = state();
    let result = add_files(
        &state,
        &[
            full_jpg_path.clone(),
            full_pdf_path,
            clean_png_path,
            webp_named_jpg_path,
        ],
    )
    .expect("adding files");

    assert_eq!(result.added.len(), 4);

    // full.jpg kinds match image_file::inspect
    let full_jpg_bytes = fs::read(&full_jpg_path).expect("reading full.jpg");
    let expected_jpg_insp = image_file::inspect(&full_jpg_bytes).expect("inspecting full.jpg");
    let full_jpg_item = &result.added[0];
    assert_eq!(full_jpg_item.format, Some(Format::Jpeg));
    assert_eq!(full_jpg_item.kinds, expected_jpg_insp.kinds);
    assert_eq!(full_jpg_item.error, None);

    // full.pdf kinds are non-empty and error is None
    let full_pdf_item = &result.added[1];
    assert_eq!(full_pdf_item.format, Some(Format::Pdf));
    assert!(!full_pdf_item.kinds.is_empty());
    assert_eq!(full_pdf_item.error, None);

    // clean.png kinds are empty
    let clean_png_item = &result.added[2];
    assert_eq!(clean_png_item.format, Some(Format::Png));
    assert!(clean_png_item.kinds.is_empty());
    assert_eq!(clean_png_item.error, None);

    // webp_named.jpg format is Webp
    let webp_item = &result.added[3];
    assert_eq!(webp_item.format, Some(Format::Webp));
    assert_eq!(webp_item.error, None);
}

#[test]
fn details_and_re_reading_without_cached_values() {
    let dir = tempfile::tempdir().expect("creating a temporary folder");
    let modifiable_path = put(dir.path(), "full.jpg", "modifiable.jpg");
    let full_pdf_path = put(dir.path(), "full.pdf", "full.pdf");
    let corrupt_jpg_path = put(dir.path(), "corrupt.jpg", "corrupt.jpg");

    let state = state();
    let result = add_files(
        &state,
        &[modifiable_path.clone(), full_pdf_path, corrupt_jpg_path],
    )
    .expect("adding files");

    let img_id = result.added[0].id;
    let pdf_id = result.added[1].id;
    let err_id = result.added[2].id;

    // Image details ok
    let img_details = details(&state, img_id).expect("getting image details");
    assert!(!img_details.groups.is_empty());

    // PDF details ok
    let pdf_details = details(&state, pdf_id).expect("getting pdf details");
    assert!(!pdf_details.groups.is_empty());

    // Error item returns its error
    let err_details = details(&state, err_id).expect_err("details for corrupt file");
    assert_eq!(err_details.code, ErrorCode::DecodeFailed);

    // Unknown handle
    let missing_details = details(&state, 9999).expect_err("details for unknown ID");
    assert_eq!(missing_details.code, ErrorCode::UnknownHandle);

    // Verify details are not cached: overwrite the file with clean.jpg content
    let clean_bytes = fs::read(fixture("clean.jpg")).expect("reading clean.jpg");
    fs::write(&modifiable_path, clean_bytes).expect("overwriting modifiable.jpg");

    // Second call to details sees the new file content and returns empty groups
    let new_details = details(&state, img_id).expect("getting details after overwrite");
    assert!(new_details.groups.is_empty());
}

#[test]
fn operations_during_job_running() {
    let dir = tempfile::tempdir().expect("creating a temporary folder");
    let img_path = put(dir.path(), "full.jpg", "img.jpg");

    let state = state();
    let add_res = add_files(&state, std::slice::from_ref(&img_path)).expect("adding file");
    let id = add_res.added[0].id;

    // Set job running flag
    state.is_running.store(true, Ordering::SeqCst);

    // add_files rejected with JobRunning
    let add_err =
        add_files(&state, std::slice::from_ref(&img_path)).expect_err("add_files during job");
    assert_eq!(add_err.code, ErrorCode::JobRunning);

    // add_folder rejected with JobRunning
    let folder_err = add_folder(&state, dir.path()).expect_err("add_folder during job");
    assert_eq!(folder_err.code, ErrorCode::JobRunning);

    // remove_items rejected with JobRunning
    let rm_err = remove_items(&state, &[id]).expect_err("remove_items during job");
    assert_eq!(rm_err.code, ErrorCode::JobRunning);

    // add_dropped returns empty added and JobRunning error
    let dropped = add_dropped(&state, &[img_path]);
    assert!(dropped.added.is_empty());
    assert_eq!(dropped.skipped.unsupported, 0);
    assert_eq!(
        dropped.error.as_ref().map(|e| e.code),
        Some(ErrorCode::JobRunning)
    );

    // details is still accepted during job running
    let det = details(&state, id).expect("details accepted during job");
    assert!(!det.groups.is_empty());
}

#[test]
fn drop_sorting_and_addition() {
    let dir = tempfile::tempdir().expect("creating a temporary folder");
    let img_path = put(dir.path(), "full.jpg", "drop_img.jpg");
    let pdf_path = put(dir.path(), "full.pdf", "drop_pdf.pdf");
    let text_path = dir.path().join("drop_txt.txt");
    fs::write(&text_path, "plain text").expect("writing text file");

    let sub = dir.path().join("sub_folder");
    fs::create_dir(&sub).expect("creating subfolder");
    put(&sub, "clean.png", "sub_img.png");
    let nested = sub.join("nested_folder");
    fs::create_dir(&nested).expect("creating nested folder");

    let state = state();
    let dropped = add_dropped(&state, &[img_path, pdf_path, text_path, sub]);

    assert_eq!(dropped.error, None);
    assert_eq!(dropped.added.len(), 3); // drop_img.jpg, drop_pdf.pdf, sub_img.png
    assert_eq!(dropped.skipped.unsupported, 1); // drop_txt.txt
    assert_eq!(dropped.skipped.folders, 1); // nested_folder inside sub_folder
}
