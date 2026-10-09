//! WebP (RIFF) chunk walker and metadata stripper (design §4.4).

use crate::error::CoreError;

const KEEP_CHUNKS: [&[u8; 4]; 7] = [
    b"VP8 ", b"VP8L", b"VP8X", b"ALPH", b"ANIM", b"ANMF", b"ICCP",
];

const FLAG_XMP: u8 = 0x04;
const FLAG_EXIF: u8 = 0x08;
const EXIF_PREFIX: &[u8] = b"Exif\0\0";

#[derive(Debug, Clone, Copy)]
struct Chunk<'a> {
    id: [u8; 4],
    data: &'a [u8],
    raw_bytes: &'a [u8],
}

/// Parsed WebP file structure borrowing from the original data.
#[derive(Debug)]
pub struct Webp<'a> {
    chunks: Vec<Chunk<'a>>,
    trailing: &'a [u8],
}

/// Parses a WebP byte stream.
///
/// Returns [`CoreError::DecodeFailed`] if RIFF/WEBP header is missing,
/// RIFF size exceeds data, or any chunk header/payload exceeds the RIFF boundary.
pub fn parse(data: &[u8]) -> Result<Webp<'_>, CoreError> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return Err(CoreError::DecodeFailed);
    }
    let riff_payload_len = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let riff_end = match 8usize.checked_add(riff_payload_len) {
        Some(e) if e <= data.len() => e,
        _ => return Err(CoreError::DecodeFailed),
    };

    let mut pos = 12;
    let mut chunks = Vec::new();

    while pos < riff_end {
        if pos + 8 > riff_end {
            return Err(CoreError::DecodeFailed);
        }
        let id: [u8; 4] = [data[pos], data[pos + 1], data[pos + 2], data[pos + 3]];
        let len = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
            as usize;

        let data_end = match pos.checked_add(8).and_then(|p| p.checked_add(len)) {
            Some(e) if e <= riff_end => e,
            _ => return Err(CoreError::DecodeFailed),
        };
        let padded_end = match data_end.checked_add(len & 1) {
            Some(e) if e <= riff_end => e,
            _ => return Err(CoreError::DecodeFailed),
        };

        chunks.push(Chunk {
            id,
            data: &data[pos + 8..data_end],
            raw_bytes: &data[pos..padded_end],
        });

        pos = padded_end;
    }

    Ok(Webp {
        chunks,
        trailing: &data[riff_end..],
    })
}

impl<'a> Webp<'a> {
    /// Returns the raw TIFF bytes of the first EXIF chunk, stripping `Exif\0\0` if present.
    pub fn exif(&self) -> Option<&'a [u8]> {
        for c in &self.chunks {
            if &c.id == b"EXIF" {
                let data = c.data;
                let tiff = data.strip_prefix(EXIF_PREFIX).unwrap_or(data);
                return Some(tiff);
            }
        }
        None
    }

    /// Returns byte slices of preserved image chunks except `VP8X`.
    pub fn image_chunks(&self) -> Vec<&'a [u8]> {
        self.chunks
            .iter()
            .filter(|c| KEEP_CHUNKS.contains(&&c.id) && &c.id != b"VP8X")
            .map(|c| c.raw_bytes)
            .collect()
    }

    /// The bytes past the RIFF size. [`strip`] never writes them.
    pub fn trailing(&self) -> &'a [u8] {
        self.trailing
    }

    /// Returns dropped metadata chunks in file order as `(id, data)`.
    pub fn dropped_chunks(&self) -> Vec<([u8; 4], &'a [u8])> {
        self.chunks
            .iter()
            .filter(|c| !KEEP_CHUNKS.contains(&&c.id))
            .map(|c| (c.id, c.data))
            .collect()
    }

    /// Returns the data payload of the `ICCP` chunk, if present.
    pub fn iccp(&self) -> Option<&'a [u8]> {
        self.chunks
            .iter()
            .find(|c| &c.id == b"ICCP")
            .map(|c| c.data)
    }
}

/// Strips metadata chunks from the WebP. When the file has `VP8X` and an
/// `EXIF` chunk and `kept_exif` is given, appends one `EXIF` chunk holding it
/// after every kept chunk.
pub fn strip(webp: &Webp<'_>, kept_exif: Option<&[u8]>) -> Vec<u8> {
    let has_vp8x = webp.chunks.iter().any(|c| &c.id == b"VP8X");
    let will_write_exif = kept_exif.is_some() && has_vp8x && webp.exif().is_some();

    let mut body = b"WEBP".to_vec();

    for c in &webp.chunks {
        if &c.id == b"VP8X" {
            let mut vp8x_data = c.data.to_vec();
            if !vp8x_data.is_empty() {
                vp8x_data[0] &= !FLAG_XMP;
                if will_write_exif {
                    vp8x_data[0] |= FLAG_EXIF;
                } else {
                    vp8x_data[0] &= !FLAG_EXIF;
                }
            }
            body.extend_from_slice(b"VP8X");
            body.extend_from_slice(&(vp8x_data.len() as u32).to_le_bytes());
            body.extend_from_slice(&vp8x_data);
            if vp8x_data.len() & 1 == 1 {
                body.push(0);
            }
        } else if KEEP_CHUNKS.contains(&&c.id) {
            body.extend_from_slice(c.raw_bytes);
        }
    }

    if will_write_exif && let Some(exif_bytes) = kept_exif {
        body.extend_from_slice(b"EXIF");
        body.extend_from_slice(&(exif_bytes.len() as u32).to_le_bytes());
        body.extend_from_slice(exif_bytes);
        if exif_bytes.len() & 1 == 1 {
            body.push(0);
        }
    }

    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = id.to_vec();
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        if data.len() & 1 == 1 {
            out.push(0);
        }
        out
    }

    fn build_riff(chunks: &[Vec<u8>], trailer: &[u8]) -> Vec<u8> {
        let mut body = b"WEBP".to_vec();
        for c in chunks {
            body.extend_from_slice(c);
        }
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out.extend_from_slice(trailer);
        out
    }

    #[test]
    fn test_riff_size_beyond_file_decode_failed() {
        let mut data = b"RIFF....WEBP".to_vec();
        data[4..8].copy_from_slice(&1000u32.to_le_bytes());
        assert_eq!(parse(&data).err(), Some(CoreError::DecodeFailed));
    }

    #[test]
    fn test_incomplete_chunk_header_decode_failed() {
        let mut data = b"RIFF....WEBP".to_vec();
        data.extend_from_slice(b"VP8"); // only 3 bytes, less than 8-byte header
        let riff_len = (data.len() - 8) as u32;
        data[4..8].copy_from_slice(&riff_len.to_le_bytes());
        assert_eq!(parse(&data).err(), Some(CoreError::DecodeFailed));
    }

    #[test]
    fn test_odd_length_chunk_padding() {
        // Chunk with 3 bytes of data (odd length) -> 1 byte padding
        let chunk = make_chunk(b"VP8L", &[1, 2, 3]);
        let data = build_riff(&[chunk], &[]);

        let parsed = parse(&data).expect("should parse odd-length chunk with padding");
        assert_eq!(parsed.chunks.len(), 1);
        assert_eq!(parsed.chunks[0].data.len(), 3);
        assert_eq!(parsed.chunks[0].raw_bytes.len(), 12); // 4 + 4 + 3 + 1
    }

    #[test]
    fn test_exif_with_prefix_stripped() {
        let exif_data = b"Exif\0\0II*\0\x08\0\0\0";
        let exif_chunk = make_chunk(b"EXIF", exif_data);
        let data = build_riff(&[exif_chunk], &[]);

        let parsed = parse(&data).expect("should parse");
        assert_eq!(parsed.exif(), Some(b"II*\0\x08\0\0\0".as_slice()));
    }

    #[test]
    fn test_exif_without_prefix_kept_as_is() {
        let exif_data = b"II*\0\x08\0\0\0";
        let exif_chunk = make_chunk(b"EXIF", exif_data);
        let data = build_riff(&[exif_chunk], &[]);

        let parsed = parse(&data).expect("should parse");
        assert_eq!(parsed.exif(), Some(b"II*\0\x08\0\0\0".as_slice()));
    }

    #[test]
    fn test_no_vp8x_does_not_write_kept_exif() {
        // WebP without VP8X (e.g. simple lossy VP8), carrying an EXIF chunk
        // that the format does not allow there
        let vp8 = make_chunk(b"VP8 ", &[0; 10]);
        let exif = make_chunk(b"EXIF", b"II*\0ORIG");
        let data = build_riff(&[vp8, exif], &[]);

        let parsed = parse(&data).expect("should parse");
        let stripped = strip(&parsed, Some(b"II*\0KEPT"));
        assert!(!stripped.windows(4).any(|w| w == b"EXIF"));
    }

    #[test]
    fn test_no_original_exif_does_not_write_kept_exif() {
        let vp8x = make_chunk(b"VP8X", &[0u8; 10]);
        let vp8l = make_chunk(b"VP8L", &[0xAA; 10]);
        let data = build_riff(&[vp8x, vp8l], &[]);

        let parsed = parse(&data).expect("should parse");
        let stripped = strip(&parsed, Some(b"II*\0KEPT"));
        assert!(!stripped.windows(4).any(|w| w == b"EXIF"));
        assert_eq!(stripped[20] & FLAG_EXIF, 0);
    }

    #[test]
    fn test_alph_chunk_sequence_preserved() {
        // As noted in design §11.1, ALPH precedes VP8 for lossy images with alpha
        let mut vp8x_data = vec![0u8; 10];
        vp8x_data[0] = 0x10; // Alpha flag
        let vp8x = make_chunk(b"VP8X", &vp8x_data);
        let alph = make_chunk(b"ALPH", &[0x01, 0x02, 0x03, 0x04]);
        let vp8 = make_chunk(b"VP8 ", &[0xAA; 10]);
        let xmp = make_chunk(b"XMP ", b"<xmp/>");

        let data = build_riff(&[vp8x, alph.clone(), vp8.clone(), xmp], &[]);
        let parsed = parse(&data).expect("should parse");

        let img_chunks = parsed.image_chunks();
        assert_eq!(img_chunks.len(), 2); // ALPH and VP8 (VP8X excluded)
        assert!(img_chunks.iter().any(|c| c.starts_with(b"ALPH")));

        let stripped = strip(&parsed, None);
        assert!(!stripped.windows(4).any(|w| w == b"XMP "));
        assert!(stripped.windows(4).any(|w| w == b"ALPH"));
        assert!(stripped.windows(4).any(|w| w == b"VP8 "));
    }

    #[test]
    fn test_vp8x_flags_updated() {
        // VP8X with both XMP (0x04) and EXIF (0x08) set
        let mut vp8x_data = vec![0u8; 10];
        vp8x_data[0] = FLAG_XMP | FLAG_EXIF;
        let vp8x = make_chunk(b"VP8X", &vp8x_data);
        let vp8l = make_chunk(b"VP8L", &[0; 8]);
        let exif = make_chunk(b"EXIF", b"II*\0old");

        let data = build_riff(&[vp8x, vp8l, exif], &[]);
        let parsed = parse(&data).expect("should parse");

        // Case 1: with kept_exif -> XMP cleared (0), EXIF set (1)
        let kept = b"II*\0NEW_EXIF";
        let stripped_with_exif = strip(&parsed, Some(kept));
        // VP8X payload starts at index 12 (RIFF:4, len:4, WEBP:4) + 8 (VP8X:4, len:4) = 20
        assert_eq!(stripped_with_exif[20] & FLAG_XMP, 0);
        assert_ne!(stripped_with_exif[20] & FLAG_EXIF, 0);
        // EXIF chunk is at the very end
        assert!(
            stripped_with_exif.ends_with(kept)
                || stripped_with_exif[..stripped_with_exif.len() - 1].ends_with(kept)
        );

        // Case 2: without kept_exif -> both XMP and EXIF cleared
        let stripped_without_exif = strip(&parsed, None);
        assert_eq!(stripped_without_exif[20] & FLAG_XMP, 0);
        assert_eq!(stripped_without_exif[20] & FLAG_EXIF, 0);
        assert!(!stripped_without_exif.windows(4).any(|w| w == b"EXIF"));
    }

    #[test]
    fn test_trailer_after_riff_dropped() {
        let trailer = b"EXTRA_TRAILER_BYTES";
        let vp8 = make_chunk(b"VP8 ", &[0; 10]);
        let data = build_riff(&[vp8], trailer);

        let parsed = parse(&data).expect("should parse");
        assert_eq!(parsed.trailing(), trailer);

        let stripped = strip(&parsed, None);
        assert!(!stripped.windows(trailer.len()).any(|w| w == trailer));
    }
}
