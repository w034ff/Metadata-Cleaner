//! Performance test for PDF cleaning (design §11.2, work-plan T06).

use std::path::Path;
use std::time::Instant;

use lopdf::{Document, Stream, dictionary};
use mcleaner_worker::pdf::clean;

const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../core/tests/fixtures");

fn read_fixture(name: &str) -> Vec<u8> {
    let path = Path::new(FIXTURES_DIR).join(name);
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}

fn build_100_page_pdf(jpeg_bytes: &[u8]) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let xmp_content = b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:creator>Page Author</dc:creator></rdf:Description></rdf:RDF></x:xmpmeta>".to_vec();

    let mut page_ids = Vec::new();

    // 100 pages, each with DCTDecode image and XMP
    for _ in 0..100 {
        let xmp_stream = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "Metadata",
                "Subtype" => "XML",
            },
            xmp_content.clone(),
        ));

        let img_stream = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 64,
                "Height" => 48,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
                "Filter" => "DCTDecode",
            },
            jpeg_bytes.to_vec(),
        ));

        let res = doc.add_object(dictionary! {
            "XObject" => dictionary! {
                "Im0" => img_stream,
            },
        });

        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "MediaBox" => vec![0.into(), 0.into(), 595.28.into(), 841.89.into()],
            "Resources" => res,
            "Metadata" => xmp_stream,
        });

        page_ids.push(page.into());
    }

    let pages = doc.add_object(dictionary! {
        "Type" => "Pages",
        "Kids" => page_ids,
        "Count" => 100,
    });

    let root = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages,
    });
    doc.trailer.set("Root", root);

    let mut out = Vec::new();
    doc.save_modern(&mut out)
        .expect("failed to save generated 100-page PDF");
    out
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance test only runs in release mode"
)]
fn test_pdf_cleaning_performance_100_pages() {
    let full_jpg = read_fixture("full.jpg");
    let pdf_bytes = build_100_page_pdf(&full_jpg);

    let start = Instant::now();
    let cleaned = clean(&pdf_bytes).expect("clean should succeed on 100-page PDF");
    let elapsed = start.elapsed();

    eprintln!(
        "PDF cleaning performance (100 pages): {:.2?} (input: {} bytes, output: {} bytes)",
        elapsed,
        pdf_bytes.len(),
        cleaned.len()
    );

    assert!(
        elapsed.as_secs_f64() < 5.0,
        "clean took too long: {:.2?} (limit: 5.0s)",
        elapsed
    );
}
