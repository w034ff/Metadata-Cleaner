//! High-level image inspection, detail extraction, and cleaning pipeline (design §4.2–§4.5, §4.7).

use crate::detect::{self, Format};
use crate::error::CoreError;
use crate::exif;
use crate::iptc;
use crate::jpeg;
use crate::png;
use crate::report::{
    self, DetailEntry, DetailValue, Details, Field, Inspection, KeptInfo, MetadataKind,
    ResolutionUnit,
};
use crate::webp;
use crate::xmp;

/// Inspects an image file and returns its format, found metadata kinds, and kept info.
pub fn inspect(bytes: &[u8]) -> Result<Inspection, CoreError> {
    let format = detect::detect(bytes)?;
    if format == Format::Pdf {
        return Err(CoreError::UnsupportedFormat);
    }

    let (raw_entries, kept) = collect_raw_entries_and_kept(format, bytes)?;

    let mut kinds: Vec<MetadataKind> = raw_entries.iter().map(|(k, _)| *k).collect();
    kinds.sort();
    kinds.dedup();

    Ok(Inspection {
        format,
        kinds,
        kept: report::sort_kept(kept),
    })
}

/// Inspects an image file and returns structured detail groups and kept info.
pub fn details(bytes: &[u8]) -> Result<Details, CoreError> {
    let format = detect::detect(bytes)?;
    if format == Format::Pdf {
        return Err(CoreError::UnsupportedFormat);
    }

    let (raw_entries, kept) = collect_raw_entries_and_kept(format, bytes)?;
    Ok(report::build_details(raw_entries, kept))
}

/// Cleans an image file by stripping metadata and preserving allowed tags.
pub fn clean(bytes: &[u8]) -> Result<Vec<u8>, CoreError> {
    let format = detect::detect(bytes)?;
    match format {
        Format::Jpeg => {
            let parsed = jpeg::parse(bytes)?;
            let kept_exif = parsed.exif().and_then(exif::kept_only);
            Ok(jpeg::strip(&parsed, kept_exif.as_deref()))
        }
        Format::Png => {
            let parsed = png::parse(bytes)?;
            let kept_exif = parsed.exif().and_then(exif::kept_only);
            Ok(png::strip(&parsed, kept_exif.as_deref()))
        }
        Format::Webp => {
            let parsed = webp::parse(bytes)?;
            let kept_exif = parsed.exif().and_then(exif::kept_only);
            Ok(webp::strip(&parsed, kept_exif.as_deref()))
        }
        Format::Pdf => Err(CoreError::UnsupportedFormat),
    }
}

type RawEntriesAndKept = (Vec<(MetadataKind, DetailEntry)>, Vec<KeptInfo>);

/// Collects all metadata entries in file order and extracts kept metadata.
fn collect_raw_entries_and_kept(
    format: Format,
    bytes: &[u8],
) -> Result<RawEntriesAndKept, CoreError> {
    match format {
        Format::Jpeg => collect_jpeg_entries_and_kept(bytes),
        Format::Png => collect_png_entries_and_kept(bytes),
        Format::Webp => collect_webp_entries_and_kept(bytes),
        Format::Pdf => Err(CoreError::UnsupportedFormat),
    }
}

/// Collects entries and kept info for a JPEG image.
fn collect_jpeg_entries_and_kept(bytes: &[u8]) -> Result<RawEntriesAndKept, CoreError> {
    let jpeg = jpeg::parse(bytes)?;
    let mut entries = Vec::new();
    let mut kept = Vec::new();

    // 1. Kept Orientation
    if let Some(tiff) = jpeg.exif()
        && let (Some(orient), _) = exif::parse_kept_info(tiff)
    {
        kept.push(KeptInfo::Orientation { value: orient });
    }

    // 2. Kept ColorProfile
    let icc_payloads = jpeg.icc_payloads();
    if !icc_payloads.is_empty() {
        let assembled_icc = assemble_jpeg_icc(&icc_payloads);
        let description = parse_icc_description(&assembled_icc);
        kept.push(KeptInfo::ColorProfile { description });
    }

    // 3. Kept Resolution
    let exif_res = jpeg.exif().and_then(|t| exif::parse_kept_info(t).1);
    if let Some(res) = exif_res {
        kept.push(res);
    } else if let Some(jfif) = jpeg.jfif_payload()
        && jfif.len() >= 14
        && jfif.starts_with(b"JFIF\0")
    {
        let unit_code = jfif[7];
        let x = u16::from_be_bytes([jfif[8], jfif[9]]) as u32;
        let y = u16::from_be_bytes([jfif[10], jfif[11]]) as u32;
        let unit = match unit_code {
            1 => Some(ResolutionUnit::Inch),
            2 => Some(ResolutionUnit::Centimeter),
            _ => None,
        };
        if let Some(u) = unit
            && x > 0
            && y > 0
        {
            kept.push(KeptInfo::Resolution { x, y, unit: u });
        }
    }

    // 4. Metadata entries from segments in file order
    for (marker, payload) in jpeg.dropped_segments() {
        match marker {
            0xE0 => {
                // APP0
                if payload.starts_with(b"JFIF\0") && payload.len() >= 14 {
                    let w = payload[12];
                    let h = payload[13];
                    if w > 0 && h > 0 {
                        entries.push((
                            MetadataKind::Thumbnail,
                            DetailEntry {
                                field: Field::Thumbnail,
                                name: None,
                                value: DetailValue::Text(format!("{w} × {h} px")),
                            },
                        ));
                    }
                } else if payload.starts_with(b"JFXX\0") {
                    entries.push((
                        MetadataKind::Thumbnail,
                        DetailEntry {
                            field: Field::Thumbnail,
                            name: None,
                            value: DetailValue::Bytes(payload.len() as u64),
                        },
                    ));
                } else {
                    entries.push((
                        MetadataKind::Other,
                        DetailEntry {
                            field: Field::Other,
                            name: Some("APP0".to_string()),
                            value: DetailValue::Bytes(payload.len() as u64),
                        },
                    ));
                }
            }
            0xE1 => {
                // APP1
                if let Some(tiff) = payload.strip_prefix(b"Exif\0\0") {
                    entries.extend(exif::collect_details(tiff));
                } else if let Some(xmp) = payload.strip_prefix(b"http://ns.adobe.com/xap/1.0/\0") {
                    entries.extend(xmp::xmp_detail_entries(xmp));
                } else if let Some(xmp) =
                    payload.strip_prefix(b"http://ns.adobe.com/xmp/extension/\0")
                {
                    entries.extend(xmp::xmp_detail_entries(xmp));
                } else {
                    entries.push((
                        MetadataKind::Other,
                        DetailEntry {
                            field: Field::Other,
                            name: Some("APP1".to_string()),
                            value: DetailValue::Bytes(payload.len() as u64),
                        },
                    ));
                }
            }
            0xE2 => {
                // APP2
                if payload.starts_with(b"ICC_PROFILE\0") {
                    // Kept ColorProfile, do not count
                } else if payload.starts_with(b"MPF\0") {
                    // If trailing data starts with FF D8 FF, only emit Thumbnail once from trailing
                    if !jpeg.trailing().starts_with(&[0xFF, 0xD8, 0xFF]) {
                        entries.push((
                            MetadataKind::Thumbnail,
                            DetailEntry {
                                field: Field::Thumbnail,
                                name: None,
                                value: DetailValue::Bytes(payload.len() as u64),
                            },
                        ));
                    }
                } else {
                    entries.push((
                        MetadataKind::Other,
                        DetailEntry {
                            field: Field::Other,
                            name: Some("APP2".to_string()),
                            value: DetailValue::Bytes(payload.len() as u64),
                        },
                    ));
                }
            }
            0xED => {
                // APP13 (Photoshop 3.0 / IPTC)
                if payload.starts_with(b"Photoshop 3.0\0") {
                    if let Some(iptc_data) = iptc::parse_app13_iptc(payload) {
                        entries.extend(iptc::collect_details(iptc_data));
                    } else {
                        entries.push((
                            MetadataKind::Other,
                            DetailEntry {
                                field: Field::Other,
                                name: Some("APP13".to_string()),
                                value: DetailValue::Bytes(payload.len() as u64),
                            },
                        ));
                    }
                } else {
                    entries.push((
                        MetadataKind::Other,
                        DetailEntry {
                            field: Field::Other,
                            name: Some("APP13".to_string()),
                            value: DetailValue::Bytes(payload.len() as u64),
                        },
                    ));
                }
            }
            0xEE => {
                // APP14 (Adobe)
                if payload.starts_with(b"Adobe") {
                    // Kept segment, do not count
                } else {
                    entries.push((
                        MetadataKind::Other,
                        DetailEntry {
                            field: Field::Other,
                            name: Some("APP14".to_string()),
                            value: DetailValue::Bytes(payload.len() as u64),
                        },
                    ));
                }
            }
            0xFE => {
                // COM
                entries.push((
                    MetadataKind::Comment,
                    DetailEntry {
                        field: Field::Comment,
                        name: None,
                        value: DetailValue::Text(String::from_utf8_lossy(payload).into_owned()),
                    },
                ));
            }
            _ => {
                // Other APPn
                let name = format!("APP{}", marker & 0x0F);
                entries.push((
                    MetadataKind::Other,
                    DetailEntry {
                        field: Field::Other,
                        name: Some(name),
                        value: DetailValue::Bytes(payload.len() as u64),
                    },
                ));
            }
        }
    }

    entries.extend(trailing_entry(jpeg.trailing()));

    Ok((entries, kept))
}

/// Collects entries and kept info for a PNG image.
fn collect_png_entries_and_kept(bytes: &[u8]) -> Result<RawEntriesAndKept, CoreError> {
    let png = png::parse(bytes)?;
    let mut entries = Vec::new();
    let mut kept = Vec::new();

    // 1. Kept Orientation
    if let Some(tiff) = png.exif()
        && let (Some(orient), _) = exif::parse_kept_info(tiff)
    {
        kept.push(KeptInfo::Orientation { value: orient });
    }

    // 2. Kept ColorProfile
    if png.iccp().is_some() {
        kept.push(KeptInfo::ColorProfile { description: None });
    }

    // 3. Kept Resolution
    let exif_res = png.exif().and_then(|t| exif::parse_kept_info(t).1);
    if let Some(res) = exif_res {
        kept.push(res);
    } else if let Some(phys) = png.phys()
        && phys.len() >= 9
    {
        let x = u32::from_be_bytes([phys[0], phys[1], phys[2], phys[3]]);
        let y = u32::from_be_bytes([phys[4], phys[5], phys[6], phys[7]]);
        let unit_code = phys[8];
        if unit_code == 1 && x > 0 && y > 0 {
            kept.push(KeptInfo::Resolution {
                x,
                y,
                unit: ResolutionUnit::Meter,
            });
        }
    }

    // 4. Metadata entries from dropped chunks in file order
    for (kind, data) in png.dropped_chunks() {
        match &kind {
            b"eXIf" => {
                entries.extend(exif::collect_details(data));
            }
            b"tEXt" => {
                if let Some((kw, text)) = parse_png_text_chunk(data) {
                    process_png_keyword(&kw, DetailValue::Text(text), &mut entries);
                } else {
                    entries.push((
                        MetadataKind::Other,
                        DetailEntry {
                            field: Field::Other,
                            name: Some("tEXt".to_string()),
                            value: DetailValue::Bytes(data.len() as u64),
                        },
                    ));
                }
            }
            b"zTXt" => {
                if let Some((kw, comp_bytes)) = parse_png_ztxt_chunk(data) {
                    process_png_keyword(
                        &kw,
                        DetailValue::Bytes(comp_bytes.len() as u64),
                        &mut entries,
                    );
                } else {
                    entries.push((
                        MetadataKind::Other,
                        DetailEntry {
                            field: Field::Other,
                            name: Some("zTXt".to_string()),
                            value: DetailValue::Bytes(data.len() as u64),
                        },
                    ));
                }
            }
            b"iTXt" => {
                if let Some((kw, comp_flag, text_data)) = parse_png_itxt_chunk(data) {
                    if kw == "XML:com.adobe.xmp" {
                        entries.extend(xmp::xmp_detail_entries(text_data));
                    } else {
                        let value = if comp_flag == 1 {
                            DetailValue::Bytes(text_data.len() as u64)
                        } else {
                            DetailValue::Text(String::from_utf8_lossy(text_data).into_owned())
                        };
                        process_png_keyword(&kw, value, &mut entries);
                    }
                } else {
                    entries.push((
                        MetadataKind::Other,
                        DetailEntry {
                            field: Field::Other,
                            name: Some("iTXt".to_string()),
                            value: DetailValue::Bytes(data.len() as u64),
                        },
                    ));
                }
            }
            b"tIME" => {
                if data.len() >= 7 {
                    let year = u16::from_be_bytes([data[0], data[1]]);
                    let val = format!(
                        "{year:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                        data[2], data[3], data[4], data[5], data[6]
                    );
                    entries.push((
                        MetadataKind::DateTime,
                        DetailEntry {
                            field: Field::Modified,
                            name: None,
                            value: DetailValue::Text(val),
                        },
                    ));
                } else {
                    entries.push((
                        MetadataKind::DateTime,
                        DetailEntry {
                            field: Field::Modified,
                            name: None,
                            value: DetailValue::Bytes(data.len() as u64),
                        },
                    ));
                }
            }
            _ => {
                let name = String::from_utf8_lossy(&kind).into_owned();
                entries.push((
                    MetadataKind::Other,
                    DetailEntry {
                        field: Field::Other,
                        name: Some(name),
                        value: DetailValue::Bytes(data.len() as u64),
                    },
                ));
            }
        }
    }

    entries.extend(trailing_entry(png.trailing()));

    Ok((entries, kept))
}

/// Collects entries and kept info for a WebP image.
fn collect_webp_entries_and_kept(bytes: &[u8]) -> Result<RawEntriesAndKept, CoreError> {
    let webp = webp::parse(bytes)?;
    let mut entries = Vec::new();
    let mut kept = Vec::new();

    // 1. Kept Orientation
    if let Some(tiff) = webp.exif()
        && let (Some(orient), _) = exif::parse_kept_info(tiff)
    {
        kept.push(KeptInfo::Orientation { value: orient });
    }

    // 2. Kept ColorProfile
    if let Some(icc_data) = webp.iccp() {
        let description = parse_icc_description(icc_data);
        kept.push(KeptInfo::ColorProfile { description });
    }

    // 3. Kept Resolution
    if let Some(tiff) = webp.exif()
        && let (_, Some(res)) = exif::parse_kept_info(tiff)
    {
        kept.push(res);
    }

    // 4. Metadata entries from dropped chunks in file order
    for (id, data) in webp.dropped_chunks() {
        match &id {
            b"EXIF" => {
                let tiff = data.strip_prefix(b"Exif\0\0").unwrap_or(data);
                entries.extend(exif::collect_details(tiff));
            }
            b"XMP " => {
                entries.extend(xmp::xmp_detail_entries(data));
            }
            _ => {
                let name = String::from_utf8_lossy(&id).into_owned();
                entries.push((
                    MetadataKind::Other,
                    DetailEntry {
                        field: Field::Other,
                        name: Some(name),
                        value: DetailValue::Bytes(data.len() as u64),
                    },
                ));
            }
        }
    }

    entries.extend(trailing_entry(webp.trailing()));

    Ok((entries, kept))
}

/// The entry for the bytes after the image: an appended image (an MPF image
/// or a motion photo's still) is a thumbnail, anything else is `Other`.
fn trailing_entry(trailing: &[u8]) -> Option<(MetadataKind, DetailEntry)> {
    if trailing.is_empty() {
        return None;
    }
    if trailing.starts_with(&[0xFF, 0xD8, 0xFF]) {
        let value = if let Ok(parsed) = jpeg::parse(trailing)
            && let Some((w, h)) = parsed.dimensions()
        {
            DetailValue::Text(format!("{w} × {h} px"))
        } else {
            DetailValue::Bytes(trailing.len() as u64)
        };
        return Some((
            MetadataKind::Thumbnail,
            DetailEntry {
                field: Field::Thumbnail,
                name: None,
                value,
            },
        ));
    }
    Some((
        MetadataKind::Other,
        DetailEntry {
            field: Field::Other,
            name: Some("trailing data".to_string()),
            value: DetailValue::Bytes(trailing.len() as u64),
        },
    ))
}

/// Dispatches a PNG text keyword to its category and field.
fn process_png_keyword(
    kw: &str,
    value: DetailValue,
    entries: &mut Vec<(MetadataKind, DetailEntry)>,
) {
    match kw {
        "Author" => entries.push((
            MetadataKind::Author,
            DetailEntry {
                field: Field::Author,
                name: None,
                value,
            },
        )),
        "Copyright" => entries.push((
            MetadataKind::Author,
            DetailEntry {
                field: Field::Copyright,
                name: None,
                value,
            },
        )),
        "Software" => entries.push((
            MetadataKind::Software,
            DetailEntry {
                field: Field::Software,
                name: None,
                value,
            },
        )),
        "Title" => entries.push((
            MetadataKind::Comment,
            DetailEntry {
                field: Field::Title,
                name: None,
                value,
            },
        )),
        "Description" => entries.push((
            MetadataKind::Comment,
            DetailEntry {
                field: Field::Description,
                name: None,
                value,
            },
        )),
        "Comment" => entries.push((
            MetadataKind::Comment,
            DetailEntry {
                field: Field::Comment,
                name: None,
                value,
            },
        )),
        "Creation Time" => {
            let val = match value {
                DetailValue::Text(t) => {
                    let formatted =
                        if t.len() >= 19 && t.as_bytes()[4] == b':' && t.as_bytes()[7] == b':' {
                            format!("{}-{}-{}", &t[..4], &t[5..7], &t[8..])
                        } else {
                            t
                        };
                    DetailValue::Text(formatted)
                }
                other => other,
            };
            entries.push((
                MetadataKind::DateTime,
                DetailEntry {
                    field: Field::Created,
                    name: None,
                    value: val,
                },
            ));
        }
        _ => entries.push((
            MetadataKind::Other,
            DetailEntry {
                field: Field::Other,
                name: Some(kw.to_string()),
                value,
            },
        )),
    }
}

/// Parses a `tEXt` chunk into `(keyword, text)`.
fn parse_png_text_chunk(data: &[u8]) -> Option<(String, String)> {
    let null_pos = data.iter().position(|&b| b == 0)?;
    let kw = String::from_utf8_lossy(&data[..null_pos]).into_owned();
    let text = String::from_utf8_lossy(&data[null_pos + 1..]).into_owned();
    Some((kw, text))
}

/// Parses a `zTXt` chunk into `(keyword, compressed_data)`.
fn parse_png_ztxt_chunk(data: &[u8]) -> Option<(String, &[u8])> {
    let null_pos = data.iter().position(|&b| b == 0)?;
    if null_pos + 2 > data.len() {
        return None;
    }
    let kw = String::from_utf8_lossy(&data[..null_pos]).into_owned();
    let comp_data = &data[null_pos + 2..];
    Some((kw, comp_data))
}

/// Parses an `iTXt` chunk into `(keyword, comp_flag, text_data)`.
fn parse_png_itxt_chunk(data: &[u8]) -> Option<(String, u8, &[u8])> {
    let null_pos = data.iter().position(|&b| b == 0)?;
    if null_pos + 3 > data.len() {
        return None;
    }
    let kw = String::from_utf8_lossy(&data[..null_pos]).into_owned();
    let comp_flag = data[null_pos + 1];
    let mut pos = null_pos + 3; // skip null, comp_flag, comp_method

    // Skip language tag
    let lang_null = data[pos..].iter().position(|&b| b == 0)?;
    pos += lang_null + 1;

    // Skip translated keyword
    let trans_null = data[pos..].iter().position(|&b| b == 0)?;
    pos += trans_null + 1;

    Some((kw, comp_flag, &data[pos..]))
}

/// Assembles multiple JPEG APP2 ICC chunks in sequence order.
fn assemble_jpeg_icc(payloads: &[&[u8]]) -> Vec<u8> {
    let mut chunks: Vec<(u8, &[u8])> = payloads
        .iter()
        .filter_map(|p| {
            if p.len() >= 14 && p.starts_with(b"ICC_PROFILE\0") {
                let seq = p[12];
                Some((seq, &p[14..]))
            } else {
                None
            }
        })
        .collect();

    chunks.sort_by_key(|&(seq, _)| seq);

    let mut out = Vec::new();
    for (_, data) in chunks {
        out.extend_from_slice(data);
    }
    out
}

/// Parses the `desc` text description from an ICC profile (ASCII textDescriptionType).
fn parse_icc_description(icc: &[u8]) -> Option<String> {
    if icc.len() < 132 {
        return None;
    }
    let tag_count = u32::from_be_bytes([icc[128], icc[129], icc[130], icc[131]]) as usize;
    let mut pos = 132;
    for _ in 0..tag_count {
        if pos + 12 > icc.len() {
            return None;
        }
        let tag_sig = &icc[pos..pos + 4];
        let offset =
            u32::from_be_bytes([icc[pos + 4], icc[pos + 5], icc[pos + 6], icc[pos + 7]]) as usize;
        let size =
            u32::from_be_bytes([icc[pos + 8], icc[pos + 9], icc[pos + 10], icc[pos + 11]]) as usize;
        pos += 12;

        if tag_sig == b"desc" {
            let end = offset.checked_add(size)?;
            if end > icc.len() || size < 12 {
                return None;
            }
            let payload = &icc[offset..end];
            if &payload[0..4] == b"desc" {
                let ascii_count =
                    u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]]) as usize;
                if 12 + ascii_count > payload.len() {
                    return None;
                }
                let mut ascii_bytes = &payload[12..12 + ascii_count];
                if let Some(stripped) = ascii_bytes.strip_suffix(b"\0") {
                    ascii_bytes = stripped;
                }
                return Some(String::from_utf8_lossy(ascii_bytes).into_owned());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::exif as kamadak;
    use kamadak::{Context, In, Tag, Value};

    fn make_png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(kind);
        hasher.update(data);
        out.extend_from_slice(&hasher.finalize().to_be_bytes());
        out
    }

    fn minimal_png(chunks: &[Vec<u8>], trailing: &[u8]) -> Vec<u8> {
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let ihdr_data = [
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00,
        ];
        out.extend_from_slice(&make_png_chunk(b"IHDR", &ihdr_data));
        for c in chunks {
            out.extend_from_slice(c);
        }
        out.extend_from_slice(&make_png_chunk(
            b"IDAT",
            &[
                0x78, 0x01, 0x01, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x01,
            ],
        ));
        out.extend_from_slice(&make_png_chunk(b"IEND", &[]));
        out.extend_from_slice(trailing);
        out
    }

    fn make_jpeg_seg(marker: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = vec![0xFF, marker];
        let len = (payload.len() + 2) as u16;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(payload);
        out
    }

    fn minimal_jpeg(segments: &[Vec<u8>], trailing: &[u8]) -> Vec<u8> {
        let mut out = vec![0xFF, 0xD8];
        for s in segments {
            out.extend_from_slice(s);
        }
        out.extend_from_slice(&[
            0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00,
        ]);
        out.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
        out.extend_from_slice(&[0xFF, 0xD9]);
        out.extend_from_slice(trailing);
        out
    }

    fn make_iptc_dataset(rec: u8, num: u8, data: &[u8]) -> Vec<u8> {
        let mut out = vec![0x1C, rec, num];
        out.extend_from_slice(&(data.len() as u16).to_be_bytes());
        out.extend_from_slice(data);
        out
    }

    fn make_iptc_app13(datasets: &[Vec<u8>]) -> Vec<u8> {
        let mut iptc = Vec::new();
        for d in datasets {
            iptc.extend_from_slice(d);
        }
        let mut bim = Vec::new();
        bim.extend_from_slice(b"8BIM\x04\x04\x00\x00");
        bim.extend_from_slice(&(iptc.len() as u32).to_be_bytes());
        bim.extend_from_slice(&iptc);
        if iptc.len() % 2 != 0 {
            bim.push(0);
        }
        let mut app13 = b"Photoshop 3.0\0".to_vec();
        app13.extend_from_slice(&bim);
        make_jpeg_seg(0xED, &app13)
    }

    #[test]
    fn test_inspect_and_details_kinds_match() {
        let full_jpg = std::fs::read("tests/fixtures/full.jpg").expect("read full.jpg");
        let insp = inspect(&full_jpg).expect("inspect full.jpg");
        let det = details(&full_jpg).expect("details full.jpg");

        let det_kinds: Vec<MetadataKind> = det.groups.iter().map(|g| g.kind).collect();
        assert_eq!(insp.kinds, det_kinds);
    }

    // --- Table row 1 to 32 tests ---

    #[test]
    fn test_table_row_01_exif_gps_coords() {
        let full_jpg = std::fs::read("tests/fixtures/full.jpg").expect("read full.jpg");
        let det = details(&full_jpg).expect("details full.jpg");
        let loc_group = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Location)
            .unwrap();

        let lat_entry = loc_group
            .entries
            .iter()
            .find(|e| e.field == Field::Latitude)
            .unwrap();
        assert_eq!(lat_entry.value, DetailValue::Text("12°34′0″ N".to_string()));

        let lon_entry = loc_group
            .entries
            .iter()
            .find(|e| e.field == Field::Longitude)
            .unwrap();
        assert_eq!(lon_entry.value, DetailValue::Text("65°43′0″ E".to_string()));
    }

    #[test]
    fn test_table_row_02_exif_gps_other() {
        let mut writer = kamadak::experimental::Writer::new();
        let field = kamadak::Field {
            tag: Tag(Context::Gps, 6), // GPSAltitude
            ifd_num: In::PRIMARY,
            value: Value::Rational(vec![kamadak::Rational { num: 100, denom: 1 }]),
        };
        writer.push_field(&field);
        let mut cur = std::io::Cursor::new(Vec::new());
        writer.write(&mut cur, true).unwrap();
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(&cur.into_inner());

        let jpg = minimal_jpeg(&[make_jpeg_seg(0xE1, &payload)], &[]);
        let det = details(&jpg).unwrap();
        let loc = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Location)
            .unwrap();
        assert!(loc.entries.iter().any(|e| e.field == Field::Other));
    }

    #[test]
    fn test_table_row_03_exif_datetime_taken_modified() {
        let full_jpg = std::fs::read("tests/fixtures/full.jpg").expect("read full.jpg");
        let det = details(&full_jpg).expect("details full.jpg");
        let dt_group = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::DateTime)
            .unwrap();

        let taken = dt_group
            .entries
            .iter()
            .find(|e| e.field == Field::Taken)
            .unwrap();
        assert_eq!(
            taken.value,
            DetailValue::Text("2026-01-02 03:04:05".to_string())
        );

        let modified = dt_group
            .entries
            .iter()
            .find(|e| e.field == Field::Modified)
            .unwrap();
        assert_eq!(
            modified.value,
            DetailValue::Text("2026-01-02 03:04:05".to_string())
        );
    }

    #[test]
    fn test_table_row_04_exif_datetime_other() {
        let mut writer = kamadak::experimental::Writer::new();
        let field = kamadak::Field {
            tag: Tag::DateTimeDigitized,
            ifd_num: In::PRIMARY,
            value: Value::Ascii(vec![b"2026:01:02 03:04:05".to_vec()]),
        };
        writer.push_field(&field);
        let mut cur = std::io::Cursor::new(Vec::new());
        writer.write(&mut cur, true).unwrap();
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(&cur.into_inner());

        let jpg = minimal_jpeg(&[make_jpeg_seg(0xE1, &payload)], &[]);
        let det = details(&jpg).unwrap();
        let dt = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::DateTime)
            .unwrap();
        let other_dt = dt.entries.iter().find(|e| e.field == Field::Other).unwrap();
        assert_eq!(
            other_dt.value,
            DetailValue::Text("2026-01-02 03:04:05".to_string())
        );
    }

    #[test]
    fn test_table_row_05_exif_device_make_model_serial_makernote() {
        let full_jpg = std::fs::read("tests/fixtures/full.jpg").expect("read full.jpg");
        let det = details(&full_jpg).expect("details full.jpg");
        let dev = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Device)
            .unwrap();

        assert!(dev.entries.iter().any(|e| e.field == Field::CameraMake));
        assert!(dev.entries.iter().any(|e| e.field == Field::CameraModel));
        assert!(dev.entries.iter().any(|e| e.field == Field::SerialNumber));
        let mn = dev
            .entries
            .iter()
            .find(|e| e.field == Field::MakerNote)
            .unwrap();
        assert!(matches!(mn.value, DetailValue::Bytes(_)));
    }

    #[test]
    fn test_table_row_06_exif_device_lens_other() {
        let mut writer = kamadak::experimental::Writer::new();
        let field = kamadak::Field {
            tag: Tag::LensMake,
            ifd_num: In::PRIMARY,
            value: Value::Ascii(vec![b"Lens Maker".to_vec()]),
        };
        writer.push_field(&field);
        let mut cur = std::io::Cursor::new(Vec::new());
        writer.write(&mut cur, true).unwrap();
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(&cur.into_inner());

        let jpg = minimal_jpeg(&[make_jpeg_seg(0xE1, &payload)], &[]);
        let det = details(&jpg).unwrap();
        let dev = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Device)
            .unwrap();
        let other_dev = dev
            .entries
            .iter()
            .find(|e| e.field == Field::Other)
            .unwrap();
        assert_eq!(other_dev.value, DetailValue::Text("Lens Maker".to_string()));
    }

    #[test]
    fn test_table_row_07_exif_author_artist_copyright() {
        let full_jpg = std::fs::read("tests/fixtures/full.jpg").expect("read full.jpg");
        let det = details(&full_jpg).expect("details full.jpg");
        let auth = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Author)
            .unwrap();

        let artist = auth
            .entries
            .iter()
            .find(|e| e.field == Field::Author)
            .unwrap();
        assert_eq!(
            artist.value,
            DetailValue::Text("Example Author".to_string())
        );

        let cprt = auth
            .entries
            .iter()
            .find(|e| e.field == Field::Copyright)
            .unwrap();
        assert_eq!(
            cprt.value,
            DetailValue::Text("Copyright (C) 2026 Example Author".to_string())
        );
    }

    #[test]
    fn test_table_row_08_exif_author_camera_owner_other() {
        let mut writer = kamadak::experimental::Writer::new();
        let field = kamadak::Field {
            tag: Tag::CameraOwnerName,
            ifd_num: In::PRIMARY,
            value: Value::Ascii(vec![b"Camera Owner".to_vec()]),
        };
        writer.push_field(&field);
        let mut cur = std::io::Cursor::new(Vec::new());
        writer.write(&mut cur, true).unwrap();
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(&cur.into_inner());

        let jpg = minimal_jpeg(&[make_jpeg_seg(0xE1, &payload)], &[]);
        let det = details(&jpg).unwrap();
        let auth = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Author)
            .unwrap();
        let other_auth = auth
            .entries
            .iter()
            .find(|e| e.field == Field::Other)
            .unwrap();
        assert_eq!(
            other_auth.value,
            DetailValue::Text("Camera Owner".to_string())
        );
    }

    #[test]
    fn test_table_row_09_exif_software() {
        let full_jpg = std::fs::read("tests/fixtures/full.jpg").expect("read full.jpg");
        let det = details(&full_jpg).expect("details full.jpg");
        let soft = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Software)
            .unwrap();
        let sw_entry = soft
            .entries
            .iter()
            .find(|e| e.field == Field::Software)
            .unwrap();
        assert_eq!(
            sw_entry.value,
            DetailValue::Text("Example Software".to_string())
        );
    }

    #[test]
    fn test_table_row_10_exif_host_computer() {
        let mut writer = kamadak::experimental::Writer::new();
        let field = kamadak::Field {
            tag: Tag(Context::Tiff, 0x013C),
            ifd_num: In::PRIMARY,
            value: Value::Ascii(vec![b"Host PC".to_vec()]),
        };
        writer.push_field(&field);
        let mut cur = std::io::Cursor::new(Vec::new());
        writer.write(&mut cur, true).unwrap();
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(&cur.into_inner());

        let jpg = minimal_jpeg(&[make_jpeg_seg(0xE1, &payload)], &[]);
        let det = details(&jpg).unwrap();
        let soft = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Software)
            .unwrap();
        let other_soft = soft
            .entries
            .iter()
            .find(|e| e.field == Field::Other)
            .unwrap();
        assert_eq!(other_soft.value, DetailValue::Text("Host PC".to_string()));
    }

    #[test]
    fn test_table_row_11_exif_comment_description_user_comment() {
        let full_jpg = std::fs::read("tests/fixtures/full.jpg").expect("read full.jpg");
        let det = details(&full_jpg).expect("details full.jpg");
        let comm = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Comment)
            .unwrap();

        assert!(comm.entries.iter().any(|e| e.field == Field::Description));
        let user_comm = comm
            .entries
            .iter()
            .find(|e| e.field == Field::Comment)
            .unwrap();
        assert_eq!(
            user_comm.value,
            DetailValue::Text("Example Comment".to_string())
        );
    }

    #[test]
    fn test_table_row_12_exif_ifd1_thumbnail() {
        let full_jpg = std::fs::read("tests/fixtures/full.jpg").expect("read full.jpg");
        let det = details(&full_jpg).expect("details full.jpg");
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
        assert_eq!(thumb_entry.value, DetailValue::Text("2 × 2 px".to_string())); // first thumb is JFIF 2x2
    }

    #[test]
    fn test_table_row_13_exif_ifd0_kept_tags_not_counted() {
        let mut writer = kamadak::experimental::Writer::new();
        let orient = kamadak::Field {
            tag: Tag::Orientation,
            ifd_num: In::PRIMARY,
            value: Value::Short(vec![6]),
        };
        let x_res = kamadak::Field {
            tag: Tag::XResolution,
            ifd_num: In::PRIMARY,
            value: Value::Rational(vec![kamadak::Rational { num: 300, denom: 1 }]),
        };
        let y_res = kamadak::Field {
            tag: Tag::YResolution,
            ifd_num: In::PRIMARY,
            value: Value::Rational(vec![kamadak::Rational { num: 300, denom: 1 }]),
        };
        let unit = kamadak::Field {
            tag: Tag::ResolutionUnit,
            ifd_num: In::PRIMARY,
            value: Value::Short(vec![2]),
        };
        writer.push_field(&orient);
        writer.push_field(&x_res);
        writer.push_field(&y_res);
        writer.push_field(&unit);
        let mut cur = std::io::Cursor::new(Vec::new());
        writer.write(&mut cur, true).unwrap();
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(&cur.into_inner());

        let jpg = minimal_jpeg(&[make_jpeg_seg(0xE1, &payload)], &[]);
        let insp = inspect(&jpg).unwrap();
        assert!(insp.kinds.is_empty());
        let det = details(&jpg).unwrap();
        assert!(det.groups.is_empty());
        assert_eq!(det.kept.len(), 2);
    }

    #[test]
    fn test_table_row_14_exif_pointer_tags_not_counted() {
        let mut writer = kamadak::experimental::Writer::new();
        let field = kamadak::Field {
            tag: Tag::Software,
            ifd_num: In::PRIMARY,
            value: Value::Ascii(vec![b"App".to_vec()]),
        };
        writer.push_field(&field);
        let mut cur = std::io::Cursor::new(Vec::new());
        writer.write(&mut cur, true).unwrap();
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(&cur.into_inner());

        let jpg = minimal_jpeg(&[make_jpeg_seg(0xE1, &payload)], &[]);
        let det = details(&jpg).unwrap();
        for group in &det.groups {
            for entry in &group.entries {
                assert!(entry.name != Some("ExifIFDPointer".to_string()));
            }
        }
    }

    #[test]
    fn test_table_row_15_exif_other_tags() {
        let mut writer = kamadak::experimental::Writer::new();
        let field = kamadak::Field {
            tag: Tag::ExifVersion,
            ifd_num: In::PRIMARY,
            value: Value::Undefined(b"0231".to_vec(), 0),
        };
        writer.push_field(&field);
        let mut cur = std::io::Cursor::new(Vec::new());
        writer.write(&mut cur, true).unwrap();
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(&cur.into_inner());

        let jpg = minimal_jpeg(&[make_jpeg_seg(0xE1, &payload)], &[]);
        let det = details(&jpg).unwrap();
        let other = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Other)
            .unwrap();
        assert!(other.entries.iter().any(|e| e.field == Field::Other));
    }

    #[test]
    fn test_table_row_16_iptc_location_city_state_country() {
        let app13 = make_iptc_app13(&[
            make_iptc_dataset(2, 90, b"Tokyo"),
            make_iptc_dataset(2, 95, b"Tokyo-to"),
            make_iptc_dataset(2, 101, b"Japan"),
        ]);
        let jpg = minimal_jpeg(&[app13], &[]);
        let det = details(&jpg).unwrap();
        let loc = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Location)
            .unwrap();

        assert_eq!(
            loc.entries
                .iter()
                .find(|e| e.field == Field::City)
                .unwrap()
                .value,
            DetailValue::Text("Tokyo".to_string())
        );
        assert_eq!(
            loc.entries
                .iter()
                .find(|e| e.field == Field::State)
                .unwrap()
                .value,
            DetailValue::Text("Tokyo-to".to_string())
        );
        assert_eq!(
            loc.entries
                .iter()
                .find(|e| e.field == Field::Country)
                .unwrap()
                .value,
            DetailValue::Text("Japan".to_string())
        );
    }

    #[test]
    fn test_table_row_17_iptc_location_other() {
        let app13 = make_iptc_app13(&[
            make_iptc_dataset(2, 92, b"SubLocation"),
            make_iptc_dataset(2, 100, b"JPN"),
        ]);
        let jpg = minimal_jpeg(&[app13], &[]);
        let det = details(&jpg).unwrap();
        let loc = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Location)
            .unwrap();
        assert_eq!(loc.entries.len(), 2);
        assert!(loc.entries.iter().all(|e| e.field == Field::Other));
    }

    #[test]
    fn test_table_row_18_iptc_datetime_created() {
        let app13 = make_iptc_app13(&[
            make_iptc_dataset(2, 55, b"20260102"),
            make_iptc_dataset(2, 60, b"030405+0900"),
        ]);
        let jpg = minimal_jpeg(&[app13], &[]);
        let det = details(&jpg).unwrap();
        let dt = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::DateTime)
            .unwrap();
        let created = dt
            .entries
            .iter()
            .find(|e| e.field == Field::Created)
            .unwrap();
        assert_eq!(
            created.value,
            DetailValue::Text("2026-01-02 03:04:05".to_string())
        );
    }

    #[test]
    fn test_table_row_19_iptc_author_copyright() {
        let app13 = make_iptc_app13(&[
            make_iptc_dataset(2, 80, b"Author Name"),
            make_iptc_dataset(2, 116, b"Copyright Notice"),
        ]);
        let jpg = minimal_jpeg(&[app13], &[]);
        let det = details(&jpg).unwrap();
        let auth = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Author)
            .unwrap();
        assert_eq!(
            auth.entries
                .iter()
                .find(|e| e.field == Field::Author)
                .unwrap()
                .value,
            DetailValue::Text("Author Name".to_string())
        );
        assert_eq!(
            auth.entries
                .iter()
                .find(|e| e.field == Field::Copyright)
                .unwrap()
                .value,
            DetailValue::Text("Copyright Notice".to_string())
        );
    }

    #[test]
    fn test_table_row_20_iptc_comment_title_description() {
        let app13 = make_iptc_app13(&[
            make_iptc_dataset(2, 5, b"Title Name"),
            make_iptc_dataset(2, 120, b"Description Text"),
        ]);
        let jpg = minimal_jpeg(&[app13], &[]);
        let det = details(&jpg).unwrap();
        let comm = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Comment)
            .unwrap();
        assert_eq!(
            comm.entries
                .iter()
                .find(|e| e.field == Field::Title)
                .unwrap()
                .value,
            DetailValue::Text("Title Name".to_string())
        );
        assert_eq!(
            comm.entries
                .iter()
                .find(|e| e.field == Field::Description)
                .unwrap()
                .value,
            DetailValue::Text("Description Text".to_string())
        );
    }

    #[test]
    fn test_table_row_21_iptc_other_and_unreadable() {
        let app13 = make_iptc_app13(&[make_iptc_dataset(2, 105, b"Headline Text")]);
        let jpg = minimal_jpeg(&[app13], &[]);
        let det = details(&jpg).unwrap();
        let other = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Other)
            .unwrap();
        assert_eq!(other.entries[0].name.as_deref(), Some("2:105"));

        let corrupt_app13 = make_iptc_app13(&[b"bad iptc stream".to_vec()]);
        let corrupt_jpg = minimal_jpeg(&[corrupt_app13], &[]);
        let det_corrupt = details(&corrupt_jpg).unwrap();
        let other_corrupt = det_corrupt
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Other)
            .unwrap();
        assert_eq!(other_corrupt.entries[0].name.as_deref(), Some("IPTC"));
    }

    #[test]
    fn test_table_row_22_png_text_predefined_keywords() {
        let chunks = vec![
            make_png_chunk(b"tEXt", b"Author\0Author Name"),
            make_png_chunk(b"tEXt", b"Copyright\0Copyright Text"),
            make_png_chunk(b"tEXt", b"Software\0Software Name"),
            make_png_chunk(b"tEXt", b"Title\0Title Name"),
            make_png_chunk(b"tEXt", b"Description\0Desc Text"),
            make_png_chunk(b"tEXt", b"Comment\0Comment Text"),
            make_png_chunk(b"tEXt", b"Creation Time\x002026:01:02 03:04:05"),
        ];
        let png = minimal_png(&chunks, &[]);
        let det = details(&png).unwrap();

        let auth = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Author)
            .unwrap();
        assert!(auth.entries.iter().any(|e| e.field == Field::Author));
        assert!(auth.entries.iter().any(|e| e.field == Field::Copyright));

        let soft = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Software)
            .unwrap();
        assert!(soft.entries.iter().any(|e| e.field == Field::Software));

        let comm = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Comment)
            .unwrap();
        assert!(comm.entries.iter().any(|e| e.field == Field::Title));
        assert!(comm.entries.iter().any(|e| e.field == Field::Description));
        assert!(comm.entries.iter().any(|e| e.field == Field::Comment));

        let dt = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::DateTime)
            .unwrap();
        assert_eq!(
            dt.entries
                .iter()
                .find(|e| e.field == Field::Created)
                .unwrap()
                .value,
            DetailValue::Text("2026-01-02 03:04:05".to_string())
        );
    }

    #[test]
    fn test_table_row_23_png_text_xmp() {
        let xmp_data = b"XML:com.adobe.xmp\0\0\0\0\0<xmp:CreateDate>2026-01-02</xmp:CreateDate>";
        let chunk = make_png_chunk(b"iTXt", xmp_data);
        let png = minimal_png(&[chunk], &[]);
        let det = details(&png).unwrap();
        let dt = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::DateTime)
            .unwrap();
        assert!(dt.entries.iter().any(|e| e.field == Field::Xmp));
    }

    #[test]
    fn test_table_row_24_png_text_other_keywords() {
        let chunk = make_png_chunk(b"tEXt", b"CustomKey\0CustomVal");
        let png = minimal_png(&[chunk], &[]);
        let det = details(&png).unwrap();
        let other = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Other)
            .unwrap();
        assert_eq!(other.entries[0].name.as_deref(), Some("CustomKey"));
    }

    #[test]
    fn test_png_short_time_chunk_is_still_reported() {
        let mut png_bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        png_bytes.extend(make_png_chunk(
            b"IHDR",
            &[0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0],
        ));
        png_bytes.extend(make_png_chunk(b"tIME", &[0x07, 0xEA]));
        png_bytes.extend(make_png_chunk(b"IEND", &[]));
        let insp = inspect(&png_bytes).expect("inspect");
        assert_eq!(insp.kinds, vec![MetadataKind::DateTime]);
    }

    #[test]
    fn test_table_row_25_png_time() {
        let chunk = make_png_chunk(b"tIME", &[0x07, 0xEA, 1, 2, 3, 4, 5]); // 2026-01-02 03:04:05
        let png = minimal_png(&[chunk], &[]);
        let det = details(&png).unwrap();
        let dt = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::DateTime)
            .unwrap();
        let mod_entry = dt
            .entries
            .iter()
            .find(|e| e.field == Field::Modified)
            .unwrap();
        assert_eq!(
            mod_entry.value,
            DetailValue::Text("2026-01-02 03:04:05".to_string())
        );
    }

    #[test]
    fn test_table_row_26_jpeg_com() {
        let com = make_jpeg_seg(0xFE, b"Sample Comment");
        let jpg = minimal_jpeg(&[com], &[]);
        let det = details(&jpg).unwrap();
        let comm = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Comment)
            .unwrap();
        assert_eq!(
            comm.entries
                .iter()
                .find(|e| e.field == Field::Comment)
                .unwrap()
                .value,
            DetailValue::Text("Sample Comment".to_string())
        );
    }

    #[test]
    fn test_table_row_27_jpeg_jfif_thumbnail_and_jfxx() {
        // JFIF with width 2, height 2 thumbnail
        let mut jfif = b"JFIF\0\x01\x02\x01\x01\x2c\x01\x2c\x02\x02".to_vec();
        jfif.extend_from_slice(&[0u8; 12]); // 4 rgb pixels
        let jfif_seg = make_jpeg_seg(0xE0, &jfif);
        let jfxx_seg = make_jpeg_seg(0xE0, b"JFXX\0\x10thumb");
        let jpg = minimal_jpeg(&[jfif_seg, jfxx_seg], &[]);
        let det = details(&jpg).unwrap();
        let thumb = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Thumbnail)
            .unwrap();
        assert_eq!(thumb.entries.len(), 2);
        assert_eq!(
            thumb.entries[0].value,
            DetailValue::Text("2 × 2 px".to_string())
        );
    }

    #[test]
    fn test_table_row_28_jpeg_app2_mpf() {
        let mpf = make_jpeg_seg(0xE2, b"MPF\0payload");
        let jpg = minimal_jpeg(&[mpf], &[]);
        let det = details(&jpg).unwrap();
        let thumb = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Thumbnail)
            .unwrap();
        assert!(matches!(thumb.entries[0].value, DetailValue::Bytes(_)));
    }

    #[test]
    fn test_table_row_29_trailing_image() {
        let dummy_thumb = minimal_jpeg(&[], &[]);
        let jpg = minimal_jpeg(&[], &dummy_thumb);
        let det = details(&jpg).unwrap();
        let thumb = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Thumbnail)
            .unwrap();
        assert!(
            thumb
                .entries
                .iter()
                .any(|e| matches!(e.value, DetailValue::Text(_)))
        );
    }

    #[test]
    fn test_table_row_30_trailing_other_data() {
        let jpg = minimal_jpeg(&[], b"arbitrary trailing data");
        let det = details(&jpg).unwrap();
        let other = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Other)
            .unwrap();
        let trailing = other
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("trailing data"))
            .unwrap();
        assert_eq!(
            trailing.value,
            DetailValue::Bytes(b"arbitrary trailing data".len() as u64)
        );
    }

    #[test]
    fn test_table_row_31_jpeg_other_appn() {
        let app3 = make_jpeg_seg(0xE3, b"custom app3");
        let jpg = minimal_jpeg(&[app3], &[]);
        let det = details(&jpg).unwrap();
        let other = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Other)
            .unwrap();
        assert_eq!(other.entries[0].name.as_deref(), Some("APP3"));
        assert_eq!(
            other.entries[0].value,
            DetailValue::Bytes(b"custom app3".len() as u64)
        );
    }

    #[test]
    fn test_table_row_32_png_webp_other_chunks() {
        let chunk = make_png_chunk(b"caBX", b"binary chunk");
        let png = minimal_png(&[chunk], &[]);
        let det = details(&png).unwrap();
        let other = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::Other)
            .unwrap();
        assert_eq!(other.entries[0].name.as_deref(), Some("caBX"));
        assert_eq!(
            other.entries[0].value,
            DetailValue::Bytes(b"binary chunk".len() as u64)
        );
    }

    // --- Fixture classification tests ---

    #[test]
    fn test_fixtures_classification() {
        let full_jpg = std::fs::read("tests/fixtures/full.jpg").unwrap();
        assert_eq!(
            inspect(&full_jpg).unwrap().kinds,
            vec![
                MetadataKind::Location,
                MetadataKind::DateTime,
                MetadataKind::Device,
                MetadataKind::Author,
                MetadataKind::Software,
                MetadataKind::Comment,
                MetadataKind::Thumbnail,
                MetadataKind::Other,
            ]
        );

        let full_png = std::fs::read("tests/fixtures/full.png").unwrap();
        assert_eq!(
            inspect(&full_png).unwrap().kinds,
            vec![
                MetadataKind::Location,
                MetadataKind::DateTime,
                MetadataKind::Device,
                MetadataKind::Author,
                MetadataKind::Software,
                MetadataKind::Comment,
                MetadataKind::Other,
            ]
        );

        let full_webp = std::fs::read("tests/fixtures/full.webp").unwrap();
        assert_eq!(
            inspect(&full_webp).unwrap().kinds,
            vec![
                MetadataKind::Location,
                MetadataKind::DateTime,
                MetadataKind::Device,
                MetadataKind::Author,
                MetadataKind::Software,
                MetadataKind::Comment,
            ]
        );

        let prog_jpg = std::fs::read("tests/fixtures/progressive.jpg").unwrap();
        assert_eq!(
            inspect(&prog_jpg).unwrap().kinds,
            vec![
                MetadataKind::Location,
                MetadataKind::DateTime,
                MetadataKind::Device,
                MetadataKind::Author,
                MetadataKind::Software,
                MetadataKind::Comment,
            ]
        );

        let cmyk_jpg = std::fs::read("tests/fixtures/cmyk.jpg").unwrap();
        assert_eq!(
            inspect(&cmyk_jpg).unwrap().kinds,
            Vec::<MetadataKind>::new()
        );

        let anim_png = std::fs::read("tests/fixtures/anim.png").unwrap();
        assert_eq!(
            inspect(&anim_png).unwrap().kinds,
            vec![MetadataKind::Comment]
        );

        let anim_webp = std::fs::read("tests/fixtures/anim.webp").unwrap();
        assert_eq!(
            inspect(&anim_webp).unwrap().kinds,
            vec![
                MetadataKind::Location,
                MetadataKind::DateTime,
                MetadataKind::Device,
                MetadataKind::Author,
                MetadataKind::Software,
                MetadataKind::Comment,
            ]
        );

        for clean_name in ["clean.jpg", "clean.png", "clean.webp"] {
            let bytes = std::fs::read(format!("tests/fixtures/{clean_name}")).unwrap();
            assert!(inspect(&bytes).unwrap().kinds.is_empty());
        }

        let webp_named = std::fs::read("tests/fixtures/webp_named.jpg").unwrap();
        assert_eq!(
            inspect(&webp_named).unwrap().kinds,
            vec![
                MetadataKind::Location,
                MetadataKind::DateTime,
                MetadataKind::Device,
                MetadataKind::Author,
                MetadataKind::Software,
                MetadataKind::Comment,
            ]
        );
    }

    #[test]
    fn test_xmp_byte_length_in_details() {
        let xmp_payload = b"<xmp:CreateDate>2026-01-02</xmp:CreateDate>";
        let mut app1_payload = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
        app1_payload.extend_from_slice(xmp_payload);

        let jpg = minimal_jpeg(&[make_jpeg_seg(0xE1, &app1_payload)], &[]);
        let det = details(&jpg).unwrap();
        let dt = det
            .groups
            .iter()
            .find(|g| g.kind == MetadataKind::DateTime)
            .unwrap();
        let xmp_entry = dt.entries.iter().find(|e| e.field == Field::Xmp).unwrap();
        assert_eq!(
            xmp_entry.value,
            DetailValue::Bytes(xmp_payload.len() as u64)
        );
    }
}
