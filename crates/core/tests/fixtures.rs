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
