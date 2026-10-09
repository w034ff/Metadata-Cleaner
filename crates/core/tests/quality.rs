//! Quality tests verifying that cleaning preserves image data, orientation,
//! and ICC color profiles across all valid image fixtures (design §11.2).

use std::io::Cursor;
use std::path::Path;

use image::ImageDecoder;
use mcleaner_core::detect::Format;
use mcleaner_core::image_file;
use mcleaner_core::jpeg;
use mcleaner_core::png;
use mcleaner_core::report::KeptInfo;
use mcleaner_core::webp;

const FIXTURE_DIR: &str = "tests/fixtures";

const NON_CORRUPT_FIXTURES: &[&str] = &[
    "full.jpg",
    "orient1.jpg",
    "orient2.jpg",
    "orient3.jpg",
    "orient4.jpg",
    "orient5.jpg",
    "orient6.jpg",
    "orient7.jpg",
    "orient8.jpg",
    "cmyk.jpg",
    "progressive.jpg",
    "no_jfif_dpi.jpg",
    "clean.jpg",
    "full.png",
    "anim.png",
    "clean.png",
    "full.webp",
    "anim.webp",
    "clean.webp",
    "webp_named.jpg",
];

fn get_decoder_icc(format: Format, bytes: &[u8]) -> Option<Vec<u8>> {
    match format {
        Format::Jpeg => {
            let mut dec = image::codecs::jpeg::JpegDecoder::new(Cursor::new(bytes)).ok()?;
            dec.icc_profile().ok().flatten()
        }
        Format::Png => {
            let mut dec = image::codecs::png::PngDecoder::new(Cursor::new(bytes)).ok()?;
            dec.icc_profile().ok().flatten()
        }
        Format::Webp => {
            let mut dec = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes)).ok()?;
            dec.icc_profile().ok().flatten()
        }
        Format::Pdf => None,
    }
}

#[test]
fn test_image_fixtures_quality() {
    let base_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_DIR);

    for &name in NON_CORRUPT_FIXTURES {
        let path = base_dir.join(name);
        let orig_bytes =
            std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {name}: {e}"));

        let orig_insp = image_file::inspect(&orig_bytes)
            .unwrap_or_else(|e| panic!("failed to inspect original {name}: {e}"));

        let cleaned_bytes = image_file::clean(&orig_bytes)
            .unwrap_or_else(|e| panic!("failed to clean {name}: {e}"));

        let clean_insp = image_file::inspect(&cleaned_bytes)
            .unwrap_or_else(|e| panic!("failed to inspect cleaned {name}: {e}"));

        // 1. Decoded pixels are identical (first frame for animations)
        let orig_img = image::load_from_memory(&orig_bytes)
            .unwrap_or_else(|e| panic!("failed to decode original {name}: {e}"))
            .to_rgba8();
        let clean_img = image::load_from_memory(&cleaned_bytes)
            .unwrap_or_else(|e| panic!("failed to decode cleaned {name}: {e}"))
            .to_rgba8();
        assert_eq!(
            orig_img, clean_img,
            "decoded pixels mismatch for fixture {name}"
        );

        // 2. Kept orientation is preserved
        let orig_orient = orig_insp
            .kept
            .iter()
            .find(|k| matches!(k, KeptInfo::Orientation { .. }));
        let clean_orient = clean_insp
            .kept
            .iter()
            .find(|k| matches!(k, KeptInfo::Orientation { .. }));
        assert_eq!(
            orig_orient, clean_orient,
            "kept orientation mismatch for fixture {name}"
        );

        // 3. ICC profile bytes are identical
        let orig_icc = get_decoder_icc(orig_insp.format, &orig_bytes);
        let clean_icc = get_decoder_icc(clean_insp.format, &cleaned_bytes);
        assert_eq!(
            orig_icc, clean_icc,
            "decoder ICC profile mismatch for fixture {name}"
        );

        // 4. Cleaned output has empty kinds
        assert!(
            clean_insp.kinds.is_empty(),
            "expected clean output to have empty kinds for {name}, found {:?}",
            clean_insp.kinds
        );

        // 5. Raw image segments / chunks are identical
        match orig_insp.format {
            Format::Jpeg => {
                let seg1 = jpeg::parse(&orig_bytes)
                    .unwrap_or_else(|e| panic!("failed to parse original JPEG {name}: {e}"))
                    .image_segments();
                let seg2 = jpeg::parse(&cleaned_bytes)
                    .unwrap_or_else(|e| panic!("failed to parse cleaned JPEG {name}: {e}"))
                    .image_segments();
                assert_eq!(seg1, seg2, "JPEG image_segments mismatch for {name}");
            }
            Format::Png => {
                let chunks1 = png::parse(&orig_bytes)
                    .unwrap_or_else(|e| panic!("failed to parse original PNG {name}: {e}"))
                    .image_chunks();
                let chunks2 = png::parse(&cleaned_bytes)
                    .unwrap_or_else(|e| panic!("failed to parse cleaned PNG {name}: {e}"))
                    .image_chunks();
                assert_eq!(chunks1, chunks2, "PNG image_chunks mismatch for {name}");
            }
            Format::Webp => {
                let chunks1 = webp::parse(&orig_bytes)
                    .unwrap_or_else(|e| panic!("failed to parse original WebP {name}: {e}"))
                    .image_chunks();
                let chunks2 = webp::parse(&cleaned_bytes)
                    .unwrap_or_else(|e| panic!("failed to parse cleaned WebP {name}: {e}"))
                    .image_chunks();
                assert_eq!(chunks1, chunks2, "WebP image_chunks mismatch for {name}");
            }
            Format::Pdf => {}
        }
    }
}
