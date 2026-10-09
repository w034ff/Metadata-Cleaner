//! Format detection and file size checks (design §4.1).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::CoreError;

/// Supported file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Jpeg,
    Png,
    Webp,
    Pdf,
}

/// Allowed extensions for user input selection.
pub const SUPPORTED_EXTENSIONS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "pdf"];

/// Maximum allowed file size for image formats (256 MiB).
pub const MAX_IMAGE_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// Maximum allowed file size for PDF files (512 MiB).
pub const MAX_PDF_FILE_BYTES: u64 = 512 * 1024 * 1024;

/// Number of initial file bytes inspected for format detection.
pub const DETECT_HEAD_BYTES: usize = 1024;

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PDF_SIGNATURE: &[u8; 5] = b"%PDF-";

/// Detects the file format from the initial bytes of a file.
///
/// Only inspects up to [`DETECT_HEAD_BYTES`] bytes. Returns [`CoreError::UnsupportedFormat`]
/// if no known file signature is matched.
pub fn detect(head: &[u8]) -> Result<Format, CoreError> {
    let probe = if head.len() > DETECT_HEAD_BYTES {
        &head[..DETECT_HEAD_BYTES]
    } else {
        head
    };

    if probe.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok(Format::Jpeg);
    }
    if probe.starts_with(PNG_SIGNATURE) {
        return Ok(Format::Png);
    }
    if probe.len() >= 12 && &probe[0..4] == b"RIFF" && &probe[8..12] == b"WEBP" {
        return Ok(Format::Webp);
    }
    if probe
        .windows(PDF_SIGNATURE.len())
        .any(|w| w == PDF_SIGNATURE)
    {
        return Ok(Format::Pdf);
    }

    Err(CoreError::UnsupportedFormat)
}

/// Checks that the file size does not exceed the limit for its format.
pub fn check_size(format: Format, file_bytes: u64) -> Result<(), CoreError> {
    let limit = match format {
        Format::Jpeg | Format::Png | Format::Webp => MAX_IMAGE_FILE_BYTES,
        Format::Pdf => MAX_PDF_FILE_BYTES,
    };
    if file_bytes > limit {
        Err(CoreError::TooLarge { limit_bytes: limit })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_jpeg() {
        assert_eq!(detect(&[0xFF, 0xD8, 0xFF, 0xE0]), Ok(Format::Jpeg));
        assert_eq!(detect(&[0xFF, 0xD8, 0xFF, 0xDB]), Ok(Format::Jpeg));
    }

    #[test]
    fn test_detect_png() {
        assert_eq!(
            detect(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR"),
            Ok(Format::Png)
        );
    }

    #[test]
    fn test_detect_webp() {
        let mut data = b"RIFF....WEBPVP8 ".to_vec();
        data[4..8].copy_from_slice(&8u32.to_le_bytes());
        assert_eq!(detect(&data), Ok(Format::Webp));
    }

    #[test]
    fn test_detect_pdf_within_1024_bytes() {
        let mut data = vec![0x20; 500];
        data.extend_from_slice(b"%PDF-1.7\n");
        assert_eq!(detect(&data), Ok(Format::Pdf));
    }

    #[test]
    fn test_detect_pdf_beyond_1024_bytes_ignored() {
        let mut data = vec![0x20; 1024];
        data.extend_from_slice(b"%PDF-1.7\n");
        assert_eq!(detect(&data), Err(CoreError::UnsupportedFormat));
    }

    #[test]
    fn test_detect_empty_and_unsupported() {
        assert_eq!(detect(&[]), Err(CoreError::UnsupportedFormat));
        assert_eq!(
            detect(b"plain text file contents"),
            Err(CoreError::UnsupportedFormat)
        );
    }

    #[test]
    fn test_check_size_exact_and_over_image() {
        assert_eq!(check_size(Format::Jpeg, MAX_IMAGE_FILE_BYTES), Ok(()));
        assert_eq!(check_size(Format::Png, MAX_IMAGE_FILE_BYTES), Ok(()));
        assert_eq!(check_size(Format::Webp, MAX_IMAGE_FILE_BYTES), Ok(()));

        let err = check_size(Format::Jpeg, MAX_IMAGE_FILE_BYTES + 1);
        assert_eq!(
            err,
            Err(CoreError::TooLarge {
                limit_bytes: MAX_IMAGE_FILE_BYTES
            })
        );
        assert_eq!(err.unwrap_err().detail(), Some("256 MB".to_string()));
        assert_eq!(err.unwrap_err().code(), "TooLarge");
    }

    #[test]
    fn test_check_size_exact_and_over_pdf() {
        assert_eq!(check_size(Format::Pdf, MAX_PDF_FILE_BYTES), Ok(()));

        let err = check_size(Format::Pdf, MAX_PDF_FILE_BYTES + 1);
        assert_eq!(
            err,
            Err(CoreError::TooLarge {
                limit_bytes: MAX_PDF_FILE_BYTES
            })
        );
        assert_eq!(err.unwrap_err().detail(), Some("512 MB".to_string()));
        assert_eq!(err.unwrap_err().code(), "TooLarge");
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(CoreError::UnsupportedFormat.code(), "UnsupportedFormat");
        assert_eq!(CoreError::DecodeFailed.code(), "DecodeFailed");
        assert_eq!(CoreError::UnsupportedFormat.detail(), None);
        assert_eq!(CoreError::DecodeFailed.detail(), None);
    }
}
