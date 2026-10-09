//! JPEG segment walker and metadata stripper (design §4.2).

use crate::error::CoreError;

const EXIF_PREFIX: &[u8] = b"Exif\0\0";
const JFIF_PREFIX: &[u8] = b"JFIF\0";
const ICC_PREFIX: &[u8] = b"ICC_PROFILE\0";
const ADOBE_PREFIX: &[u8] = b"Adobe";
const MAX_APP1_PAYLOAD: usize = 65533; // 65535 - 2 bytes length header

#[derive(Debug, Clone, Copy)]
struct Segment<'a> {
    marker: u8,
    /// Raw bytes of the segment starting at `0xFF, marker` and including
    /// the length header and payload (and entropy-coded data for SOS).
    bytes: &'a [u8],
}

impl<'a> Segment<'a> {
    /// Returns the segment payload excluding the marker and the 2-byte length header.
    /// For markers without a length header, returns an empty slice.
    fn payload(&self) -> &'a [u8] {
        if self.bytes.len() >= 4 {
            &self.bytes[4..]
        } else {
            &[]
        }
    }
}

/// Parsed JPEG file structure borrowing from the original data.
#[derive(Debug)]
pub struct Jpeg<'a> {
    segments: Vec<Segment<'a>>,
    trailing: &'a [u8],
}

/// Parses a JPEG byte stream.
///
/// Stops at the EOI marker. Returns [`CoreError::DecodeFailed`] if SOI is missing,
/// a marker is invalid, segment length is malformed, or EOI is not reached.
pub fn parse(data: &[u8]) -> Result<Jpeg<'_>, CoreError> {
    if !data.starts_with(&[0xFF, 0xD8]) {
        return Err(CoreError::DecodeFailed);
    }
    let mut pos = 2;
    let mut segments = Vec::new();
    loop {
        if pos >= data.len() || data[pos] != 0xFF {
            return Err(CoreError::DecodeFailed);
        }
        while pos < data.len() && data[pos] == 0xFF {
            pos += 1;
        }
        if pos >= data.len() {
            return Err(CoreError::DecodeFailed);
        }
        let marker = data[pos];
        pos += 1;
        if marker == 0x00 {
            // 0xFF 0x00 is byte-stuffing, not a valid segment marker
            return Err(CoreError::DecodeFailed);
        }
        let start = pos - 2;

        match marker {
            0xD9 => {
                // EOI (End of Image)
                return Ok(Jpeg {
                    segments,
                    trailing: &data[pos..],
                });
            }
            0x01 | 0xD0..=0xD7 => {
                // TEM (0x01) and RST0..=RST7 (0xD0..=0xD7): markers without a length field
                segments.push(Segment {
                    marker,
                    bytes: &data[start..pos],
                });
                continue;
            }
            _ => {}
        }

        if pos + 2 > data.len() {
            return Err(CoreError::DecodeFailed);
        }
        let len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
        if len < 2 || pos + len > data.len() {
            return Err(CoreError::DecodeFailed);
        }
        pos += len;

        if marker == 0xDA {
            // SOS: scan entropy-coded scan data until the next unescaped marker
            loop {
                if pos + 1 >= data.len() {
                    return Err(CoreError::DecodeFailed);
                }
                if data[pos] == 0xFF {
                    let n = data[pos + 1];
                    if n == 0x00 || (0xD0..=0xD7).contains(&n) {
                        pos += 2;
                        continue;
                    }
                    break;
                }
                pos += 1;
            }
        }

        segments.push(Segment {
            marker,
            bytes: &data[start..pos],
        });
    }
}

impl<'a> Jpeg<'a> {
    /// Returns the TIFF bytes of the first EXIF segment, stripped of `Exif\0\0`.
    pub fn exif(&self) -> Option<&'a [u8]> {
        for s in &self.segments {
            if s.marker == 0xE1 {
                let p = s.payload();
                if p.starts_with(EXIF_PREFIX) {
                    return Some(&p[EXIF_PREFIX.len()..]);
                }
            }
        }
        None
    }

    /// Returns byte slices of non-metadata segments and preserved ICC/Adobe segments.
    pub fn image_segments(&self) -> Vec<&'a [u8]> {
        self.segments
            .iter()
            .filter(|s| is_image_segment(s))
            .map(|s| s.bytes)
            .collect()
    }

    /// The bytes after EOI, such as an MPF image or a motion photo's video. [`strip`] never writes them.
    pub fn trailing(&self) -> &'a [u8] {
        self.trailing
    }
}

fn is_metadata_marker(marker: u8) -> bool {
    matches!(marker, 0xE0..=0xEF | 0xFE)
}

fn is_image_segment(s: &Segment<'_>) -> bool {
    if !is_metadata_marker(s.marker) {
        return true;
    }
    let p = s.payload();
    if s.marker == 0xE2 && p.starts_with(ICC_PREFIX) {
        return true;
    }
    if s.marker == 0xEE && p.starts_with(ADOBE_PREFIX) {
        return true;
    }
    false
}

/// Strips metadata from the JPEG and replaces the first EXIF segment if `kept_exif` is given.
pub fn strip(jpeg: &Jpeg<'_>, kept_exif: Option<&[u8]>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0xFF, 0xD8]); // SOI

    let original_has_exif = jpeg.exif().is_some();
    let mut first_exif_seen = false;

    for s in &jpeg.segments {
        if !is_metadata_marker(s.marker) {
            out.extend_from_slice(s.bytes);
            continue;
        }

        let p = s.payload();
        match s.marker {
            0xE0 => {
                // APP0: keep JFIF only with thumbnail dimensions cleared to 0
                if p.starts_with(JFIF_PREFIX) && p.len() >= 14 {
                    let mut jfif = p[..14].to_vec();
                    jfif[12] = 0;
                    jfif[13] = 0;
                    let seg_len = (14 + 2) as u16;
                    out.extend_from_slice(&[0xFF, 0xE0]);
                    out.extend_from_slice(&seg_len.to_be_bytes());
                    out.extend_from_slice(&jfif);
                }
                // Other APP0 (JFXX, or JFIF < 14 bytes) are dropped
            }
            0xE1 => {
                // APP1: replace the first Exif segment with kept_exif
                if p.starts_with(EXIF_PREFIX)
                    && !first_exif_seen
                    && original_has_exif
                    && let Some(exif_bytes) = kept_exif
                    && EXIF_PREFIX.len() + exif_bytes.len() <= MAX_APP1_PAYLOAD
                {
                    first_exif_seen = true;
                    let total_payload = EXIF_PREFIX.len() + exif_bytes.len();
                    let seg_len = (total_payload + 2) as u16;
                    out.extend_from_slice(&[0xFF, 0xE1]);
                    out.extend_from_slice(&seg_len.to_be_bytes());
                    out.extend_from_slice(EXIF_PREFIX);
                    out.extend_from_slice(exif_bytes);
                } else if p.starts_with(EXIF_PREFIX) {
                    first_exif_seen = true;
                }
                // Other APP1 (XMP, etc.) and subsequent Exif segments are dropped
            }
            0xE2 if p.starts_with(ICC_PREFIX) => {
                // APP2: keep ICC profile, drop MPF and others
                out.extend_from_slice(s.bytes);
            }
            0xEE if p.starts_with(ADOBE_PREFIX) => {
                // APP14: keep Adobe segment
                out.extend_from_slice(s.bytes);
            }
            _ => {
                // Other APPn and COM (0xFE) are dropped
            }
        }
    }

    out.extend_from_slice(&[0xFF, 0xD9]); // EOI
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_segment(marker: u8, payload: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0xFF, marker];
        let len = (payload.len() + 2) as u16;
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    fn minimal_jpeg(extra_segments: &[Vec<u8>], trailer: &[u8]) -> Vec<u8> {
        let mut out = vec![0xFF, 0xD8]; // SOI
        for s in extra_segments {
            out.extend_from_slice(s);
        }
        // Minimal DQT and SOF0
        out.extend_from_slice(&make_segment(0xDB, &[0x00; 65]));
        out.extend_from_slice(&make_segment(
            0xC0,
            &[0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x11, 0x00],
        ));
        // Minimal SOS with dummy scan data
        out.extend_from_slice(&make_segment(0xDA, &[0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]));
        out.extend_from_slice(&[0x12, 0x34]); // Entropy data
        out.extend_from_slice(&[0xFF, 0xD9]); // EOI
        out.extend_from_slice(trailer);
        out
    }

    #[test]
    fn test_fill_ff_skipping() {
        let mut jpeg = vec![0xFF, 0xD8];
        jpeg.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xE0, 0x00, 0x10]);
        let jfif = b"JFIF\0\x01\x01\x00\x00\x01\x00\x01\x02\x03".to_vec();
        jpeg.extend_from_slice(&jfif);
        // Add DQT, SOF, SOS, EOI
        jpeg.extend_from_slice(&make_segment(0xDB, &[0x00; 65]));
        jpeg.extend_from_slice(&make_segment(0xDA, &[0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]));
        jpeg.extend_from_slice(&[0x00, 0xFF, 0xD9]);

        let parsed = parse(&jpeg).expect("should parse despite fill FF bytes");
        let stripped = strip(&parsed, None);
        assert!(!stripped.windows(3).any(|w| w == [0xFF, 0xFF, 0xFF]));
    }

    #[test]
    fn test_tem_and_rst_markers() {
        let mut jpeg = vec![0xFF, 0xD8];
        jpeg.extend_from_slice(&[0xFF, 0x01]); // TEM
        jpeg.extend_from_slice(&[0xFF, 0xD0]); // RST0
        jpeg.extend_from_slice(&[0xFF, 0xD7]); // RST7
        jpeg.extend_from_slice(&make_segment(0xDB, &[0x00; 65]));
        jpeg.extend_from_slice(&make_segment(0xDA, &[0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]));
        jpeg.extend_from_slice(&[0xAA, 0xFF, 0xD9]);

        let parsed = parse(&jpeg).expect("should parse TEM and RST markers");
        assert_eq!(parsed.segments.len(), 5);
    }

    #[test]
    fn test_sos_scan_with_ff00_and_ffd0() {
        let mut jpeg = vec![0xFF, 0xD8];
        jpeg.extend_from_slice(&make_segment(0xDA, &[0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]));
        // Entropy data containing FF 00 (escaped FF) and FF D0 (restart marker)
        jpeg.extend_from_slice(&[0x11, 0xFF, 0x00, 0x22, 0xFF, 0xD0, 0x33]);
        jpeg.extend_from_slice(&[0xFF, 0xD9]); // EOI

        let parsed = parse(&jpeg).expect("should parse scan with FF 00 and restart marker");
        assert_eq!(parsed.segments.len(), 1);
        let sos = parsed.segments[0];
        assert_eq!(sos.marker, 0xDA);
        assert!(sos.bytes.contains(&0x33));
    }

    #[test]
    fn test_bad_lengths_and_truncation() {
        // Bad SOI
        assert_eq!(parse(&[0x00, 0x00]).err(), Some(CoreError::DecodeFailed));

        // Length < 2
        let bad_len_0 = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x00];
        assert_eq!(parse(&bad_len_0).err(), Some(CoreError::DecodeFailed));
        let bad_len_1 = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x01];
        assert_eq!(parse(&bad_len_1).err(), Some(CoreError::DecodeFailed));

        // Length beyond file
        let len_beyond = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x01, 0x02];
        assert_eq!(parse(&len_beyond).err(), Some(CoreError::DecodeFailed));

        // Missing marker FF
        let no_marker = vec![0xFF, 0xD8, 0x00, 0x01, 0x02, 0x03];
        assert_eq!(parse(&no_marker).err(), Some(CoreError::DecodeFailed));

        // Truncated at fill byte
        let trunc_fill = vec![0xFF, 0xD8, 0xFF];
        assert_eq!(parse(&trunc_fill).err(), Some(CoreError::DecodeFailed));

        // Missing EOI
        let no_eoi = vec![
            0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, 0x12,
        ];
        assert_eq!(parse(&no_eoi).err(), Some(CoreError::DecodeFailed));
    }

    #[test]
    fn test_jfif_short_payload_dropped() {
        // JFIF payload less than 14 bytes
        let short_jfif = make_segment(0xE0, b"JFIF\0\x01\x01\x00");
        let data = minimal_jpeg(&[short_jfif], &[]);
        let parsed = parse(&data).unwrap();
        let stripped = strip(&parsed, None);
        assert!(!stripped.windows(5).any(|w| w == b"JFIF\0"));
    }

    #[test]
    fn test_jfxx_dropped() {
        let jfxx = make_segment(0xE0, b"JFXX\0\x10\x00");
        let data = minimal_jpeg(&[jfxx], &[]);
        let parsed = parse(&data).unwrap();
        let stripped = strip(&parsed, None);
        assert!(!stripped.windows(5).any(|w| w == b"JFXX\0"));
    }

    #[test]
    fn test_multiple_exif_app1_only_first_replaced() {
        let exif1 = make_segment(0xE1, b"Exif\0\0II*\0\x08\0\0\0\0\0\0\0");
        let exif2 = make_segment(0xE1, b"Exif\0\0MM\0*\0\0\0\x08\0\0\0\0");
        let data = minimal_jpeg(&[exif1, exif2], &[]);
        let parsed = parse(&data).unwrap();
        assert!(parsed.exif().is_some());

        let kept = b"II*\0NEW_EXIF";
        let stripped = strip(&parsed, Some(kept));

        // Count occurrences of APP1 Exif marker
        let exif_count = stripped.windows(6).filter(|w| *w == b"Exif\0\0").count();
        assert_eq!(exif_count, 1);
        assert!(stripped.windows(kept.len()).any(|w| w == kept));
    }

    #[test]
    fn test_too_large_kept_exif_dropped() {
        let exif1 = make_segment(0xE1, b"Exif\0\0II*\0\x08\0\0\0\0\0\0\0");
        let data = minimal_jpeg(&[exif1], &[]);
        let parsed = parse(&data).unwrap();

        // Length exceeding 65533 - 6 = 65527 bytes
        let giant_exif = vec![0x42; 65528];
        let stripped = strip(&parsed, Some(&giant_exif));
        assert!(!stripped.windows(6).any(|w| w == b"Exif\0\0"));
    }

    #[test]
    fn test_no_original_exif_does_not_write_kept_exif() {
        let data = minimal_jpeg(&[], &[]);
        let parsed = parse(&data).unwrap();
        assert!(parsed.exif().is_none());

        let kept = b"II*\0NEW_EXIF";
        let stripped = strip(&parsed, Some(kept));
        assert!(!stripped.windows(6).any(|w| w == b"Exif\0\0"));
    }

    #[test]
    fn test_trailer_after_eoi_not_written() {
        let trailer = b"EXTRA_DATA_AFTER_EOI";
        let data = minimal_jpeg(&[], trailer);
        let parsed = parse(&data).unwrap();
        assert_eq!(parsed.trailing(), trailer);

        let stripped = strip(&parsed, None);
        assert!(!stripped.windows(trailer.len()).any(|w| w == trailer));
    }

    #[test]
    fn test_drop_com_and_xmp_keep_icc_adobe() {
        let com = make_segment(0xFE, b"comment");
        let xmp = make_segment(0xE1, b"http://ns.adobe.com/xap/1.0/\0xmp-body");
        let icc = make_segment(0xE2, b"ICC_PROFILE\0data");
        let mpf = make_segment(0xE2, b"MPF\0data");
        let adobe = make_segment(0xEE, b"Adobe\0data");
        let app3 = make_segment(0xE3, b"other app");

        let data = minimal_jpeg(&[com, xmp, icc.clone(), mpf, adobe.clone(), app3], &[]);
        let parsed = parse(&data).unwrap();

        let image_segs = parsed.image_segments();
        assert!(image_segs.iter().any(|s| s.starts_with(&icc)));
        assert!(image_segs.iter().any(|s| s.starts_with(&adobe)));

        let stripped = strip(&parsed, None);
        assert!(!stripped.windows(7).any(|w| w == b"comment"));
        assert!(!stripped.windows(4).any(|w| w == b"MPF\0"));
        assert!(!stripped.windows(9).any(|w| w == b"other app"));
        assert!(stripped.windows(icc.len()).any(|w| w == icc.as_slice()));
        assert!(stripped.windows(adobe.len()).any(|w| w == adobe.as_slice()));
    }
}
