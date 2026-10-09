//! Performance test for large JPEG inspection, cleaning, and verification (design §11.2).

use std::path::Path;
use std::time::Instant;

use mcleaner_core::image_file;
use mcleaner_core::jpeg;

const FIXTURE_FULL_JPG: &str = "tests/fixtures/full.jpg";

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance test only runs in release mode"
)]
fn test_large_jpeg_performance() {
    let width = 4000u32;
    let height = 3000u32;

    // Generate pseudo-pattern pixel data (~36 MB raw RGB)
    let mut pixels = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            let r = ((x ^ y) & 0xFF) as u8;
            let g = ((x.wrapping_mul(3) ^ y) & 0xFF) as u8;
            let b = ((x ^ y.wrapping_mul(5)) & 0xFF) as u8;
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
        }
    }

    let mut base_jpeg = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut base_jpeg, 75);
    encoder
        .encode(
            &pixels,
            width as u16,
            height as u16,
            jpeg_encoder::ColorType::Rgb,
        )
        .expect("encode large jpeg");

    // Read full.jpg segments
    let full_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_FULL_JPG);
    let full_bytes = std::fs::read(&full_path).expect("read full.jpg");
    let full_jpeg = jpeg::parse(&full_bytes).expect("parse full.jpg");

    // Collect APPn and COM segments from full.jpg
    let mut metadata_segments_bytes = Vec::new();
    for (marker, payload) in full_jpeg.dropped_segments() {
        metadata_segments_bytes.extend_from_slice(&[0xFF, marker]);
        let len = (payload.len() + 2) as u16;
        metadata_segments_bytes.extend_from_slice(&len.to_be_bytes());
        metadata_segments_bytes.extend_from_slice(payload);
    }

    // Splice APPn and COM segments immediately after SOI
    assert!(base_jpeg.starts_with(&[0xFF, 0xD8]));
    let mut test_jpeg = Vec::with_capacity(base_jpeg.len() + metadata_segments_bytes.len());
    test_jpeg.extend_from_slice(&[0xFF, 0xD8]);
    test_jpeg.extend_from_slice(&metadata_segments_bytes);
    test_jpeg.extend_from_slice(&base_jpeg[2..]);

    // Measure inspect + clean + inspect(clean) + compare image_segments()
    let start = Instant::now();

    let insp1 = image_file::inspect(&test_jpeg).expect("inspect test_jpeg");
    let cleaned = image_file::clean(&test_jpeg).expect("clean test_jpeg");
    let insp2 = image_file::inspect(&cleaned).expect("inspect cleaned");

    let segs1 = jpeg::parse(&test_jpeg)
        .expect("parse test_jpeg")
        .image_segments();
    let segs2 = jpeg::parse(&cleaned)
        .expect("parse cleaned")
        .image_segments();
    assert_eq!(segs1, segs2);
    assert!(!insp1.kinds.is_empty());
    assert!(insp2.kinds.is_empty());

    let elapsed = start.elapsed();
    eprintln!(
        "Large JPEG ({} bytes) inspect+clean+verify elapsed: {:?}",
        test_jpeg.len(),
        elapsed
    );

    assert!(
        elapsed.as_secs_f64() <= 2.0,
        "performance test exceeded 2.0s limit: {:?}",
        elapsed
    );
}
