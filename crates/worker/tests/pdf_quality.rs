//! PDF quality tests (design §11.2, work-plan T06).

use std::path::Path;

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::{PixmapSettings, RenderCache, RenderSettings, render};
use lopdf::{Document, Object};
use mcleaner_worker::pdf::{clean, inspect};

const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../core/tests/fixtures");

fn read_fixture(name: &str) -> Vec<u8> {
    let path = Path::new(FIXTURES_DIR).join(name);
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}

mod fixture_values {
    #![allow(dead_code)]
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../core/fixture_values.rs"
    ));
}

const FICTIONAL_VALUES: &[&str] = &[
    fixture_values::AUTHOR,
    fixture_values::EDITOR,
    fixture_values::SOFTWARE,
    fixture_values::CAMERA_MAKE,
    fixture_values::CAMERA_MODEL,
    fixture_values::SERIAL_NUMBER,
    fixture_values::DATE_TIME,
    fixture_values::COMMENT,
    fixture_values::TITLE,
    fixture_values::SUBJECT,
    fixture_values::KEYWORDS,
    fixture_values::COPYRIGHT,
    fixture_values::CITY,
    fixture_values::STATE,
    fixture_values::COUNTRY,
];

fn render_all_pages_72dpi(pdf_bytes: &[u8]) -> Vec<Vec<u8>> {
    let pdf = Pdf::new(pdf_bytes.to_vec()).expect("hayro should load valid PDF");
    let cache = RenderCache::new();
    let interpreter_settings = InterpreterSettings::default();
    let render_settings = RenderSettings::default();
    let pixmap_settings = PixmapSettings {
        x_scale: 1.0,
        y_scale: 1.0,
        bg_color: hayro::vello_cpu::color::palette::css::WHITE,
    };

    let mut pages_pixels = Vec::new();
    for page in pdf.pages().iter() {
        let pixmap = render(
            page,
            &cache,
            &interpreter_settings,
            &render_settings,
            &pixmap_settings,
        );
        pages_pixels.push(pixmap.data_as_u8_slice().to_vec());
    }
    pages_pixels
}

fn check_no_fictional_values(cleaned_bytes: &[u8]) {
    // 1. Raw bytes
    for needle in FICTIONAL_VALUES {
        let needle_bytes = needle.as_bytes();
        assert!(
            !cleaned_bytes
                .windows(needle_bytes.len())
                .any(|w| w == needle_bytes),
            "raw bytes contain fictional value: {needle}"
        );
    }

    // Load with lopdf
    let doc = Document::load_mem(cleaned_bytes).expect("cleaned PDF must be loadable in lopdf");

    // 2. All string objects in doc
    for obj in doc.objects.values() {
        check_obj_strings_no_fictional(obj);
    }

    // 3. Decompressed stream contents
    for obj in doc.objects.values() {
        if let Object::Stream(s) = obj {
            let content = s
                .decompressed_content()
                .unwrap_or_else(|_| s.content.clone());
            for needle in FICTIONAL_VALUES {
                let needle_bytes = needle.as_bytes();
                assert!(
                    !content
                        .windows(needle_bytes.len())
                        .any(|w| w == needle_bytes),
                    "stream contains fictional value: {needle}"
                );
            }
        }
    }
}

fn check_obj_strings_no_fictional(obj: &Object) {
    match obj {
        Object::String(bytes, _) => {
            let text = String::from_utf8_lossy(bytes);
            for needle in FICTIONAL_VALUES {
                assert!(
                    !text.contains(needle),
                    "string object contains fictional value: {needle}"
                );
            }
        }
        Object::Dictionary(d) => {
            for (_, v) in d.iter() {
                check_obj_strings_no_fictional(v);
            }
        }
        Object::Array(arr) => {
            for item in arr {
                check_obj_strings_no_fictional(item);
            }
        }
        Object::Stream(s) => {
            for (_, v) in s.dict.iter() {
                check_obj_strings_no_fictional(v);
            }
        }
        _ => {}
    }
}

#[test]
fn test_pdf_quality_full_and_linearized() {
    for fixture_name in ["full.pdf", "linearized.pdf"] {
        let original_bytes = read_fixture(fixture_name);
        let cleaned_bytes = clean(&original_bytes).expect("clean should succeed");

        // 1. Hayro rendering matches pixel-for-pixel at 72 dpi
        let original_pages = render_all_pages_72dpi(&original_bytes);
        let cleaned_pages = render_all_pages_72dpi(&cleaned_bytes);
        assert_eq!(
            original_pages.len(),
            cleaned_pages.len(),
            "page count mismatch for {fixture_name}"
        );
        for (i, (orig, clean)) in original_pages.iter().zip(&cleaned_pages).enumerate() {
            assert_eq!(orig, clean, "pixel mismatch on page {i} for {fixture_name}");
        }

        // 2. Inspection kinds on cleaned PDF is empty
        let insp = inspect(&cleaned_bytes).expect("inspect should succeed on cleaned PDF");
        assert!(
            insp.kinds.is_empty(),
            "cleaned {fixture_name} should have no metadata kinds, found: {:?}",
            insp.kinds
        );

        // 3. No fictional values remain
        check_no_fictional_values(&cleaned_bytes);
    }
}
