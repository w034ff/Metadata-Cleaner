//! XMP metadata classification and detail entry extraction (design §4.5).

use crate::report::{DetailEntry, DetailValue, Field, MetadataKind};

/// Classifies an XMP byte stream into metadata categories without XML parsing.
///
/// Searches for predefined property prefixes and names. Returns `[MetadataKind::Other]`
/// if no known names match. The returned vector is deduplicated and sorted in canonical
/// order of [`MetadataKind`].
pub fn classify(xmp: &[u8]) -> Vec<MetadataKind> {
    let mut kinds = Vec::new();

    // Location: starts with exif:GPS
    if xmp.windows(8).any(|w| w == b"exif:GPS") {
        kinds.push(MetadataKind::Location);
    }

    // DateTime: xmp:CreateDate, xmp:ModifyDate, xmp:MetadataDate, photoshop:DateCreated
    const DATETIME_NEEDLES: [&[u8]; 4] = [
        b"xmp:CreateDate",
        b"xmp:ModifyDate",
        b"xmp:MetadataDate",
        b"photoshop:DateCreated",
    ];
    if DATETIME_NEEDLES
        .iter()
        .any(|needle| xmp.windows(needle.len()).any(|w| w == *needle))
    {
        kinds.push(MetadataKind::DateTime);
    }

    // Device: tiff:Make, tiff:Model, aux:SerialNumber
    const DEVICE_NEEDLES: [&[u8]; 3] = [b"tiff:Make", b"tiff:Model", b"aux:SerialNumber"];
    if DEVICE_NEEDLES
        .iter()
        .any(|needle| xmp.windows(needle.len()).any(|w| w == *needle))
    {
        kinds.push(MetadataKind::Device);
    }

    // Author: dc:creator, dc:rights
    const AUTHOR_NEEDLES: [&[u8]; 2] = [b"dc:creator", b"dc:rights"];
    if AUTHOR_NEEDLES
        .iter()
        .any(|needle| xmp.windows(needle.len()).any(|w| w == *needle))
    {
        kinds.push(MetadataKind::Author);
    }

    // Software: xmp:CreatorTool
    if xmp.windows(15).any(|w| w == b"xmp:CreatorTool") {
        kinds.push(MetadataKind::Software);
    }

    // Comment: dc:description, dc:title
    const COMMENT_NEEDLES: [&[u8]; 2] = [b"dc:description", b"dc:title"];
    if COMMENT_NEEDLES
        .iter()
        .any(|needle| xmp.windows(needle.len()).any(|w| w == *needle))
    {
        kinds.push(MetadataKind::Comment);
    }

    if kinds.is_empty() {
        vec![MetadataKind::Other]
    } else {
        kinds.sort();
        kinds.dedup();
        kinds
    }
}

/// Creates detail entries for an XMP packet.
///
/// For each matched [`MetadataKind`], produces one entry with `Field::Xmp` and the byte length.
pub fn xmp_detail_entries(xmp: &[u8]) -> Vec<(MetadataKind, DetailEntry)> {
    let kinds = classify(xmp);
    let len = xmp.len() as u64;
    kinds
        .into_iter()
        .map(|kind| {
            (
                kind,
                DetailEntry {
                    field: Field::Xmp,
                    name: None,
                    value: DetailValue::Bytes(len),
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_all_kinds() {
        let xmp = br#"<x:xmpmeta>
            <rdf:RDF>
                <rdf:Description exif:GPSLatitude="12"
                    xmp:CreateDate="2026-01-02"
                    tiff:Make="Example"
                    dc:creator="Author"
                    xmp:CreatorTool="Tool"
                    dc:description="Comment" />
            </rdf:RDF>
        </x:xmpmeta>"#;

        let kinds = classify(xmp);
        assert_eq!(
            kinds,
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
    fn test_classify_unmatched_returns_other() {
        let xmp =
            b"<x:xmpmeta><rdf:RDF><rdf:Description unknown:field=\"value\"/></rdf:RDF></x:xmpmeta>";
        assert_eq!(classify(xmp), vec![MetadataKind::Other]);
    }

    #[test]
    fn test_xmp_detail_entries_byte_length() {
        let xmp = b"<xmp:CreateDate>2026-01-02</xmp:CreateDate>";
        let entries = xmp_detail_entries(xmp);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, MetadataKind::DateTime);
        assert_eq!(entries[0].1.field, Field::Xmp);
        assert_eq!(entries[0].1.value, DetailValue::Bytes(xmp.len() as u64));
    }
}
