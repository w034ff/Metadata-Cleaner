//! Cleans all cleanable fixtures and writes them to the specified output folder.
//!
//! Used by CI to verify that ExifTool detects no stripped metadata in cleaned
//! files (design §11.2).

use std::path::{Path, PathBuf};

/// Fixture files excluded from cleaning verification because they cannot be
/// cleaned by design (design §4.6, §11.1).
const EXCLUDED_FIXTURES: &[&str] = &[
    "corrupt.jpg",
    "corrupt.pdf",
    "corrupt.png",
    "corrupt.webp",
    "encrypted.pdf",
    "restricted.pdf",
    "signed.pdf",
];

const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../core/tests/fixtures");

fn is_excluded(name: &str) -> bool {
    name.starts_with("corrupt.") || EXCLUDED_FIXTURES.contains(&name)
}

fn main() {
    let output_dir_str = match std::env::args().nth(1) {
        Some(arg) => arg,
        None => {
            eprintln!("Usage: clean_fixtures <output_dir>");
            std::process::exit(1);
        }
    };
    let output_dir = PathBuf::from(output_dir_str);

    if let Err(err) = std::fs::create_dir_all(&output_dir) {
        eprintln!(
            "Failed to create output directory {}: {err}",
            output_dir.display()
        );
        std::process::exit(1);
    }

    let fixtures_path = Path::new(FIXTURES_DIR);
    let entries = match std::fs::read_dir(fixtures_path) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!(
                "Failed to read fixtures directory {}: {err}",
                fixtures_path.display()
            );
            std::process::exit(1);
        }
    };

    let mut files = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("Failed to read directory entry: {err}");
                std::process::exit(1);
            }
        };
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(err) => {
                eprintln!(
                    "Failed to read file type for {}: {err}",
                    entry.path().display()
                );
                std::process::exit(1);
            }
        };
        if file_type.is_file() {
            let name = entry.file_name().to_string_lossy().into_owned();
            files.push((name, entry.path()));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    if files.is_empty() {
        eprintln!("No fixtures found in {}", fixtures_path.display());
        std::process::exit(1);
    }

    let mut has_failure = false;
    let mut processed_count = 0;

    for (name, path) in &files {
        if is_excluded(name) {
            continue;
        }

        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(err) => {
                eprintln!("Failed to read fixture {name}: {err}");
                has_failure = true;
                continue;
            }
        };

        let is_pdf = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"));

        let clean_result: Result<Vec<u8>, &str> = if is_pdf {
            mcleaner_worker::pdf::clean(&bytes).map_err(|e| e.code())
        } else {
            mcleaner_core::image_file::clean(&bytes).map_err(|e| e.code())
        };

        match clean_result {
            Ok(cleaned_bytes) => {
                let dest_path = output_dir.join(name);
                if let Err(err) = std::fs::write(&dest_path, cleaned_bytes) {
                    eprintln!(
                        "Failed to write cleaned file {}: {err}",
                        dest_path.display()
                    );
                    has_failure = true;
                } else {
                    processed_count += 1;
                }
            }
            Err(code) => {
                eprintln!("Failed to clean {name}: {code}");
                has_failure = true;
            }
        }
    }

    if has_failure {
        std::process::exit(1);
    }

    if processed_count == 0 {
        eprintln!("No fixtures were cleaned");
        std::process::exit(1);
    }
}
