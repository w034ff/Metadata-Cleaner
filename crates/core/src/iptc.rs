//! IPTC metadata parsing and detail entry extraction (design §4.5).

use crate::report::{DetailEntry, DetailValue, Field, MetadataKind};

const IPTC_TAG_MARKER: u8 = 0x1C;
const PHOTOSHOP_PREFIX: &[u8] = b"Photoshop 3.0\0";
const BIM_SIG: &[u8; 4] = b"8BIM";
const IPTC_RESOURCE_ID: u16 = 0x0404;

/// One parsed IPTC dataset entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IptcDataset<'a> {
    pub record: u8,
    pub dataset: u8,
    pub data: &'a [u8],
}

/// Extracts the raw IPTC-NAA data (resource ID 0x0404) from a JPEG APP13 payload.
pub fn parse_app13_iptc(payload: &[u8]) -> Option<&[u8]> {
    if !payload.starts_with(PHOTOSHOP_PREFIX) {
        return None;
    }

    let mut pos = PHOTOSHOP_PREFIX.len();
    while pos + 8 <= payload.len() {
        if &payload[pos..pos + 4] != BIM_SIG {
            break;
        }
        let res_id = u16::from_be_bytes([payload[pos + 4], payload[pos + 5]]);
        let name_len = payload[pos + 6] as usize;
        let name_block_len = (1 + name_len + 1) & !1;
        let data_ofs = pos + 6 + name_block_len;
        if data_ofs + 4 > payload.len() {
            break;
        }
        let data_len = u32::from_be_bytes([
            payload[data_ofs],
            payload[data_ofs + 1],
            payload[data_ofs + 2],
            payload[data_ofs + 3],
        ]) as usize;
        let data_start = data_ofs + 4;
        let data_end = match data_start.checked_add(data_len) {
            Some(e) if e <= payload.len() => e,
            _ => break,
        };

        if res_id == IPTC_RESOURCE_ID {
            return Some(&payload[data_start..data_end]);
        }

        pos = data_end;
        if data_len & 1 == 1 {
            pos += 1;
        }
    }

    None
}

/// Parses a raw IPTC data stream into a sequence of datasets.
/// Returns `None` if the stream is malformed.
pub fn parse_datasets(data: &[u8]) -> Option<Vec<IptcDataset<'_>>> {
    let mut pos = 0;
    let mut datasets = Vec::new();

    while pos < data.len() {
        if data[pos] != IPTC_TAG_MARKER || pos + 5 > data.len() {
            return None;
        }
        let record = data[pos + 1];
        let dataset = data[pos + 2];
        let len = u16::from_be_bytes([data[pos + 3], data[pos + 4]]) as usize;
        let start = pos + 5;
        let end = match start.checked_add(len) {
            Some(e) if e <= data.len() => e,
            _ => return None,
        };
        datasets.push(IptcDataset {
            record,
            dataset,
            data: &data[start..end],
        });
        pos = end;
    }

    if datasets.is_empty() {
        None
    } else {
        Some(datasets)
    }
}

/// Returns `DetailValue::Text` if `data` is valid UTF-8 and contains no control characters,
/// or `DetailValue::Bytes` otherwise (design §4.5).
fn parse_iptc_text(data: &[u8]) -> DetailValue {
    match std::str::from_utf8(data) {
        Ok(s)
            if !s
                .chars()
                .any(|c| c.is_control() && c != '\t' && c != '\n' && c != '\r') =>
        {
            DetailValue::Text(s.to_string())
        }
        _ => DetailValue::Bytes(data.len() as u64),
    }
}

/// Formats an IPTC date (2:55) and optional time (2:60) into `YYYY-MM-DD HH:MM:SS` or `YYYY-MM-DD`.
fn format_iptc_datetime(date_bytes: &[u8], time_bytes: Option<&[u8]>) -> DetailValue {
    let Ok(date_str) = std::str::from_utf8(date_bytes) else {
        return DetailValue::Bytes(date_bytes.len() as u64);
    };
    if date_str.chars().any(|c| c.is_control()) {
        return DetailValue::Bytes(date_bytes.len() as u64);
    }

    let formatted_date = if date_str.len() == 8 && date_str.chars().all(|c| c.is_ascii_digit()) {
        format!(
            "{}-{}-{}",
            &date_str[0..4],
            &date_str[4..6],
            &date_str[6..8]
        )
    } else {
        date_str.to_string()
    };

    if let Some(tb) = time_bytes
        && let Ok(time_str) = std::str::from_utf8(tb)
        && !time_str.chars().any(|c| c.is_control())
        && time_str.len() >= 6
        && time_str[..6].chars().all(|c| c.is_ascii_digit())
    {
        let formatted_time = format!(
            "{}:{}:{}",
            &time_str[0..2],
            &time_str[2..4],
            &time_str[4..6]
        );
        return DetailValue::Text(format!("{formatted_date} {formatted_time}"));
    }

    DetailValue::Text(formatted_date)
}

/// Collects detail entries from a raw IPTC data block.
pub fn collect_details(iptc_data: &[u8]) -> Vec<(MetadataKind, DetailEntry)> {
    let datasets = match parse_datasets(iptc_data) {
        Some(ds) => ds,
        None => {
            return vec![(
                MetadataKind::Other,
                DetailEntry {
                    field: Field::Other,
                    name: Some("IPTC".to_string()),
                    value: DetailValue::Bytes(iptc_data.len() as u64),
                },
            )];
        }
    };

    // Find 2:60 time dataset if present to merge with 2:55 date
    let time_dataset = datasets
        .iter()
        .find(|d| d.record == 2 && d.dataset == 60)
        .map(|d| d.data);

    let mut entries = Vec::new();

    for d in &datasets {
        if d.record == 1 {
            // Exclude record 1 (e.g. 1:90 character set)
            continue;
        }

        match (d.record, d.dataset) {
            (2, 60) => {
                // Merged into 2:55, do not emit standalone entry
            }
            (2, 55) => {
                let dt = format_iptc_datetime(d.data, time_dataset);
                entries.push((
                    MetadataKind::DateTime,
                    DetailEntry {
                        field: Field::Created,
                        name: None,
                        value: dt,
                    },
                ));
            }
            (2, 90) => {
                entries.push((
                    MetadataKind::Location,
                    DetailEntry {
                        field: Field::City,
                        name: None,
                        value: parse_iptc_text(d.data),
                    },
                ));
            }
            (2, 95) => {
                entries.push((
                    MetadataKind::Location,
                    DetailEntry {
                        field: Field::State,
                        name: None,
                        value: parse_iptc_text(d.data),
                    },
                ));
            }
            (2, 101) => {
                entries.push((
                    MetadataKind::Location,
                    DetailEntry {
                        field: Field::Country,
                        name: None,
                        value: parse_iptc_text(d.data),
                    },
                ));
            }
            (2, 92) | (2, 100) => {
                entries.push((
                    MetadataKind::Location,
                    DetailEntry {
                        field: Field::Other,
                        name: Some(format!("{}:{}", d.record, d.dataset)),
                        value: parse_iptc_text(d.data),
                    },
                ));
            }
            (2, 80) => {
                entries.push((
                    MetadataKind::Author,
                    DetailEntry {
                        field: Field::Author,
                        name: None,
                        value: parse_iptc_text(d.data),
                    },
                ));
            }
            (2, 116) => {
                entries.push((
                    MetadataKind::Author,
                    DetailEntry {
                        field: Field::Copyright,
                        name: None,
                        value: parse_iptc_text(d.data),
                    },
                ));
            }
            (2, 5) => {
                entries.push((
                    MetadataKind::Comment,
                    DetailEntry {
                        field: Field::Title,
                        name: None,
                        value: parse_iptc_text(d.data),
                    },
                ));
            }
            (2, 120) => {
                entries.push((
                    MetadataKind::Comment,
                    DetailEntry {
                        field: Field::Description,
                        name: None,
                        value: parse_iptc_text(d.data),
                    },
                ));
            }
            (rec, num) => {
                entries.push((
                    MetadataKind::Other,
                    DetailEntry {
                        field: Field::Other,
                        name: Some(format!("{rec}:{num}")),
                        value: parse_iptc_text(d.data),
                    },
                ));
            }
        }
    }

    // Datasets that produce no entry of their own (record 1, a lone 2:60) are
    // still removed with the block, so the block is reported.
    if entries.is_empty() {
        entries.push((
            MetadataKind::Other,
            DetailEntry {
                field: Field::Other,
                name: Some("IPTC".to_string()),
                value: DetailValue::Bytes(iptc_data.len() as u64),
            },
        ));
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_1_only_is_reported_as_other() {
        // 1:90 (coded character set: UTF-8) and nothing else
        let data = [0x1C, 1, 90, 0, 3, 0x1B, 0x25, 0x47];
        let entries = collect_details(&data);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, MetadataKind::Other);
        assert_eq!(entries[0].1.value, DetailValue::Bytes(data.len() as u64));
    }

    fn make_dataset(rec: u8, num: u8, data: &[u8]) -> Vec<u8> {
        let mut out = vec![IPTC_TAG_MARKER, rec, num];
        out.extend_from_slice(&(data.len() as u16).to_be_bytes());
        out.extend_from_slice(data);
        out
    }

    #[test]
    fn test_iptc_datetime_merging() {
        let mut data = Vec::new();
        data.extend_from_slice(&make_dataset(2, 55, b"20260102"));
        data.extend_from_slice(&make_dataset(2, 60, b"030405+0900"));

        let entries = collect_details(&data);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, MetadataKind::DateTime);
        assert_eq!(entries[0].1.field, Field::Created);
        assert_eq!(
            entries[0].1.value,
            DetailValue::Text("2026-01-02 03:04:05".to_string())
        );
    }

    #[test]
    fn test_iptc_date_only_without_time() {
        let data = make_dataset(2, 55, b"20260102");
        let entries = collect_details(&data);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, MetadataKind::DateTime);
        assert_eq!(entries[0].1.field, Field::Created);
        assert_eq!(
            entries[0].1.value,
            DetailValue::Text("2026-01-02".to_string())
        );
    }

    #[test]
    fn test_iptc_unreadable_fallback() {
        let bad_data = b"not a valid iptc stream";
        let entries = collect_details(bad_data);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, MetadataKind::Other);
        assert_eq!(entries[0].1.field, Field::Other);
        assert_eq!(entries[0].1.name.as_deref(), Some("IPTC"));
        assert_eq!(
            entries[0].1.value,
            DetailValue::Bytes(bad_data.len() as u64)
        );
    }

    #[test]
    fn test_record1_ignored() {
        let mut data = Vec::new();
        data.extend_from_slice(&make_dataset(1, 90, &[0x1B, 0x25, 0x47]));
        data.extend_from_slice(&make_dataset(2, 80, b"Photographer"));

        let entries = collect_details(&data);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, MetadataKind::Author);
        assert_eq!(entries[0].1.field, Field::Author);
        assert_eq!(
            entries[0].1.value,
            DetailValue::Text("Photographer".to_string())
        );
    }

    #[test]
    fn test_iptc_control_characters_and_invalid_utf8() {
        let mut data = Vec::new();
        // Record 2, Dataset 0 (record version, e.g. version 2 = [0x00, 0x02], binary)
        data.extend_from_slice(&make_dataset(2, 0, &[0x00, 0x02]));
        // Record 2, Dataset 80 (Author with control character NUL or BEL)
        data.extend_from_slice(&make_dataset(2, 80, b"Author\x07Name"));
        // Record 2, Dataset 90 (City with invalid UTF-8)
        data.extend_from_slice(&make_dataset(2, 90, &[0xFF, 0xFE, 0xFD]));
        // Record 2, Dataset 101 (Country with valid text)
        data.extend_from_slice(&make_dataset(2, 101, b"Japan"));

        let entries = collect_details(&data);
        assert_eq!(entries.len(), 4);

        // Version (2:0) -> Other, Bytes(2)
        assert_eq!(entries[0].0, MetadataKind::Other);
        assert_eq!(entries[0].1.field, Field::Other);
        assert_eq!(entries[0].1.name.as_deref(), Some("2:0"));
        assert_eq!(entries[0].1.value, DetailValue::Bytes(2));

        // Author (2:80) with control char -> Author, Bytes(11)
        assert_eq!(entries[1].0, MetadataKind::Author);
        assert_eq!(entries[1].1.field, Field::Author);
        assert_eq!(entries[1].1.value, DetailValue::Bytes(11));

        // City (2:90) with invalid UTF-8 -> Location, Bytes(3)
        assert_eq!(entries[2].0, MetadataKind::Location);
        assert_eq!(entries[2].1.field, Field::City);
        assert_eq!(entries[2].1.value, DetailValue::Bytes(3));

        // Country (2:101) valid -> Location, Text("Japan")
        assert_eq!(entries[3].0, MetadataKind::Location);
        assert_eq!(entries[3].1.field, Field::Country);
        assert_eq!(entries[3].1.value, DetailValue::Text("Japan".to_string()));
    }
}
