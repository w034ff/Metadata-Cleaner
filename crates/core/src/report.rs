//! Report data structures and detail types for inspected images (design §4.5, §4.7).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::detect::Format;

/// Categories for found metadata, ordered as specified in design §4.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MetadataKind {
    Location,
    DateTime,
    Device,
    Author,
    Software,
    Comment,
    Thumbnail,
    History,
    Other,
}

/// Specific metadata field keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Field {
    Latitude,
    Longitude,
    City,
    State,
    Country,
    Taken,
    Created,
    Modified,
    CameraMake,
    CameraModel,
    SerialNumber,
    MakerNote,
    Author,
    Copyright,
    Software,
    PdfProducer,
    Title,
    Description,
    Comment,
    Subject,
    Keywords,
    Thumbnail,
    EarlierVersions,
    Xmp,
    Other,
}

/// Representation of a metadata detail value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum DetailValue {
    Text(String),
    Bytes(#[ts(type = "number")] u64),
    Count(u32),
}

/// A single metadata entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DetailEntry {
    pub field: Field,
    pub name: Option<String>,
    pub value: DetailValue,
}

/// A group of detail entries sharing a metadata category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DetailGroup {
    pub kind: MetadataKind,
    pub entries: Vec<DetailEntry>,
}

/// Units for kept resolution information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ResolutionUnit {
    Inch,
    Centimeter,
    Meter,
}

/// Information preserved from the original file when cleaning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum KeptInfo {
    Orientation {
        value: u8,
    },
    ColorProfile {
        description: Option<String>,
    },
    Resolution {
        x: u32,
        y: u32,
        unit: ResolutionUnit,
    },
}

/// Quick inspection result (design §4.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub format: Format,
    pub kinds: Vec<MetadataKind>,
    pub kept: Vec<KeptInfo>,
}

/// Detailed metadata inspection result (design §4.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    pub groups: Vec<DetailGroup>,
    pub kept: Vec<KeptInfo>,
    pub truncated: bool,
}

/// Maximum allowed character length for a `DetailValue::Text` string.
pub const MAX_DETAIL_VALUE_CHARS: usize = 200;

/// Maximum total detail entries across all groups before truncation.
pub const MAX_DETAIL_ENTRIES: usize = 100;

/// Truncates a detail text string if it exceeds `MAX_DETAIL_VALUE_CHARS` characters.
pub fn truncate_detail_text(s: &str) -> String {
    if s.chars().count() > MAX_DETAIL_VALUE_CHARS {
        let mut out: String = s.chars().take(MAX_DETAIL_VALUE_CHARS).collect();
        out.push('…');
        out
    } else {
        s.to_string()
    }
}

/// Groups detail entries by [`MetadataKind`], applying value truncation and entry count limits.
pub fn build_details(
    raw_entries: Vec<(MetadataKind, DetailEntry)>,
    kept: Vec<KeptInfo>,
) -> Details {
    let truncated = raw_entries.len() > MAX_DETAIL_ENTRIES;
    let entries_to_keep = if truncated {
        &raw_entries[..MAX_DETAIL_ENTRIES]
    } else {
        &raw_entries[..]
    };

    let all_kinds = [
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

    let mut groups = Vec::new();
    for kind in all_kinds {
        let entries: Vec<DetailEntry> = entries_to_keep
            .iter()
            .filter(|(k, _)| *k == kind)
            .map(|(_, entry)| {
                let value = match &entry.value {
                    DetailValue::Text(t) => DetailValue::Text(truncate_detail_text(t)),
                    other => other.clone(),
                };
                DetailEntry {
                    field: entry.field,
                    name: entry.name.clone(),
                    value,
                }
            })
            .collect();

        if !entries.is_empty() {
            groups.push(DetailGroup { kind, entries });
        }
    }

    Details {
        groups,
        kept: sort_kept(kept),
        truncated,
    }
}

/// Sorts kept info in canonical order: Orientation, ColorProfile, Resolution (at most 1 of each).
pub fn sort_kept(kept: Vec<KeptInfo>) -> Vec<KeptInfo> {
    let mut orientation = None;
    let mut color_profile = None;
    let mut resolution = None;

    for item in kept {
        match item {
            KeptInfo::Orientation { value } if orientation.is_none() => {
                orientation = Some(KeptInfo::Orientation { value })
            }
            KeptInfo::ColorProfile { description } if color_profile.is_none() => {
                color_profile = Some(KeptInfo::ColorProfile { description });
            }
            KeptInfo::Resolution { x, y, unit } if resolution.is_none() => {
                resolution = Some(KeptInfo::Resolution { x, y, unit });
            }
            _ => {}
        }
    }

    let mut result = Vec::new();
    if let Some(o) = orientation {
        result.push(o);
    }
    if let Some(cp) = color_profile {
        result.push(cp);
    }
    if let Some(res) = resolution {
        result.push(res);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_detail_text() {
        let short = "Hello World";
        assert_eq!(truncate_detail_text(short), "Hello World");

        let exact = "a".repeat(200);
        assert_eq!(truncate_detail_text(&exact), exact);

        let long = "a".repeat(201);
        let expected = format!("{}…", "a".repeat(200));
        assert_eq!(truncate_detail_text(&long), expected);
    }

    #[test]
    fn test_build_details_truncation() {
        let mut entries = Vec::new();
        for i in 0..101 {
            entries.push((
                MetadataKind::Other,
                DetailEntry {
                    field: Field::Other,
                    name: Some(format!("tag_{i}")),
                    value: DetailValue::Text(format!("val_{i}")),
                },
            ));
        }

        let details = build_details(entries, Vec::new());
        assert!(details.truncated);
        assert_eq!(details.groups.len(), 1);
        assert_eq!(details.groups[0].entries.len(), 100);
        assert_eq!(
            details.groups[0].entries[99].name.as_deref(),
            Some("tag_99")
        );
    }

    #[test]
    fn test_sort_kept_order() {
        let kept = vec![
            KeptInfo::Resolution {
                x: 300,
                y: 300,
                unit: ResolutionUnit::Inch,
            },
            KeptInfo::Orientation { value: 6 },
            KeptInfo::ColorProfile {
                description: Some("sRGB".to_string()),
            },
        ];

        let sorted = sort_kept(kept);
        assert_eq!(
            sorted,
            vec![
                KeptInfo::Orientation { value: 6 },
                KeptInfo::ColorProfile {
                    description: Some("sRGB".to_string())
                },
                KeptInfo::Resolution {
                    x: 300,
                    y: 300,
                    unit: ResolutionUnit::Inch
                },
            ]
        );
    }
}
