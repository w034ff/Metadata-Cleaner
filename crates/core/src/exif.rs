//! EXIF metadata classification, detail value extraction, and kept metadata preservation (design §4.5).

use exif::{Context, In, Reader, Tag, Value};

use crate::jpeg;
use crate::report::{DetailEntry, DetailValue, Field, KeptInfo, MetadataKind, ResolutionUnit};

const TAG_ORIENTATION: u16 = 0x0112;
const TAG_X_RESOLUTION: u16 = 0x011A;
const TAG_Y_RESOLUTION: u16 = 0x011B;
const TAG_RESOLUTION_UNIT: u16 = 0x0128;
const TAG_HOST_COMPUTER: Tag = Tag(Context::Tiff, 0x013C);
const TAG_JPEG_INTERCHANGE_FORMAT: Tag = Tag(Context::Tiff, 0x0201);
const TAG_JPEG_INTERCHANGE_FORMAT_LENGTH: Tag = Tag(Context::Tiff, 0x0202);

/// Creates a minimal little-endian TIFF containing ONLY:
/// - IFD0 `Orientation` (if in `2..=8`)
/// - IFD0 `XResolution`, `YResolution`, and `ResolutionUnit` (if all 3 are present)
///
/// Returns `None` if neither is kept, or if the input cannot be parsed.
pub fn kept_only(tiff: &[u8]) -> Option<Vec<u8>> {
    let exif = Reader::new().read_raw(tiff.to_vec()).ok()?;

    let orient = exif
        .get_field(Tag::Orientation, In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .filter(|&o| (2..=8).contains(&o))
        .map(|o| o as u16);

    let x_res = exif
        .get_field(Tag::XResolution, In::PRIMARY)
        .and_then(|f| match &f.value {
            Value::Rational(v) if !v.is_empty() => Some(v[0]),
            _ => None,
        });

    let y_res = exif
        .get_field(Tag::YResolution, In::PRIMARY)
        .and_then(|f| match &f.value {
            Value::Rational(v) if !v.is_empty() => Some(v[0]),
            _ => None,
        });

    let res_unit = exif
        .get_field(Tag::ResolutionUnit, In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .map(|u| u as u16);

    let has_res = x_res.is_some() && y_res.is_some() && res_unit.is_some();
    if orient.is_none() && !has_res {
        return None;
    }

    let mut entry_count: u16 = 0;
    if orient.is_some() {
        entry_count += 1;
    }
    if has_res {
        entry_count += 3;
    }

    let ifd0_offset: u32 = 8;
    let entries_size = entry_count as u32 * 12;
    let next_ifd_offset_size: u32 = 4;
    let rationals_offset = ifd0_offset + 2 + entries_size + next_ifd_offset_size;

    let mut out = Vec::new();
    // Little-endian TIFF header: "II", magic 42, IFD0 offset 8
    out.extend_from_slice(b"II\x2a\x00\x08\x00\x00\x00");
    out.extend_from_slice(&entry_count.to_le_bytes());

    let mut current_rational_offset = rationals_offset;

    // Tags must be written in ascending tag number order
    // 0x0112: Orientation
    if let Some(o) = orient {
        out.extend_from_slice(&TAG_ORIENTATION.to_le_bytes());
        out.extend_from_slice(&3u16.to_le_bytes()); // Type 3: SHORT
        out.extend_from_slice(&1u32.to_le_bytes()); // Count 1
        out.extend_from_slice(&(o as u32).to_le_bytes()); // Value
    }

    // 0x011A: XResolution
    if has_res {
        out.extend_from_slice(&TAG_X_RESOLUTION.to_le_bytes());
        out.extend_from_slice(&5u16.to_le_bytes()); // Type 5: RATIONAL
        out.extend_from_slice(&1u32.to_le_bytes()); // Count 1
        out.extend_from_slice(&current_rational_offset.to_le_bytes());
        current_rational_offset += 8;

        // 0x011B: YResolution
        out.extend_from_slice(&TAG_Y_RESOLUTION.to_le_bytes());
        out.extend_from_slice(&5u16.to_le_bytes()); // Type 5: RATIONAL
        out.extend_from_slice(&1u32.to_le_bytes()); // Count 1
        out.extend_from_slice(&current_rational_offset.to_le_bytes());

        // 0x0128: ResolutionUnit
        let u = res_unit.expect("checked has_res");
        out.extend_from_slice(&TAG_RESOLUTION_UNIT.to_le_bytes());
        out.extend_from_slice(&3u16.to_le_bytes()); // Type 3: SHORT
        out.extend_from_slice(&1u32.to_le_bytes()); // Count 1
        out.extend_from_slice(&(u as u32).to_le_bytes());
    }

    // Next IFD offset: 0
    out.extend_from_slice(&0u32.to_le_bytes());

    // Rational payload values
    if has_res {
        let x = x_res.expect("checked has_res");
        let y = y_res.expect("checked has_res");
        out.extend_from_slice(&x.num.to_le_bytes());
        out.extend_from_slice(&x.denom.to_le_bytes());
        out.extend_from_slice(&y.num.to_le_bytes());
        out.extend_from_slice(&y.denom.to_le_bytes());
    }

    Some(out)
}

/// Reads the kept orientation and resolution from raw TIFF bytes, if present.
pub fn parse_kept_info(tiff: &[u8]) -> (Option<u8>, Option<KeptInfo>) {
    let exif = match Reader::new().read_raw(tiff.to_vec()) {
        Ok(e) => e,
        Err(_) => return (None, None),
    };

    let orientation = exif
        .get_field(Tag::Orientation, In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .filter(|&o| (2..=8).contains(&o))
        .map(|o| o as u8);

    let x_res = exif
        .get_field(Tag::XResolution, In::PRIMARY)
        .and_then(|f| match &f.value {
            Value::Rational(v) if !v.is_empty() => Some(v[0]),
            _ => None,
        });

    let y_res = exif
        .get_field(Tag::YResolution, In::PRIMARY)
        .and_then(|f| match &f.value {
            Value::Rational(v) if !v.is_empty() => Some(v[0]),
            _ => None,
        });

    let res_unit = exif
        .get_field(Tag::ResolutionUnit, In::PRIMARY)
        .and_then(|f| f.value.get_uint(0));

    let resolution = match (x_res, y_res, res_unit) {
        (Some(x), Some(y), Some(2)) => {
            let x_val = (x.to_f64() + 0.5) as u32;
            let y_val = (y.to_f64() + 0.5) as u32;
            if x_val > 0 && y_val > 0 {
                Some(KeptInfo::Resolution {
                    x: x_val,
                    y: y_val,
                    unit: ResolutionUnit::Inch,
                })
            } else {
                None
            }
        }
        (Some(x), Some(y), Some(3)) => {
            let x_val = (x.to_f64() + 0.5) as u32;
            let y_val = (y.to_f64() + 0.5) as u32;
            if x_val > 0 && y_val > 0 {
                Some(KeptInfo::Resolution {
                    x: x_val,
                    y: y_val,
                    unit: ResolutionUnit::Centimeter,
                })
            } else {
                None
            }
        }
        _ => None,
    };

    (orientation, resolution)
}

/// Formats GPS latitude or longitude from rational [deg, min, sec] values and optional ref string.
fn format_gps_coord(field: &exif::Field, ref_field: Option<&exif::Field>) -> String {
    let rationals = match &field.value {
        Value::Rational(v) if v.len() >= 3 => v,
        _ => return field.display_value().to_string(),
    };

    let deg = rationals[0].to_f64();
    let min = rationals[1].to_f64();
    let sec = rationals[2].to_f64();

    let mut sec_str = format!("{:.2}", sec);
    if sec_str.contains('.') {
        while sec_str.ends_with('0') {
            sec_str.pop();
        }
        if sec_str.ends_with('.') {
            sec_str.pop();
        }
    }

    let deg_int = deg.round() as u32;
    let min_int = min.round() as u32;

    let ref_str = ref_field
        .and_then(|rf| match &rf.value {
            Value::Ascii(v) if !v.is_empty() => {
                let s = String::from_utf8_lossy(&v[0]);
                let trimmed = s.trim_matches(char::from(0)).trim();
                if !trimmed.is_empty() {
                    Some(format!(" {trimmed}"))
                } else {
                    None
                }
            }
            _ => None,
        })
        .unwrap_or_default();

    format!("{deg_int}°{min_int}′{sec_str}″{ref_str}")
}

/// Formats an EXIF DateTime string (`YYYY:MM:DD HH:MM:SS` -> `YYYY-MM-DD HH:MM:SS`).
fn format_datetime(field: &exif::Field) -> String {
    let raw = match &field.value {
        Value::Ascii(v) if !v.is_empty() => {
            let s = String::from_utf8_lossy(&v[0]);
            s.trim_matches(char::from(0)).to_string()
        }
        _ => field.display_value().to_string(),
    };

    if raw.len() >= 19 {
        let b = raw.as_bytes();
        if b[4] == b':' && b[7] == b':' && b[10] == b' ' && b[13] == b':' && b[16] == b':' {
            return format!("{}-{}-{}", &raw[..4], &raw[5..7], &raw[8..]);
        }
    }

    raw
}

/// Extracts a string from an ASCII field without quotes or trailing NUL, or displays the field.
fn format_ascii_or_display(field: &exif::Field) -> String {
    match &field.value {
        Value::Ascii(v) => v
            .iter()
            .map(|slice| {
                let s = String::from_utf8_lossy(slice);
                s.trim_matches(char::from(0)).to_string()
            })
            .collect::<Vec<_>>()
            .join(", "),
        _ => field.display_value().to_string(),
    }
}

/// Formats an EXIF `UserComment` field, stripping the leading 8-byte charset marker if present.
fn format_user_comment(field: &exif::Field) -> String {
    match &field.value {
        Value::Undefined(bytes, _) => {
            if bytes.starts_with(b"ASCII\0\0\0") {
                let s = String::from_utf8_lossy(&bytes[8..]);
                s.trim_matches(char::from(0)).to_string()
            } else {
                field.display_value().to_string()
            }
        }
        _ => format_ascii_or_display(field),
    }
}

/// Collects detail entries from raw TIFF bytes in file order.
pub fn collect_details(tiff: &[u8]) -> Vec<(MetadataKind, DetailEntry)> {
    let exif = match Reader::new().read_raw(tiff.to_vec()) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut entries = Vec::new();
    let mut ifd1_handled = false;

    for f in exif.fields() {
        // IFD1 thumbnail fields
        if f.ifd_num == In::THUMBNAIL {
            if !ifd1_handled {
                ifd1_handled = true;
                let value = extract_ifd1_thumbnail_value(&exif);
                entries.push((
                    MetadataKind::Thumbnail,
                    DetailEntry {
                        field: Field::Thumbnail,
                        name: None,
                        value,
                    },
                ));
            }
            continue;
        }

        // IFD0 kept fields (Orientation, XResolution, YResolution, ResolutionUnit): do not count
        if f.ifd_num == In::PRIMARY
            && matches!(
                f.tag,
                Tag::Orientation | Tag::XResolution | Tag::YResolution | Tag::ResolutionUnit
            )
        {
            continue;
        }

        // Pointer tags: do not count
        if matches!(
            f.tag,
            Tag::ExifIFDPointer | Tag::GPSInfoIFDPointer | Tag::InteropIFDPointer
        ) {
            continue;
        }

        // GPS Ref tags: merged into Latitude/Longitude, do not emit standalone
        if matches!(f.tag, Tag::GPSLatitudeRef | Tag::GPSLongitudeRef) {
            continue;
        }

        // GPS fields
        if f.tag == Tag::GPSLatitude {
            let ref_field = exif.get_field(Tag::GPSLatitudeRef, f.ifd_num);
            let text = format_gps_coord(f, ref_field);
            entries.push((
                MetadataKind::Location,
                DetailEntry {
                    field: Field::Latitude,
                    name: None,
                    value: DetailValue::Text(text),
                },
            ));
            continue;
        }

        if f.tag == Tag::GPSLongitude {
            let ref_field = exif.get_field(Tag::GPSLongitudeRef, f.ifd_num);
            let text = format_gps_coord(f, ref_field);
            entries.push((
                MetadataKind::Location,
                DetailEntry {
                    field: Field::Longitude,
                    name: None,
                    value: DetailValue::Text(text),
                },
            ));
            continue;
        }

        if f.tag.context() == Context::Gps {
            entries.push((
                MetadataKind::Location,
                DetailEntry {
                    field: Field::Other,
                    name: Some(f.tag.to_string()),
                    value: format_other_field_value(f),
                },
            ));
            continue;
        }

        // Date/Time fields
        if f.tag == Tag::DateTimeOriginal {
            entries.push((
                MetadataKind::DateTime,
                DetailEntry {
                    field: Field::Taken,
                    name: None,
                    value: DetailValue::Text(format_datetime(f)),
                },
            ));
            continue;
        }

        if f.tag == Tag::DateTime {
            entries.push((
                MetadataKind::DateTime,
                DetailEntry {
                    field: Field::Modified,
                    name: None,
                    value: DetailValue::Text(format_datetime(f)),
                },
            ));
            continue;
        }

        if matches!(
            f.tag,
            Tag::DateTimeDigitized
                | Tag::SubSecTime
                | Tag::SubSecTimeOriginal
                | Tag::SubSecTimeDigitized
                | Tag::OffsetTime
                | Tag::OffsetTimeOriginal
                | Tag::OffsetTimeDigitized
        ) {
            let value = if f.tag == Tag::DateTimeDigitized {
                DetailValue::Text(format_datetime(f))
            } else {
                format_other_field_value(f)
            };
            entries.push((
                MetadataKind::DateTime,
                DetailEntry {
                    field: Field::Other,
                    name: Some(f.tag.to_string()),
                    value,
                },
            ));
            continue;
        }

        // Device fields
        if f.tag == Tag::Make {
            entries.push((
                MetadataKind::Device,
                DetailEntry {
                    field: Field::CameraMake,
                    name: None,
                    value: DetailValue::Text(format_ascii_or_display(f)),
                },
            ));
            continue;
        }

        if f.tag == Tag::Model {
            entries.push((
                MetadataKind::Device,
                DetailEntry {
                    field: Field::CameraModel,
                    name: None,
                    value: DetailValue::Text(format_ascii_or_display(f)),
                },
            ));
            continue;
        }

        if f.tag == Tag::BodySerialNumber {
            entries.push((
                MetadataKind::Device,
                DetailEntry {
                    field: Field::SerialNumber,
                    name: None,
                    value: DetailValue::Text(format_ascii_or_display(f)),
                },
            ));
            continue;
        }

        if f.tag == Tag::MakerNote {
            let bytes_len = match &f.value {
                Value::Undefined(v, _) => v.len() as u64,
                Value::Byte(v) => v.len() as u64,
                _ => 0,
            };
            entries.push((
                MetadataKind::Device,
                DetailEntry {
                    field: Field::MakerNote,
                    name: None,
                    value: DetailValue::Bytes(bytes_len),
                },
            ));
            continue;
        }

        if matches!(
            f.tag,
            Tag::LensMake | Tag::LensModel | Tag::LensSerialNumber
        ) {
            entries.push((
                MetadataKind::Device,
                DetailEntry {
                    field: Field::Other,
                    name: Some(f.tag.to_string()),
                    value: format_other_field_value(f),
                },
            ));
            continue;
        }

        // Author fields
        if f.tag == Tag::Artist {
            entries.push((
                MetadataKind::Author,
                DetailEntry {
                    field: Field::Author,
                    name: None,
                    value: DetailValue::Text(format_ascii_or_display(f)),
                },
            ));
            continue;
        }

        if f.tag == Tag::Copyright {
            entries.push((
                MetadataKind::Author,
                DetailEntry {
                    field: Field::Copyright,
                    name: None,
                    value: DetailValue::Text(format_ascii_or_display(f)),
                },
            ));
            continue;
        }

        if f.tag == Tag::CameraOwnerName {
            entries.push((
                MetadataKind::Author,
                DetailEntry {
                    field: Field::Other,
                    name: Some(f.tag.to_string()),
                    value: format_other_field_value(f),
                },
            ));
            continue;
        }

        // Software fields
        if f.tag == Tag::Software {
            entries.push((
                MetadataKind::Software,
                DetailEntry {
                    field: Field::Software,
                    name: None,
                    value: DetailValue::Text(format_ascii_or_display(f)),
                },
            ));
            continue;
        }

        if f.tag == TAG_HOST_COMPUTER {
            entries.push((
                MetadataKind::Software,
                DetailEntry {
                    field: Field::Other,
                    name: Some(f.tag.to_string()),
                    value: format_other_field_value(f),
                },
            ));
            continue;
        }

        // Comment fields
        if f.tag == Tag::ImageDescription {
            entries.push((
                MetadataKind::Comment,
                DetailEntry {
                    field: Field::Description,
                    name: None,
                    value: DetailValue::Text(format_ascii_or_display(f)),
                },
            ));
            continue;
        }

        if f.tag == Tag::UserComment {
            entries.push((
                MetadataKind::Comment,
                DetailEntry {
                    field: Field::Comment,
                    name: None,
                    value: DetailValue::Text(format_user_comment(f)),
                },
            ));
            continue;
        }

        // All other fields
        entries.push((
            MetadataKind::Other,
            DetailEntry {
                field: Field::Other,
                name: Some(f.tag.to_string()),
                value: format_other_field_value(f),
            },
        ));
    }

    entries
}

/// Formats the value of an `Other` EXIF field. Non-string types become `Bytes`.
fn format_other_field_value(field: &exif::Field) -> DetailValue {
    match &field.value {
        Value::Ascii(_) => DetailValue::Text(format_ascii_or_display(field)),
        Value::Undefined(bytes, _) => {
            if matches!(field.tag, Tag::ExifVersion | Tag::FlashpixVersion) {
                DetailValue::Text(field.display_value().to_string())
            } else {
                DetailValue::Bytes(bytes.len() as u64)
            }
        }
        Value::Byte(bytes) => DetailValue::Bytes(bytes.len() as u64),
        _ => DetailValue::Text(field.display_value().to_string()),
    }
}

/// Extracts thumbnail dimensions or byte size from IFD1.
fn extract_ifd1_thumbnail_value(exif: &exif::Exif) -> DetailValue {
    let offset = exif
        .get_field(TAG_JPEG_INTERCHANGE_FORMAT, In::THUMBNAIL)
        .and_then(|f| f.value.get_uint(0));
    let length = exif
        .get_field(TAG_JPEG_INTERCHANGE_FORMAT_LENGTH, In::THUMBNAIL)
        .and_then(|f| f.value.get_uint(0));

    if let (Some(ofs), Some(len)) = (offset, length) {
        let ofs = ofs as usize;
        let len = len as usize;
        if ofs + len <= exif.buf().len() {
            let jpeg_bytes = &exif.buf()[ofs..ofs + len];
            if let Ok(parsed) = jpeg::parse(jpeg_bytes)
                && let Some((w, h)) = parsed.dimensions()
            {
                return DetailValue::Text(format!("{w} × {h} px"));
            }
        }
        return DetailValue::Bytes(len as u64);
    }

    DetailValue::Bytes(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kept_only_orientation_and_resolution() {
        let fields = vec![
            exif::Field {
                tag: Tag::Orientation,
                ifd_num: In::PRIMARY,
                value: Value::Short(vec![6]),
            },
            exif::Field {
                tag: Tag::XResolution,
                ifd_num: In::PRIMARY,
                value: Value::Rational(vec![exif::Rational { num: 300, denom: 1 }]),
            },
            exif::Field {
                tag: Tag::YResolution,
                ifd_num: In::PRIMARY,
                value: Value::Rational(vec![exif::Rational { num: 300, denom: 1 }]),
            },
            exif::Field {
                tag: Tag::ResolutionUnit,
                ifd_num: In::PRIMARY,
                value: Value::Short(vec![2]),
            },
            // Extra field to be stripped
            exif::Field {
                tag: Tag::Artist,
                ifd_num: In::PRIMARY,
                value: Value::Ascii(vec![b"Author".to_vec()]),
            },
        ];

        let mut writer = exif::experimental::Writer::new();
        for f in &fields {
            writer.push_field(f);
        }
        let mut cur = std::io::Cursor::new(Vec::new());
        writer.write(&mut cur, true).unwrap();
        let raw_tiff = cur.into_inner();

        let kept = kept_only(&raw_tiff).expect("should produce kept TIFF");
        let re_parsed = Reader::new()
            .read_raw(kept)
            .expect("must parse with kamadak");

        assert_eq!(
            re_parsed
                .get_field(Tag::Orientation, In::PRIMARY)
                .unwrap()
                .value
                .get_uint(0),
            Some(6)
        );
        assert_eq!(
            re_parsed
                .get_field(Tag::ResolutionUnit, In::PRIMARY)
                .unwrap()
                .value
                .get_uint(0),
            Some(2)
        );
        assert!(re_parsed.get_field(Tag::Artist, In::PRIMARY).is_none());
    }

    #[test]
    fn test_kept_only_orient_1_is_none() {
        let mut writer = exif::experimental::Writer::new();
        let field = exif::Field {
            tag: Tag::Orientation,
            ifd_num: In::PRIMARY,
            value: Value::Short(vec![1]),
        };
        writer.push_field(&field);
        let mut cur = std::io::Cursor::new(Vec::new());
        writer.write(&mut cur, true).unwrap();
        assert!(kept_only(&cur.into_inner()).is_none());
    }

    #[test]
    fn test_kept_only_incomplete_resolution() {
        let mut writer = exif::experimental::Writer::new();
        let field = exif::Field {
            tag: Tag::XResolution,
            ifd_num: In::PRIMARY,
            value: Value::Rational(vec![exif::Rational { num: 300, denom: 1 }]),
        };
        writer.push_field(&field);
        // Missing YResolution and ResolutionUnit
        let mut cur = std::io::Cursor::new(Vec::new());
        writer.write(&mut cur, true).unwrap();
        assert!(kept_only(&cur.into_inner()).is_none());
    }

    #[test]
    fn test_kept_only_corrupt_exif_is_none() {
        assert!(kept_only(&[0x12, 0x34, 0x56, 0x78]).is_none());
        assert!(kept_only(&[]).is_none());
    }

    #[test]
    fn test_kept_only_orient_1_to_8_fixtures() {
        for o in 1..=8 {
            let path = format!("tests/fixtures/orient{o}.jpg");
            let bytes = std::fs::read(&path).expect("read orient fixture");
            let parsed = jpeg::parse(&bytes).expect("parse jpeg");
            let tiff = parsed.exif().expect("has exif");
            let kept = kept_only(tiff);

            if o == 1 {
                assert!(kept.is_none(), "orient1 should have no kept TIFF");
            } else {
                let kept_tiff = kept.expect("orient2-8 should produce kept TIFF");
                let re_parsed = Reader::new()
                    .read_raw(kept_tiff)
                    .expect("must parse with kamadak");
                let orient_val = re_parsed
                    .get_field(Tag::Orientation, In::PRIMARY)
                    .and_then(|f| f.value.get_uint(0));
                assert_eq!(orient_val, Some(o as u32));
                assert!(re_parsed.get_field(Tag::XResolution, In::PRIMARY).is_none());
            }
        }
    }

    #[test]
    fn test_kept_only_full_jpg() {
        let bytes = std::fs::read("tests/fixtures/full.jpg").expect("read full.jpg");
        let parsed = jpeg::parse(&bytes).expect("parse jpeg");
        let tiff = parsed.exif().expect("has exif");
        let kept = kept_only(tiff).expect("full.jpg has kept exif");
        let re_parsed = Reader::new()
            .read_raw(kept)
            .expect("must parse with kamadak");

        assert_eq!(
            re_parsed
                .get_field(Tag::Orientation, In::PRIMARY)
                .unwrap()
                .value
                .get_uint(0),
            Some(6)
        );
        let x_res = re_parsed.get_field(Tag::XResolution, In::PRIMARY).unwrap();
        match &x_res.value {
            Value::Rational(v) => {
                assert_eq!(v[0].num, 300);
                assert_eq!(v[0].denom, 1);
            }
            _ => panic!("expected rational"),
        }
        let y_res = re_parsed.get_field(Tag::YResolution, In::PRIMARY).unwrap();
        match &y_res.value {
            Value::Rational(v) => {
                assert_eq!(v[0].num, 300);
                assert_eq!(v[0].denom, 1);
            }
            _ => panic!("expected rational"),
        }
        assert_eq!(
            re_parsed
                .get_field(Tag::ResolutionUnit, In::PRIMARY)
                .unwrap()
                .value
                .get_uint(0),
            Some(2)
        );
        assert!(re_parsed.get_field(Tag::Artist, In::PRIMARY).is_none());
    }
}
