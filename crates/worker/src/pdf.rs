//! PDF metadata inspection, detail extraction, and stripping with lopdf (design §4.6, §4.7).

use std::collections::HashSet;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use mcleaner_core::detect::Format;
use mcleaner_core::report::{
    self, DetailEntry, DetailValue, Details, Field, Inspection, MetadataKind,
};

/// Errors encountered while opening or processing a PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfError {
    OpenFailed,
    Encrypted,
    Signed,
}

impl PdfError {
    /// Returns the stable error code string (design §6.6).
    pub fn code(&self) -> &'static str {
        match self {
            Self::OpenFailed => "PdfOpenFailed",
            Self::Encrypted => "PdfEncrypted",
            Self::Signed => "PdfSigned",
        }
    }
}

/// Upper limit for decompressing XMP metadata streams (design §4.6).
pub const MAX_XMP_BYTES: usize = 4 * 1024 * 1024;

const METADATA_KEYS: [&[u8]; 3] = [b"Metadata", b"PieceInfo", b"Thumb"];

/// Inspects a PDF and returns found metadata kinds.
pub fn inspect(bytes: &[u8]) -> Result<Inspection, PdfError> {
    let doc = open_doc(bytes)?;
    let raw_entries = collect_raw_entries(&doc, bytes);

    let mut kinds: Vec<MetadataKind> = raw_entries.iter().map(|(k, _)| *k).collect();
    kinds.sort();
    kinds.dedup();

    Ok(Inspection {
        format: Format::Pdf,
        kinds,
        kept: Vec::new(),
    })
}

/// Inspects a PDF and returns structured detail groups.
pub fn details(bytes: &[u8]) -> Result<Details, PdfError> {
    let doc = open_doc(bytes)?;
    let raw_entries = collect_raw_entries(&doc, bytes);
    Ok(report::build_details(raw_entries, Vec::new()))
}

/// Strips metadata from a PDF and returns the cleaned PDF bytes.
pub fn clean(bytes: &[u8]) -> Result<Vec<u8>, PdfError> {
    let mut doc = open_doc(bytes)?;

    doc.trailer.remove(b"Info");
    doc.trailer.remove(b"ID");

    let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    for id in ids {
        if let Some(o) = doc.objects.get_mut(&id) {
            scrub_object(o);
        }
    }
    doc.prune_objects();

    let mut out = Vec::new();
    doc.save_modern(&mut out)
        .map_err(|_| PdfError::OpenFailed)?;

    // Verify lopdf did not re-add /Info or /ID
    if let Ok(reloaded) = Document::load_mem(&out)
        && (reloaded.trailer.has(b"Info") || reloaded.trailer.has(b"ID"))
    {
        return Err(PdfError::OpenFailed);
    }

    Ok(out)
}

fn open_doc(bytes: &[u8]) -> Result<Document, PdfError> {
    let doc = Document::load_mem(bytes).map_err(|_| PdfError::OpenFailed)?;
    if doc.was_encrypted() || doc.is_encrypted() {
        return Err(PdfError::Encrypted);
    }
    if is_signed(&doc) {
        return Err(PdfError::Signed);
    }
    Ok(doc)
}

fn is_signed(doc: &Document) -> bool {
    let has_sig_object = doc.objects.values().any(|o| {
        let d = match o {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            _ => return false,
        };
        let is_sig = |k: &[u8]| d.get(k).and_then(Object::as_name).ok() == Some(b"Sig".as_slice());
        is_sig(b"Type") || is_sig(b"FT")
    });
    if has_sig_object {
        return true;
    }

    if let Some(catalog) = get_catalog(doc) {
        if catalog.has(b"Perms") {
            return true;
        }
        if let Ok(acro_form_obj) = catalog.get(b"AcroForm") {
            let acro_form_dict = match acro_form_obj {
                Object::Reference(id) => doc.get_dictionary(*id).ok(),
                Object::Dictionary(d) => Some(d),
                _ => None,
            };
            if let Some(acro_form) = acro_form_dict
                && let Ok(Object::Integer(flags)) = acro_form.get(b"SigFlags")
                && *flags != 0
            {
                return true;
            }
        }
    }

    false
}

fn get_catalog(doc: &Document) -> Option<&Dictionary> {
    match doc.trailer.get(b"Root").ok()? {
        Object::Reference(id) => doc.get_dictionary(*id).ok(),
        Object::Dictionary(d) => Some(d),
        _ => None,
    }
}

fn is_dct_only(d: &Dictionary) -> bool {
    match d.get(b"Filter") {
        Ok(Object::Name(n)) => n == b"DCTDecode",
        Ok(Object::Array(a)) => {
            a.len() == 1 && matches!(&a[0], Object::Name(n) if n == b"DCTDecode")
        }
        _ => false,
    }
}

fn scrub_dict(d: &mut Dictionary) {
    for k in METADATA_KEYS {
        d.remove(k);
    }
    for (_, v) in d.iter_mut() {
        scrub_object(v);
    }
}

fn scrub_object(o: &mut Object) {
    match o {
        Object::Dictionary(d) => scrub_dict(d),
        Object::Array(a) => {
            for item in a.iter_mut() {
                scrub_object(item);
            }
        }
        Object::Stream(s) => {
            scrub_dict(&mut s.dict);
            if is_dct_only(&s.dict)
                && let Ok(parsed) = mcleaner_core::jpeg::parse(&s.content)
            {
                let stripped = mcleaner_core::jpeg::strip(&parsed, None);
                s.set_content(stripped);
            }
        }
        _ => {}
    }
}

fn parse_pdf_string(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (chunks, _) = bytes[2..].as_chunks::<2>();
        let u16_pairs = chunks.iter().map(|chunk| u16::from_be_bytes(*chunk));
        char::decode_utf16(u16_pairs)
            .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect()
    } else if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&bytes[3..]).into_owned()
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

fn parse_pdf_date(bytes: &[u8]) -> String {
    let s = parse_pdf_string(bytes);
    if let Some(rest) = s.strip_prefix("D:") {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        match digits.len() {
            0..=3 => s,
            4..=5 => digits[..4].to_string(),
            6..=7 => format!("{}-{}", &digits[..4], &digits[4..6]),
            8..=9 => format!("{}-{}-{}", &digits[..4], &digits[4..6], &digits[6..8]),
            10..=11 => format!(
                "{}-{}-{} {}",
                &digits[..4],
                &digits[4..6],
                &digits[6..8],
                &digits[8..10]
            ),
            12..=13 => format!(
                "{}-{}-{} {}:{}",
                &digits[..4],
                &digits[4..6],
                &digits[6..8],
                &digits[8..10],
                &digits[10..12]
            ),
            _ => format!(
                "{}-{}-{} {}:{}:{}",
                &digits[..4],
                &digits[4..6],
                &digits[6..8],
                &digits[8..10],
                &digits[10..12],
                &digits[12..14]
            ),
        }
    } else {
        s
    }
}

fn object_to_text(obj: &Object) -> String {
    match obj {
        Object::String(bytes, _) => parse_pdf_string(bytes),
        _ => format!("{obj:?}"),
    }
}

fn resolve_stream<'a>(
    doc: &'a Document,
    obj: &'a Object,
) -> Option<(Option<ObjectId>, &'a Stream)> {
    match obj {
        Object::Reference(id) => {
            let target = doc.get_object(*id).ok()?;
            let stream = target.as_stream().ok()?;
            Some((Some(*id), stream))
        }
        Object::Stream(stream) => Some((None, stream)),
        _ => None,
    }
}

fn resolve_dict<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Dictionary> {
    match obj {
        Object::Reference(id) => doc.get_dictionary(*id).ok(),
        Object::Dictionary(d) => Some(d),
        _ => None,
    }
}

fn collect_from_dict(
    doc: &Document,
    dict: &Dictionary,
    seen_streams: &mut HashSet<ObjectId>,
    raw_entries: &mut Vec<(MetadataKind, DetailEntry)>,
) {
    for (k, v) in dict.iter() {
        match k.as_slice() {
            b"Metadata" => {
                if let Some((opt_id, stream)) = resolve_stream(doc, v) {
                    let should_process = match opt_id {
                        Some(id) => seen_streams.insert(id),
                        None => true,
                    };
                    if should_process {
                        match stream.decompressed_content_with_limit(MAX_XMP_BYTES) {
                            Ok(xmp) => {
                                let kinds = mcleaner_core::xmp::classify(&xmp);
                                for kind in kinds {
                                    raw_entries.push((
                                        kind,
                                        DetailEntry {
                                            field: Field::Xmp,
                                            name: None,
                                            value: DetailValue::Bytes(xmp.len() as u64),
                                        },
                                    ));
                                }
                            }
                            Err(_) => {
                                raw_entries.push((
                                    MetadataKind::Other,
                                    DetailEntry {
                                        field: Field::Xmp,
                                        name: None,
                                        value: DetailValue::Bytes(stream.content.len() as u64),
                                    },
                                ));
                            }
                        }
                    }
                }
            }
            b"PieceInfo" => {
                if let Some(piece_dict) = resolve_dict(doc, v) {
                    let keys: Vec<String> = piece_dict
                        .iter()
                        .map(|(key, _)| String::from_utf8_lossy(key).into_owned())
                        .collect();
                    let joined = keys.join(", ");
                    raw_entries.push((
                        MetadataKind::Other,
                        DetailEntry {
                            field: Field::Other,
                            name: Some("PieceInfo".to_string()),
                            value: DetailValue::Text(joined),
                        },
                    ));
                }
            }
            b"Thumb" => {
                if let Some((opt_id, stream)) = resolve_stream(doc, v) {
                    let should_process = match opt_id {
                        Some(id) => seen_streams.insert(id),
                        None => true,
                    };
                    if should_process {
                        let w = stream.dict.get(b"Width").and_then(Object::as_i64);
                        let h = stream.dict.get(b"Height").and_then(Object::as_i64);
                        let value = match (w, h) {
                            (Ok(w), Ok(h)) => DetailValue::Text(format!("{w} × {h} px")),
                            _ => DetailValue::Bytes(stream.content.len() as u64),
                        };
                        raw_entries.push((
                            MetadataKind::Thumbnail,
                            DetailEntry {
                                field: Field::Thumbnail,
                                name: None,
                                value,
                            },
                        ));
                    }
                }
            }
            _ => match v {
                Object::Dictionary(nested_dict) => {
                    collect_from_dict(doc, nested_dict, seen_streams, raw_entries);
                }
                Object::Array(arr) => {
                    for item in arr {
                        if let Object::Dictionary(nested_dict) = item {
                            collect_from_dict(doc, nested_dict, seen_streams, raw_entries);
                        }
                    }
                }
                _ => {}
            },
        }
    }
}

fn collect_jpeg_details(content: &[u8], raw_entries: &mut Vec<(MetadataKind, DetailEntry)>) {
    if mcleaner_core::jpeg::parse(content).is_err() {
        return;
    }
    if let Ok(details) = mcleaner_core::image_file::details(content) {
        for group in details.groups {
            for entry in group.entries {
                raw_entries.push((group.kind, entry));
            }
        }
    }
}

fn is_first_object_linearized(doc: &Document) -> bool {
    if let Some((_id, Object::Dictionary(d))) = doc.objects.iter().next() {
        d.has(b"Linearized")
    } else {
        false
    }
}

fn collect_raw_entries(doc: &Document, bytes: &[u8]) -> Vec<(MetadataKind, DetailEntry)> {
    let mut raw_entries = Vec::new();

    // 1. Trailer /Info
    if let Ok(info_obj) = doc.trailer.get(b"Info") {
        let info_dict = match info_obj {
            Object::Reference(id) => doc.get_dictionary(*id).ok(),
            Object::Dictionary(d) => Some(d),
            _ => None,
        };
        if let Some(info) = info_dict {
            for (k, v) in info.iter() {
                match k.as_slice() {
                    b"Title" => raw_entries.push((
                        MetadataKind::Comment,
                        DetailEntry {
                            field: Field::Title,
                            name: None,
                            value: DetailValue::Text(object_to_text(v)),
                        },
                    )),
                    b"Author" => raw_entries.push((
                        MetadataKind::Author,
                        DetailEntry {
                            field: Field::Author,
                            name: None,
                            value: DetailValue::Text(object_to_text(v)),
                        },
                    )),
                    b"Subject" => raw_entries.push((
                        MetadataKind::Comment,
                        DetailEntry {
                            field: Field::Subject,
                            name: None,
                            value: DetailValue::Text(object_to_text(v)),
                        },
                    )),
                    b"Keywords" => raw_entries.push((
                        MetadataKind::Comment,
                        DetailEntry {
                            field: Field::Keywords,
                            name: None,
                            value: DetailValue::Text(object_to_text(v)),
                        },
                    )),
                    b"Creator" => raw_entries.push((
                        MetadataKind::Software,
                        DetailEntry {
                            field: Field::Software,
                            name: None,
                            value: DetailValue::Text(object_to_text(v)),
                        },
                    )),
                    b"Producer" => raw_entries.push((
                        MetadataKind::Software,
                        DetailEntry {
                            field: Field::PdfProducer,
                            name: None,
                            value: DetailValue::Text(object_to_text(v)),
                        },
                    )),
                    b"CreationDate" => {
                        let date_str = match v {
                            Object::String(b, _) => parse_pdf_date(b),
                            _ => object_to_text(v),
                        };
                        raw_entries.push((
                            MetadataKind::DateTime,
                            DetailEntry {
                                field: Field::Created,
                                name: None,
                                value: DetailValue::Text(date_str),
                            },
                        ));
                    }
                    b"ModDate" => {
                        let date_str = match v {
                            Object::String(b, _) => parse_pdf_date(b),
                            _ => object_to_text(v),
                        };
                        raw_entries.push((
                            MetadataKind::DateTime,
                            DetailEntry {
                                field: Field::Modified,
                                name: None,
                                value: DetailValue::Text(date_str),
                            },
                        ));
                    }
                    other_key => {
                        let key_name = String::from_utf8_lossy(other_key).into_owned();
                        raw_entries.push((
                            MetadataKind::Other,
                            DetailEntry {
                                field: Field::Other,
                                name: Some(key_name),
                                value: DetailValue::Text(object_to_text(v)),
                            },
                        ));
                    }
                }
            }
        }
    }

    // 2. Trailer /ID
    if let Ok(id_obj) = doc.trailer.get(b"ID") {
        let id_bytes_len = match id_obj {
            Object::Array(arr) => {
                let mut sum = 0u64;
                for item in arr {
                    if let Object::String(s, _) = item {
                        sum += s.len() as u64;
                    }
                }
                sum
            }
            Object::String(s, _) => s.len() as u64,
            _ => 0u64,
        };
        raw_entries.push((
            MetadataKind::Other,
            DetailEntry {
                field: Field::Other,
                name: Some("ID".to_string()),
                value: DetailValue::Bytes(id_bytes_len),
            },
        ));
    }

    // 3. Objects in ascending order
    let mut seen_streams = HashSet::new();
    for (obj_id, obj) in &doc.objects {
        match obj {
            Object::Dictionary(d) => {
                collect_from_dict(doc, d, &mut seen_streams, &mut raw_entries);
            }
            Object::Stream(s) => {
                collect_from_dict(doc, &s.dict, &mut seen_streams, &mut raw_entries);
                if is_dct_only(&s.dict) && seen_streams.insert(*obj_id) {
                    collect_jpeg_details(&s.content, &mut raw_entries);
                }
            }
            Object::Array(arr) => {
                for item in arr {
                    if let Object::Dictionary(nested_dict) = item {
                        collect_from_dict(doc, nested_dict, &mut seen_streams, &mut raw_entries);
                    }
                }
            }
            _ => {}
        }
    }

    // 4. Earlier versions from startxref count
    let startxref_count = bytes.windows(9).filter(|w| *w == b"startxref").count();

    if startxref_count >= 2 {
        let is_linearized = startxref_count == 2 && is_first_object_linearized(doc);
        if !is_linearized {
            raw_entries.push((
                MetadataKind::History,
                DetailEntry {
                    field: Field::EarlierVersions,
                    name: None,
                    value: DetailValue::Count((startxref_count - 1) as u32),
                },
            ));
        }
    }

    raw_entries
}

#[cfg(test)]
mod fixture_values {
    #![allow(dead_code)]
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../core/fixture_values.rs"
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../core/tests/fixtures");

    fn read_fixture(name: &str) -> Vec<u8> {
        let path = format!("{FIXTURES_DIR}/{name}");
        std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path}: {e}"))
    }

    #[test]
    fn test_reject_encrypted() {
        for fixture in &["encrypted.pdf", "restricted.pdf"] {
            let bytes = read_fixture(fixture);
            assert_eq!(inspect(&bytes), Err(PdfError::Encrypted));
            assert_eq!(details(&bytes), Err(PdfError::Encrypted));
            assert_eq!(clean(&bytes), Err(PdfError::Encrypted));
        }
    }

    #[test]
    fn test_reject_signed() {
        let bytes = read_fixture("signed.pdf");
        assert_eq!(inspect(&bytes), Err(PdfError::Signed));
        assert_eq!(details(&bytes), Err(PdfError::Signed));
        assert_eq!(clean(&bytes), Err(PdfError::Signed));
    }

    #[test]
    fn test_reject_corrupt() {
        let bytes = read_fixture("corrupt.pdf");
        assert_eq!(inspect(&bytes), Err(PdfError::OpenFailed));
        assert_eq!(details(&bytes), Err(PdfError::OpenFailed));
        assert_eq!(clean(&bytes), Err(PdfError::OpenFailed));
    }

    #[test]
    fn test_error_code_strings() {
        assert_eq!(PdfError::OpenFailed.code(), "PdfOpenFailed");
        assert_eq!(PdfError::Encrypted.code(), "PdfEncrypted");
        assert_eq!(PdfError::Signed.code(), "PdfSigned");
    }

    #[test]
    fn test_signature_detection_three_conditions() {
        // Condition 1: /Type /Sig or /FT /Sig object
        {
            let mut doc = Document::with_version("1.4");
            let sig_dict = doc.add_object(dictionary! {
                "Type" => "Sig",
                "Filter" => "Adobe.PPKLite",
            });
            let pages = doc.add_object(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![],
                "Count" => 0,
            });
            let root = doc.add_object(dictionary! {
                "Type" => "Catalog",
                "Pages" => pages,
                "SigRef" => sig_dict,
            });
            doc.trailer.set("Root", root);
            let mut bytes = Vec::new();
            doc.save_to(&mut bytes).unwrap();

            assert_eq!(inspect(&bytes), Err(PdfError::Signed));
        }

        // Condition 2: Catalog has /Perms
        {
            let mut doc = Document::with_version("1.4");
            let pages = doc.add_object(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![],
                "Count" => 0,
            });
            let root = doc.add_object(dictionary! {
                "Type" => "Catalog",
                "Pages" => pages,
                "Perms" => dictionary! { "DocMDP" => dictionary! {} },
            });
            doc.trailer.set("Root", root);
            let mut bytes = Vec::new();
            doc.save_to(&mut bytes).unwrap();

            assert_eq!(inspect(&bytes), Err(PdfError::Signed));
        }

        // Condition 3: Catalog has /AcroForm with non-zero /SigFlags
        {
            let mut doc = Document::with_version("1.4");
            let pages = doc.add_object(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![],
                "Count" => 0,
            });
            let acro = doc.add_object(dictionary! {
                "SigFlags" => 1,
            });
            let root = doc.add_object(dictionary! {
                "Type" => "Catalog",
                "Pages" => pages,
                "AcroForm" => acro,
            });
            doc.trailer.set("Root", root);
            let mut bytes = Vec::new();
            doc.save_to(&mut bytes).unwrap();

            assert_eq!(inspect(&bytes), Err(PdfError::Signed));
        }

        // SigFlags 0 is not rejected
        {
            let mut doc = Document::with_version("1.4");
            let pages = doc.add_object(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![],
                "Count" => 0,
            });
            let acro = doc.add_object(dictionary! {
                "SigFlags" => 0,
            });
            let root = doc.add_object(dictionary! {
                "Type" => "Catalog",
                "Pages" => pages,
                "AcroForm" => acro,
            });
            doc.trailer.set("Root", root);
            let mut bytes = Vec::new();
            doc.save_to(&mut bytes).unwrap();

            assert!(inspect(&bytes).is_ok());
        }
    }

    #[test]
    fn test_clean_full_pdf_removals() {
        let bytes = read_fixture("full.pdf");
        let cleaned = clean(&bytes).expect("clean must succeed for full.pdf");

        let doc = Document::load_mem(&cleaned).expect("cleaned PDF must be loadable");
        // 1. Trailer has no /Info and no /ID
        assert!(!doc.trailer.has(b"Info"), "trailer must not have /Info");
        assert!(!doc.trailer.has(b"ID"), "trailer must not have /ID");

        // 2. No dictionary has /Metadata, /PieceInfo, or /Thumb (including nested)
        for obj in doc.objects.values() {
            check_no_metadata_keys(obj);
        }

        // 3. DCTDecode image has no APPn (except JFIF/ICC/Adobe) and no COM
        for obj in doc.objects.values() {
            if let Object::Stream(s) = obj
                && is_dct_only(&s.dict)
            {
                let parsed =
                    mcleaner_core::jpeg::parse(&s.content).expect("DCT stream must be valid JPEG");
                let dropped = parsed.dropped_segments();
                for (marker, payload) in dropped {
                    if marker == 0xFE {
                        panic!("cleaned JPEG must not contain COM segment");
                    }
                    if marker == 0xE0 && payload.starts_with(b"JFIF\0") {
                        continue;
                    }
                    if marker == 0xE2 && payload.starts_with(b"ICC_PROFILE\0") {
                        continue;
                    }
                    if marker == 0xEE && payload.starts_with(b"Adobe") {
                        continue;
                    }
                    panic!("cleaned JPEG must not contain unexpected segment 0x{marker:02X}");
                }
            }
        }
    }

    fn check_no_metadata_keys(obj: &Object) {
        match obj {
            Object::Dictionary(d) => {
                for k in METADATA_KEYS {
                    assert!(
                        !d.has(k),
                        "dictionary has prohibited key: {:?}",
                        String::from_utf8_lossy(k)
                    );
                }
                for (_, v) in d.iter() {
                    check_no_metadata_keys(v);
                }
            }
            Object::Array(arr) => {
                for item in arr {
                    check_no_metadata_keys(item);
                }
            }
            Object::Stream(s) => {
                for k in METADATA_KEYS {
                    assert!(
                        !s.dict.has(k),
                        "stream dictionary has prohibited key: {:?}",
                        String::from_utf8_lossy(k)
                    );
                }
                for (_, v) in s.dict.iter() {
                    check_no_metadata_keys(v);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn test_non_dct_and_corrupt_dct_streams_are_preserved() {
        let mut doc = Document::with_version("1.5");

        let multi_filter_content = b"fake multi filter image stream content".to_vec();
        let multi_filter_stream = doc.add_object(Stream::new(
            dictionary! {
                "Filter" => vec![Object::Name(b"FlateDecode".to_vec()), Object::Name(b"DCTDecode".to_vec())],
            },
            multi_filter_content.clone(),
        ));

        let corrupt_jpeg_content = b"not a real jpeg bytes".to_vec();
        let corrupt_stream = doc.add_object(Stream::new(
            dictionary! {
                "Filter" => "DCTDecode",
            },
            corrupt_jpeg_content.clone(),
        ));

        let xobjects = doc.add_object(dictionary! {
            "Im1" => multi_filter_stream,
            "Im2" => corrupt_stream,
        });
        let resources = doc.add_object(dictionary! {
            "XObject" => xobjects,
        });
        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Resources" => resources,
        });
        let pages = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page.into()],
            "Count" => 1,
        });
        let root = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages,
        });
        doc.trailer.set("Root", root);

        let mut bytes = Vec::new();
        doc.save_modern(&mut bytes).unwrap();

        let cleaned = clean(&bytes).unwrap();
        let reloaded = Document::load_mem(&cleaned).unwrap();

        let stream1 = reloaded
            .get_object(multi_filter_stream)
            .unwrap()
            .as_stream()
            .unwrap();
        assert_eq!(stream1.content, multi_filter_content);

        let stream2 = reloaded
            .get_object(corrupt_stream)
            .unwrap()
            .as_stream()
            .unwrap();
        assert_eq!(stream2.content, corrupt_jpeg_content);
    }

    #[test]
    fn test_inspect_and_details_full_pdf() {
        let bytes = read_fixture("full.pdf");
        let insp = inspect(&bytes).unwrap();

        let expected_kinds = vec![
            MetadataKind::Location,
            MetadataKind::DateTime,
            MetadataKind::Device,
            MetadataKind::Author,
            MetadataKind::Software,
            MetadataKind::Comment,
            MetadataKind::Thumbnail,
            MetadataKind::History,
            MetadataKind::Other,
        ];
        assert_eq!(insp.kinds, expected_kinds);
        assert!(insp.kept.is_empty());

        let det = details(&bytes).unwrap();
        assert!(det.kept.is_empty());

        // Check author in second version is EDITOR
        let author_group = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Author)
            .unwrap();
        let author_entry = author_group
            .entries
            .iter()
            .find(|e| e.field == Field::Author)
            .unwrap();
        assert_eq!(
            author_entry.value,
            DetailValue::Text(fixture_values::EDITOR.to_string())
        );

        // Check created date
        let dt_group = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::DateTime)
            .unwrap();
        let created_entry = dt_group
            .entries
            .iter()
            .find(|e| e.field == Field::Created)
            .unwrap();
        assert_eq!(
            created_entry.value,
            DetailValue::Text("2026-01-02 03:04:05".to_string())
        );

        // Check PdfProducer exists
        let sw_group = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Software)
            .unwrap();
        assert!(
            sw_group
                .entries
                .iter()
                .any(|e| e.field == Field::PdfProducer)
        );

        // Check EarlierVersions is Count(1)
        let hist_group = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::History)
            .unwrap();
        let hist_entry = hist_group
            .entries
            .iter()
            .find(|e| e.field == Field::EarlierVersions)
            .unwrap();
        assert_eq!(hist_entry.value, DetailValue::Count(1));

        // Check Thumbnail is 4 × 4 px
        let thumb_group = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Thumbnail)
            .unwrap();
        let thumb_entry = thumb_group
            .entries
            .iter()
            .find(|e| e.field == Field::Thumbnail)
            .unwrap();
        assert_eq!(thumb_entry.value, DetailValue::Text("4 × 4 px".to_string()));
    }

    #[test]
    fn test_linearized_pdf_no_history() {
        let bytes = read_fixture("linearized.pdf");
        let insp = inspect(&bytes).unwrap();
        assert!(insp.kinds.is_empty());
        let det = details(&bytes).unwrap();
        assert!(det.groups.is_empty());
    }

    #[test]
    fn test_oversized_xmp_stream_reported_as_other() {
        let mut doc = Document::with_version("1.4");
        let pages = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![],
            "Count" => 0,
        });

        // 5 MiB of repetitive XML compressed with FlateDecode
        let raw_xmp = vec![b'a'; 5 * 1024 * 1024];
        let mut stream = Stream::new(
            dictionary! {
                "Type" => "Metadata",
                "Subtype" => "XML",
            },
            raw_xmp,
        );
        stream.compress().unwrap();
        let compressed_len = stream.content.len() as u64;

        let xmp_stream = doc.add_object(stream);

        let root = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages,
            "Metadata" => xmp_stream,
        });
        doc.trailer.set("Root", root);

        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();

        let insp = inspect(&bytes).unwrap();
        assert_eq!(insp.kinds, vec![MetadataKind::Other]);

        let det = details(&bytes).unwrap();
        let other_group = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Other)
            .unwrap();
        let xmp_entry = other_group
            .entries
            .iter()
            .find(|e| e.field == Field::Xmp)
            .unwrap();
        assert_eq!(xmp_entry.value, DetailValue::Bytes(compressed_len));
    }

    #[test]
    fn test_shared_xmp_stream_counted_once() {
        let mut doc = Document::with_version("1.4");
        let xmp_content = b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:creator>Author</dc:creator></rdf:Description></rdf:RDF></x:xmpmeta>".to_vec();
        let xmp_stream = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "Metadata",
                "Subtype" => "XML",
            },
            xmp_content,
        ));

        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Metadata" => xmp_stream,
        });
        let pages = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page.into()],
            "Count" => 1,
        });
        let root = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages,
            "Metadata" => xmp_stream,
        });
        doc.trailer.set("Root", root);

        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();

        let det = details(&bytes).unwrap();
        let author_group = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Author)
            .unwrap();
        assert_eq!(author_group.entries.len(), 1);
    }

    #[test]
    fn test_pdf_date_parsing_formats() {
        assert_eq!(parse_pdf_date(b"D:2026"), "2026");
        assert_eq!(parse_pdf_date(b"D:202601"), "2026-01");
        assert_eq!(parse_pdf_date(b"D:20260102"), "2026-01-02");
        assert_eq!(parse_pdf_date(b"D:2026010203"), "2026-01-02 03");
        assert_eq!(parse_pdf_date(b"D:202601020304"), "2026-01-02 03:04");
        assert_eq!(parse_pdf_date(b"D:20260102030405"), "2026-01-02 03:04:05");
        assert_eq!(parse_pdf_date(b"D:20260102030405Z"), "2026-01-02 03:04:05");
        assert_eq!(
            parse_pdf_date(b"D:20260102030405+09'00'"),
            "2026-01-02 03:04:05"
        );
        assert_eq!(parse_pdf_date(b"D:99"), "D:99");
        assert_eq!(parse_pdf_date(b"invalid"), "invalid");
    }

    #[test]
    fn test_pdf_string_decoding() {
        // UTF-16BE
        let utf16 = vec![0xFE, 0xFF, 0x30, 0x42, 0x30, 0x44]; // "あい"
        assert_eq!(parse_pdf_string(&utf16), "あい");

        // UTF-8 with BOM
        let utf8_bom = vec![0xEF, 0xBB, 0xBF, 0xE3, 0x81, 0x82]; // "あ"
        assert_eq!(parse_pdf_string(&utf8_bom), "あ");

        // Latin-1 fallback
        let latin1 = b"Hello \xA9 2026"; // © is 0xA9 in Latin-1
        assert_eq!(parse_pdf_string(latin1), "Hello © 2026");
    }
}
