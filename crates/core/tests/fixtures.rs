//! The committed fixtures are exactly what `examples/gen_fixtures.rs` writes,
//! so that a change to the generator or to an encoder it uses cannot leave them
//! silently out of date (design §11.1).

use std::collections::BTreeSet;
use std::path::Path;

// The example's `main` is unused when it is compiled as a module of this test.
#[allow(dead_code)]
#[path = "../examples/gen_fixtures.rs"]
mod gen_fixtures;

#[test]
fn committed_fixtures_match_the_generator() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(gen_fixtures::FIXTURES_DIR);
    let generated = gen_fixtures::fixtures().expect("generating the fixtures in memory");

    let mut differing: Vec<&str> = generated
        .iter()
        .filter(|fixture| {
            std::fs::read(dir.join(&fixture.name)).ok().as_deref() != Some(&fixture.bytes[..])
        })
        .map(|fixture| fixture.name.as_str())
        .collect();
    let names: BTreeSet<&str> = generated
        .iter()
        .map(|fixture| fixture.name.as_str())
        .collect();
    let committed: Vec<String> = std::fs::read_dir(&dir)
        .expect("reading the fixtures directory")
        .map(|entry| {
            entry
                .expect("reading a fixtures entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    differing.extend(
        committed
            .iter()
            .map(String::as_str)
            .filter(|name| !names.contains(name)),
    );

    assert!(
        differing.is_empty(),
        "fixtures differ from the generator; run `cargo run -p mcleaner-core --example gen_fixtures` and commit: {differing:?}"
    );
}

#[test]
fn non_corrupt_images_can_be_read_by_image_crate() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(gen_fixtures::FIXTURES_DIR);
    let generated = gen_fixtures::fixtures().expect("generating the fixtures in memory");

    for fixture in &generated {
        let name = &fixture.name;
        if name.ends_with(".pdf") || name.starts_with("corrupt.") {
            continue;
        }
        let path = dir.join(name);
        let bytes = std::fs::read(&path).expect("reading fixture");
        let decoded = image::load_from_memory(&bytes);
        assert!(
            decoded.is_ok(),
            "expected {name} to be decoded by image crate, got: {:?}",
            decoded.err()
        );
    }
}

#[test]
fn corrupt_images_fail_to_decode_by_image_crate() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(gen_fixtures::FIXTURES_DIR);
    for name in ["corrupt.jpg", "corrupt.png", "corrupt.webp"] {
        let path = dir.join(name);
        let bytes = std::fs::read(&path).expect("reading corrupt fixture");
        let decoded = image::load_from_memory(&bytes);
        assert!(
            decoded.is_err(),
            "expected {name} to fail decoding, but it succeeded"
        );
    }
}

/// The MPF entry of full.jpg must point at the image after EOI, as a camera's
/// would, so that the cleaner is tested on a real multi-picture file.
#[test]
fn full_jpg_mpf_points_at_the_second_image() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(gen_fixtures::FIXTURES_DIR);
    let jpeg = std::fs::read(dir.join("full.jpg")).expect("reading full.jpg");
    let marker = b"\xFF\xE2";
    let mpf = jpeg
        .windows(8)
        .position(|w| w.starts_with(marker) && &w[4..8] == b"MPF\0")
        .expect("full.jpg has an MPF segment");
    let tiff = mpf + 8;
    let le_u32 = |at: usize| u32::from_le_bytes(jpeg[at..at + 4].try_into().unwrap()) as usize;
    // The MP Index IFD follows the 8-byte TIFF header: a 2-byte count, then
    // 12-byte entries. MPEntry is the third entry, and its value offset is
    // the entry's last 4 bytes. Each MPEntry value is 16 bytes.
    let entries = tiff + le_u32(tiff + 8 + 2 + 2 * 12 + 8);
    let (first_size, second_size, second_offset) = (
        le_u32(entries + 4),
        le_u32(entries + 20),
        le_u32(entries + 24),
    );
    let second = tiff + second_offset;
    assert_eq!(&jpeg[first_size - 2..first_size], b"\xFF\xD9");
    assert_eq!(second, first_size);
    assert_eq!(&jpeg[second..second + 2], b"\xFF\xD8");
    assert_eq!(second + second_size, jpeg.len());
}
