//! Tests that the PDF fixtures match the requirements of design §4.6 and §11.1
//! using lopdf, which is only available in the worker crate (design §1, §9).

use lopdf::{Document, Object};
use std::path::Path;

fn read_fixture(name: &str) -> Vec<u8> {
    let base = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../core/tests/fixtures"
    ));
    std::fs::read(base.join(name)).unwrap_or_else(|e| panic!("failed to read fixture {name}: {e}"))
}

#[test]
fn encrypted_pdf_is_encrypted() {
    let bytes = read_fixture("encrypted.pdf");
    let doc = Document::load_mem(&bytes).expect("loading encrypted.pdf");
    assert!(doc.is_encrypted(), "encrypted.pdf should be encrypted");
    assert!(
        !doc.was_encrypted(),
        "encrypted.pdf was not decrypted with empty password"
    );
}

#[test]
fn restricted_pdf_was_encrypted() {
    let bytes = read_fixture("restricted.pdf");
    let doc = Document::load_mem(&bytes).expect("loading restricted.pdf");
    assert!(
        doc.was_encrypted(),
        "restricted.pdf was originally encrypted"
    );
    assert!(
        !doc.is_encrypted(),
        "restricted.pdf should be decrypted with empty password"
    );
}

#[test]
fn valid_pdfs_have_one_page() {
    for name in ["full.pdf", "linearized.pdf", "signed.pdf"] {
        let bytes = read_fixture(name);
        let doc = Document::load_mem(&bytes).unwrap_or_else(|e| panic!("loading {name}: {e}"));
        assert_eq!(
            doc.get_pages().len(),
            1,
            "{name} should have exactly 1 page"
        );
    }
}

#[test]
fn full_pdf_second_version_has_editor_author() {
    let bytes = read_fixture("full.pdf");
    let doc = Document::load_mem(&bytes).expect("loading full.pdf");

    let info_ref = doc.trailer.get(b"Info").expect("Info in trailer");
    let info_dict = match info_ref {
        Object::Reference(id) => doc.get_dictionary(*id).expect("Info dictionary"),
        Object::Dictionary(dict) => dict,
        other => panic!("expected Info dictionary, got {other:?}"),
    };

    let author_obj = info_dict.get(b"Author").expect("Author in Info dict");
    let author_bytes = match author_obj {
        Object::String(bytes, _) => bytes.as_slice(),
        other => panic!("expected string author, got {other:?}"),
    };

    assert_eq!(
        std::str::from_utf8(author_bytes).expect("utf-8 author"),
        "Example Editor",
        "full.pdf should expose the updated author from the second incremental revision"
    );
}

#[test]
fn corrupt_pdf_fails_to_open() {
    let bytes = read_fixture("corrupt.pdf");
    let result = Document::load_mem(&bytes);
    assert!(result.is_err(), "corrupt.pdf should fail to load in lopdf");
}
