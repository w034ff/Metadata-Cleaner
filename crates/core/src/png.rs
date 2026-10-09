//! PNG chunk walker and metadata stripper (design §4.3).

use crate::error::CoreError;

const PNG_SIG: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// Chunks that carry image pixels, palette, color management, or APNG animation.
const KEEP_CHUNKS: [&[u8; 4]; 18] = [
    b"IHDR", b"PLTE", b"IDAT", b"IEND", b"tRNS", b"gAMA", b"cHRM", b"sRGB", b"iCCP", b"cICP",
    b"mDCV", b"cLLI", b"sBIT", b"bKGD", b"pHYs", b"acTL", b"fcTL", b"fdAT",
];

#[derive(Debug, Clone, Copy)]
struct Chunk<'a> {
    kind: [u8; 4],
    data: &'a [u8],
    raw_bytes: &'a [u8],
}

/// Parsed PNG file structure borrowing from the original data.
#[derive(Debug)]
pub struct Png<'a> {
    chunks: Vec<Chunk<'a>>,
    trailing: &'a [u8],
}

/// Parses a PNG byte stream.
///
/// Stops at the `IEND` chunk. Returns [`CoreError::DecodeFailed`] if the signature
/// is invalid, a chunk length exceeds the data, or `IEND` is missing.
pub fn parse(data: &[u8]) -> Result<Png<'_>, CoreError> {
    if !data.starts_with(PNG_SIG) {
        return Err(CoreError::DecodeFailed);
    }
    let mut pos = PNG_SIG.len();
    let mut chunks = Vec::new();
    let mut iend_found = false;

    while pos < data.len() {
        if pos + 12 > data.len() {
            return Err(CoreError::DecodeFailed);
        }
        let len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let kind: [u8; 4] = [data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]];

        let end = match pos.checked_add(12).and_then(|p| p.checked_add(len)) {
            Some(e) if e <= data.len() => e,
            _ => return Err(CoreError::DecodeFailed),
        };

        let chunk_data = &data[pos + 8..pos + 8 + len];
        let raw_bytes = &data[pos..end];
        chunks.push(Chunk {
            kind,
            data: chunk_data,
            raw_bytes,
        });

        pos = end;
        if &kind == b"IEND" {
            iend_found = true;
            break;
        }
    }

    if !iend_found {
        return Err(CoreError::DecodeFailed);
    }

    Ok(Png {
        chunks,
        trailing: &data[pos..],
    })
}

impl<'a> Png<'a> {
    /// Returns the raw TIFF bytes of the first `eXIf` chunk.
    pub fn exif(&self) -> Option<&'a [u8]> {
        for c in &self.chunks {
            if &c.kind == b"eXIf" {
                return Some(c.data);
            }
        }
        None
    }

    /// Returns raw byte slices of preserved image chunks (excluding `eXIf`).
    pub fn image_chunks(&self) -> Vec<&'a [u8]> {
        self.chunks
            .iter()
            .filter(|c| KEEP_CHUNKS.contains(&&c.kind))
            .map(|c| c.raw_bytes)
            .collect()
    }

    /// The bytes after IEND. [`strip`] never writes them.
    pub fn trailing(&self) -> &'a [u8] {
        self.trailing
    }

    /// Returns dropped metadata chunks in file order as `(kind, data)`.
    pub fn dropped_chunks(&self) -> Vec<([u8; 4], &'a [u8])> {
        self.chunks
            .iter()
            .filter(|c| !KEEP_CHUNKS.contains(&&c.kind))
            .map(|c| (c.kind, c.data))
            .collect()
    }

    /// Returns the data payload of the `iCCP` chunk, if present.
    pub fn iccp(&self) -> Option<&'a [u8]> {
        self.chunks
            .iter()
            .find(|c| &c.kind == b"iCCP")
            .map(|c| c.data)
    }

    /// Returns the data payload of the `pHYs` chunk, if present.
    pub fn phys(&self) -> Option<&'a [u8]> {
        self.chunks
            .iter()
            .find(|c| &c.kind == b"pHYs")
            .map(|c| c.data)
    }
}

/// Strips metadata chunks from the PNG and replaces the first `eXIf` chunk if `kept_exif` is given.
pub fn strip(png: &Png<'_>, kept_exif: Option<&[u8]>) -> Vec<u8> {
    let mut out = PNG_SIG.to_vec();
    let original_has_exif = png.exif().is_some();
    let mut first_exif_seen = false;

    for c in &png.chunks {
        if &c.kind == b"eXIf" {
            if !first_exif_seen
                && original_has_exif
                && let Some(exif_bytes) = kept_exif
            {
                first_exif_seen = true;
                out.extend_from_slice(&(exif_bytes.len() as u32).to_be_bytes());
                out.extend_from_slice(b"eXIf");
                out.extend_from_slice(exif_bytes);
                let mut hasher = crc32fast::Hasher::new();
                hasher.update(b"eXIf");
                hasher.update(exif_bytes);
                out.extend_from_slice(&hasher.finalize().to_be_bytes());
            } else {
                first_exif_seen = true;
            }
        } else if KEEP_CHUNKS.contains(&&c.kind) {
            out.extend_from_slice(c.raw_bytes);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(kind);
        hasher.update(data);
        out.extend_from_slice(&hasher.finalize().to_be_bytes());
        out
    }

    fn minimal_png(extra_chunks: &[Vec<u8>], trailer: &[u8]) -> Vec<u8> {
        let mut out = PNG_SIG.to_vec();
        // IHDR: 13 bytes
        let ihdr_data = [
            0x00, 0x00, 0x00, 0x01, // width = 1
            0x00, 0x00, 0x00, 0x01, // height = 1
            0x08, 0x06, 0x00, 0x00, 0x00, // 8-bit RGBA, no compression/filter/interlace
        ];
        out.extend_from_slice(&make_chunk(b"IHDR", &ihdr_data));
        for c in extra_chunks {
            out.extend_from_slice(c);
        }
        // Minimal IDAT
        out.extend_from_slice(&make_chunk(
            b"IDAT",
            &[0x78, 0x9C, 0x63, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01],
        ));
        // IEND
        out.extend_from_slice(&make_chunk(b"IEND", &[]));
        out.extend_from_slice(trailer);
        out
    }

    #[test]
    fn test_missing_iend_decode_failed() {
        let mut data = PNG_SIG.to_vec();
        data.extend_from_slice(&make_chunk(b"IHDR", &[0; 13]));
        assert_eq!(parse(&data).err(), Some(CoreError::DecodeFailed));
    }

    #[test]
    fn test_truncated_chunk_decode_failed() {
        let mut data = PNG_SIG.to_vec();
        // Chunk length says 100 bytes but file ends
        data.extend_from_slice(&100u32.to_be_bytes());
        data.extend_from_slice(b"IHDR");
        data.extend_from_slice(&[0; 10]);
        assert_eq!(parse(&data).err(), Some(CoreError::DecodeFailed));
    }

    #[test]
    fn test_multiple_exif_only_first_replaced() {
        let exif1 = make_chunk(b"eXIf", b"II*\0first");
        let exif2 = make_chunk(b"eXIf", b"II*\0second");
        let png_data = minimal_png(&[exif1, exif2], &[]);

        let parsed = parse(&png_data).expect("should parse");
        assert_eq!(parsed.exif(), Some(b"II*\0first".as_slice()));

        let kept = b"II*\0NEW_EXIF";
        let stripped = strip(&parsed, Some(kept));

        let exif_count = stripped.windows(4).filter(|w| *w == b"eXIf").count();
        assert_eq!(exif_count, 1);
        assert!(stripped.windows(kept.len()).any(|w| w == kept));
    }

    #[test]
    fn test_unknown_chunk_and_text_dropped() {
        let unknown = make_chunk(b"foOB", b"custom data");
        let text = make_chunk(b"tEXt", b"Author\0Example");
        let phys = make_chunk(b"pHYs", &[0, 0, 0x0E, 0xC4, 0, 0, 0x0E, 0xC4, 1]);
        let png_data = minimal_png(&[unknown, text, phys.clone()], &[]);

        let parsed = parse(&png_data).expect("should parse");
        let image_chunks = parsed.image_chunks();
        assert!(
            image_chunks
                .iter()
                .any(|c| c.windows(4).any(|w| w == b"pHYs"))
        );
        assert!(
            !image_chunks
                .iter()
                .any(|c| c.windows(4).any(|w| w == b"foOB"))
        );

        let stripped = strip(&parsed, None);
        assert!(!stripped.windows(4).any(|w| w == b"foOB"));
        assert!(!stripped.windows(4).any(|w| w == b"tEXt"));
        assert!(stripped.windows(4).any(|w| w == b"pHYs"));
    }

    #[test]
    fn test_trailer_after_iend_not_written() {
        let trailer = b"EXTRA_TRAILER_BYTES";
        let png_data = minimal_png(&[], trailer);
        let parsed = parse(&png_data).expect("should parse");
        assert_eq!(parsed.trailing(), trailer);

        let stripped = strip(&parsed, None);
        assert!(!stripped.windows(trailer.len()).any(|w| w == trailer));
    }

    #[test]
    fn test_no_original_exif_does_not_write_kept() {
        let png_data = minimal_png(&[], &[]);
        let parsed = parse(&png_data).expect("should parse");
        assert!(parsed.exif().is_none());

        let stripped = strip(&parsed, Some(b"II*\0NEW_EXIF"));
        assert!(!stripped.windows(4).any(|w| w == b"eXIf"));
    }

    #[test]
    fn test_invalid_signature_decode_failed() {
        assert_eq!(
            parse(b"NOT_A_PNG_HEADER").err(),
            Some(CoreError::DecodeFailed)
        );
    }
}
