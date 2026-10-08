//! PDF metadata removal with lopdf, and a fixture builder.

use lopdf::{dictionary, Dictionary, Document, IncrementalDocument, Object, ObjectId, Stream};

const XMP: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?><x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>Spike Author</dc:creator></rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end="w"?>"#;

/// Keys whose value is metadata wherever they appear.
const METADATA_KEYS: [&[u8]; 3] = [b"Metadata", b"PieceInfo", b"Thumb"];

fn xmp_stream() -> Stream {
    Stream::new(dictionary! {"Type" => "Metadata", "Subtype" => "XML"}, XMP.as_bytes().to_vec())
}

fn resolve_dict(doc: &Document, o: &Object) -> Dictionary {
    match o {
        Object::Reference(id) => doc.get_dictionary(*id).cloned().unwrap_or_default(),
        Object::Dictionary(d) => d.clone(),
        _ => Dictionary::new(),
    }
}

pub fn fixture(base: &[u8], jpeg: &[u8], (w, h): (u32, u32)) -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut doc = Document::load_mem(base).map_err(|e| e.to_string())?;
    let info = doc.add_object(dictionary! {
        "Author" => Object::string_literal("Spike Author"),
        "Title" => Object::string_literal("Spike Title"),
        "Creator" => Object::string_literal("SpikeCreator"),
        "Producer" => Object::string_literal("SpikeProducer"),
        "CreationDate" => Object::string_literal("D:20260102030405Z"),
    });
    doc.trailer.set("Info", info);
    let xmp = doc.add_object(xmp_stream());
    let root = doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(|e| e.to_string())?;
    doc.get_dictionary_mut(root).map_err(|e| e.to_string())?.set("Metadata", xmp);

    let page = *doc.get_pages().get(&1).ok_or("no page")?;
    let page_xmp = doc.add_object(xmp_stream());
    let img = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => w, "Height" => h,
            "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8, "Filter" => "DCTDecode",
        },
        jpeg.to_vec(),
    ));
    let content = doc.add_object(Stream::new(Dictionary::new(), b"q 120 0 0 90 20 20 cm /ImSpike Do Q".to_vec()));
    let page_dict = doc.get_dictionary(page).map_err(|e| e.to_string())?.clone();
    let mut res = page_dict.get(b"Resources").map(|o| resolve_dict(&doc, o)).unwrap_or_default();
    let mut xobjects = res.get(b"XObject").map(|o| resolve_dict(&doc, o)).unwrap_or_default();
    xobjects.set("ImSpike", img);
    res.set("XObject", xobjects);
    let mut contents = match page_dict.get(b"Contents") {
        Ok(Object::Array(a)) => a.clone(),
        Ok(o) => vec![o.clone()],
        Err(_) => vec![],
    };
    contents.push(content.into());
    let p = doc.get_dictionary_mut(page).map_err(|e| e.to_string())?;
    p.set("Resources", res);
    p.set("Contents", contents);
    p.set("Metadata", page_xmp);
    p.set("PieceInfo", dictionary! {"SpikeApp" => dictionary! {"Private" => Object::string_literal("SpikePrivate")}});
    let mut first = Vec::new();
    doc.save_to(&mut first).map_err(|e| e.to_string())?;

    // A second revision appended after the first: the old Info stays in the
    // file bytes even though no viewer shows it.
    let prev = Document::load_mem(&first).map_err(|e| e.to_string())?;
    let mut inc = IncrementalDocument::create_from(first.clone(), prev);
    inc.opt_clone_object_to_new_document(info).map_err(|e| e.to_string())?;
    inc.new_document
        .get_dictionary_mut(info)
        .map_err(|e| e.to_string())?
        .set("Author", Object::string_literal("Spike Editor"));
    let mut second = Vec::new();
    inc.save_to(&mut second).map_err(|e| e.to_string())?;
    Ok((first, second))
}

fn is_dct_only(d: &Dictionary) -> bool {
    match d.get(b"Filter") {
        Ok(Object::Name(n)) => n == b"DCTDecode",
        Ok(Object::Array(a)) => a.len() == 1 && matches!(&a[0], Object::Name(n) if n == b"DCTDecode"),
        _ => false,
    }
}

fn scrub(o: &mut Object, stats: &mut Stats) {
    match o {
        Object::Dictionary(d) => scrub_dict(d, stats),
        Object::Array(a) => a.iter_mut().for_each(|o| scrub(o, stats)),
        Object::Stream(s) => {
            scrub_dict(&mut s.dict, stats);
            if is_dct_only(&s.dict) {
                match crate::jpeg::strip(&s.content, false) {
                    Ok(out) => {
                        if out.len() != s.content.len() {
                            stats.jpegs += 1;
                        }
                        s.set_content(out);
                    }
                    Err(_) => stats.jpeg_errors += 1,
                }
            }
        }
        _ => {}
    }
}

fn scrub_dict(d: &mut Dictionary, stats: &mut Stats) {
    for k in METADATA_KEYS {
        if d.remove(k).is_some() {
            stats.keys += 1;
        }
    }
    for (_, v) in d.iter_mut() {
        scrub(v, stats);
    }
}

#[derive(Default, Debug)]
pub struct Stats {
    pub keys: usize,
    pub jpegs: usize,
    pub jpeg_errors: usize,
    pub pruned: usize,
}

fn is_signed(doc: &Document) -> bool {
    doc.objects.values().any(|o| {
        let d = match o {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            _ => return false,
        };
        let name = |k: &[u8]| d.get(k).and_then(Object::as_name).ok() == Some(b"Sig".as_slice());
        name(b"Type") || name(b"FT")
    })
}

pub fn strip(d: &[u8]) -> Result<(Vec<u8>, Stats), String> {
    let mut doc = Document::load_mem(d).map_err(|e| format!("load: {e}"))?;
    if doc.was_encrypted() || doc.is_encrypted() {
        return Err("encrypted".into());
    }
    if is_signed(&doc) {
        return Err("signed".into());
    }
    let mut stats = Stats::default();
    doc.trailer.remove(b"Info");
    doc.trailer.remove(b"ID");
    let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    for id in ids {
        if let Some(o) = doc.objects.get_mut(&id) {
            scrub(o, &mut stats);
        }
    }
    stats.pruned = doc.prune_objects().len();
    let mut out = Vec::new();
    // Object streams keep the output from growing past the input when the
    // input used them.
    doc.save_modern(&mut out).map_err(|e| format!("save: {e}"))?;
    Ok((out, stats))
}

/// What is left that the cleaner promises to remove.
pub fn leftovers(d: &[u8], needles: &[&[u8]]) -> Result<Vec<String>, String> {
    let doc = Document::load_mem(d).map_err(|e| e.to_string())?;
    let mut found = Vec::new();
    if doc.trailer.has(b"Info") {
        found.push("trailer /Info".into());
    }
    for n in needles {
        if d.windows(n.len()).any(|w| w == *n) {
            found.push(format!("raw bytes contain {:?}", String::from_utf8_lossy(n)));
        }
    }
    for (id, o) in &doc.objects {
        let (dict, stream) = match o {
            Object::Dictionary(d) => (d, None),
            Object::Stream(s) => (&s.dict, Some(s)),
            _ => continue,
        };
        for k in METADATA_KEYS {
            if dict.has(k) {
                found.push(format!("{id:?} has /{}", String::from_utf8_lossy(k)));
            }
        }
        if let Some(s) = stream {
            if is_dct_only(&s.dict) {
                if let Ok(desc) = crate::jpeg::describe(&s.content) {
                    let desc: Vec<_> = desc.into_iter().filter(|c| c != "ICC(kept)").collect();
                    if !desc.is_empty() {
                        found.push(format!("{id:?} JPEG: {desc:?}"));
                    }
                }
            } else if let Ok(plain) = s.decompressed_content() {
                for n in needles {
                    if plain.windows(n.len()).any(|w| w == *n) {
                        found.push(format!("{id:?} stream contains {:?}", String::from_utf8_lossy(n)));
                    }
                }
            }
        }
    }
    Ok(found)
}
