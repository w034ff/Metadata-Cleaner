//! Integration tests for image formats using fixtures (design §11.1, work-plan T03).

use mcleaner_core::detect::{Format, detect};
use mcleaner_core::error::CoreError;
use mcleaner_core::{jpeg, png, webp};

/// Fixed little-endian TIFF bytes holding a single IFD0 entry with Orientation = 6 (26 bytes).
const KEPT_EXIF: [u8; 26] = [
    b'I', b'I', 0x2A, 0x00, 0x08, 0x00, 0x00,
    0x00, // TIFF header: little-endian, offset to IFD0 = 8
    0x01, 0x00, // 1 entry in IFD0
    0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00,
    0x00, // Orientation (0x0112) SHORT (3) count 1 value 6
    0x00, 0x00, 0x00, 0x00, // Next IFD = 0
];

const FICTIONAL_VALUES: &[&str] = &[
    "Example Author",
    "Example Editor",
    "Example Software",
    "Example Camera Make",
    "Example Camera Model",
    "EX-12345678",
    "2026:01:02 03:04:05",
    "Example Comment",
    "Example Title",
    "Example Subject",
    "Example Keywords",
    "Copyright (C) 2026 Example Author",
    "Example City",
    "Example State",
    "Example Country",
];

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path}: {e}"))
}

#[test]
fn test_detect_fixtures() {
    assert_eq!(detect(&fixture_bytes("full.jpg")), Ok(Format::Jpeg));
    assert_eq!(detect(&fixture_bytes("clean.jpg")), Ok(Format::Jpeg));
    assert_eq!(detect(&fixture_bytes("full.png")), Ok(Format::Png));
    assert_eq!(detect(&fixture_bytes("clean.png")), Ok(Format::Png));
    assert_eq!(detect(&fixture_bytes("full.webp")), Ok(Format::Webp));
    assert_eq!(detect(&fixture_bytes("clean.webp")), Ok(Format::Webp));
    assert_eq!(detect(&fixture_bytes("full.pdf")), Ok(Format::Pdf));

    // webp_named.jpg has .jpg extension but WebP content
    assert_eq!(detect(&fixture_bytes("webp_named.jpg")), Ok(Format::Webp));

    // Plain text is unsupported
    assert_eq!(
        detect(b"plain text bytes without valid signatures"),
        Err(CoreError::UnsupportedFormat)
    );
}

#[test]
fn test_corrupt_fixtures_fail_to_decode() {
    assert_eq!(
        jpeg::parse(&fixture_bytes("corrupt.jpg")).err(),
        Some(CoreError::DecodeFailed)
    );
    assert_eq!(
        png::parse(&fixture_bytes("corrupt.png")).err(),
        Some(CoreError::DecodeFailed)
    );
    assert_eq!(
        webp::parse(&fixture_bytes("corrupt.webp")).err(),
        Some(CoreError::DecodeFailed)
    );
}

#[test]
fn test_full_jpg_cleaning() {
    let input = fixture_bytes("full.jpg");
    let parsed = jpeg::parse(&input).expect("full.jpg must parse");
    assert!(parsed.exif().is_some());

    let stripped = jpeg::strip(&parsed, Some(&KEPT_EXIF));
    let reparsed = jpeg::parse(&stripped).expect("strip output must parse again");

    // image_segments byte-for-byte identical
    assert_eq!(reparsed.image_segments(), parsed.image_segments());

    // JFIF and ICC kept, JFIF thumbnail dimensions set to 0
    assert!(stripped.windows(5).any(|w| w == b"JFIF\0"));
    let jfif_pos = stripped.windows(5).position(|w| w == b"JFIF\0").unwrap();
    // In JFIF payload, index 12 is Xthumbnail and 13 is Ythumbnail
    assert_eq!(stripped[jfif_pos + 12], 0);
    assert_eq!(stripped[jfif_pos + 13], 0);

    assert!(stripped.windows(12).any(|w| w == b"ICC_PROFILE\0"));

    // EXIF replaced with kept_exif
    assert_eq!(reparsed.exif(), Some(KEPT_EXIF.as_slice()));

    // XMP, IPTC (Photoshop APP13 0xED), MPF, COM (0xFE), trailer stripped
    assert!(
        !stripped
            .windows(29)
            .any(|w| w == b"http://ns.adobe.com/xap/1.0/")
    );
    assert!(!stripped.windows(14).any(|w| w == b"Photoshop 3.0\0"));
    assert!(!stripped.windows(4).any(|w| w == b"MPF\0"));
    // File must end with EOI marker (0xFF, 0xD9) without trailing bytes
    assert!(stripped.ends_with(&[0xFF, 0xD9]));

    // Fictional metadata constants must not appear in cleaned output
    for &val in FICTIONAL_VALUES {
        assert!(
            !stripped.windows(val.len()).any(|w| w == val.as_bytes()),
            "cleaned JPEG contains fictional string: {val}"
        );
    }
}

#[test]
fn test_progressive_jpg_cleaning() {
    let input = fixture_bytes("progressive.jpg");
    let parsed = jpeg::parse(&input).expect("progressive.jpg must parse");

    let stripped = jpeg::strip(&parsed, Some(&KEPT_EXIF));
    let reparsed = jpeg::parse(&stripped).expect("strip output must parse again");

    // image_segments identical before and after strip
    assert_eq!(reparsed.image_segments(), parsed.image_segments());

    // Intermediate COM and XMP segments stripped
    assert!(
        !stripped
            .windows(29)
            .any(|w| w == b"http://ns.adobe.com/xap/1.0/")
    );
    for &val in FICTIONAL_VALUES {
        assert!(!stripped.windows(val.len()).any(|w| w == val.as_bytes()));
    }
}

#[test]
fn test_cmyk_jpg_cleaning() {
    let input = fixture_bytes("cmyk.jpg");
    let parsed = jpeg::parse(&input).expect("cmyk.jpg must parse");

    let stripped = jpeg::strip(&parsed, Some(&KEPT_EXIF));
    let reparsed = jpeg::parse(&stripped).expect("strip output must parse again");

    // image_segments identical
    assert_eq!(reparsed.image_segments(), parsed.image_segments());

    // Adobe APP14 segment kept intact
    assert!(stripped.windows(5).any(|w| w == b"Adobe"));
}

#[test]
fn test_full_png_cleaning() {
    let input = fixture_bytes("full.png");
    let parsed = png::parse(&input).expect("full.png must parse");
    assert!(parsed.exif().is_some());

    let stripped = png::strip(&parsed, Some(&KEPT_EXIF));
    let reparsed = png::parse(&stripped).expect("strip output must parse again");

    // Preserved chunks identical (eXIf excluded from image_chunks)
    assert_eq!(reparsed.image_chunks(), parsed.image_chunks());

    // iCCP and pHYs kept
    assert!(stripped.windows(4).any(|w| w == b"iCCP"));
    assert!(stripped.windows(4).any(|w| w == b"pHYs"));

    // eXIf replaced with kept_exif
    assert_eq!(reparsed.exif(), Some(KEPT_EXIF.as_slice()));

    // tEXt, zTXt, iTXt, tIME stripped
    assert!(!stripped.windows(4).any(|w| w == b"tEXt"));
    assert!(!stripped.windows(4).any(|w| w == b"zTXt"));
    assert!(!stripped.windows(4).any(|w| w == b"iTXt"));
    assert!(!stripped.windows(4).any(|w| w == b"tIME"));

    // Ends with IEND chunk (len: 0, IEND, crc)
    assert!(stripped.ends_with(b"IEND\xAE\x42\x60\x82"));

    // Fictional metadata constants stripped
    for &val in FICTIONAL_VALUES {
        assert!(
            !stripped.windows(val.len()).any(|w| w == val.as_bytes()),
            "cleaned PNG contains fictional string: {val}"
        );
    }
}

#[test]
fn test_anim_png_cleaning() {
    let input = fixture_bytes("anim.png");
    let parsed = png::parse(&input).expect("anim.png must parse");

    let stripped = png::strip(&parsed, None);
    let reparsed = png::parse(&stripped).expect("strip output must parse again");

    assert_eq!(reparsed.image_chunks(), parsed.image_chunks());

    // APNG animation chunks kept
    assert!(stripped.windows(4).any(|w| w == b"acTL"));
    assert!(stripped.windows(4).any(|w| w == b"fcTL"));
    assert!(stripped.windows(4).any(|w| w == b"fdAT"));

    // tEXt stripped
    assert!(!stripped.windows(4).any(|w| w == b"tEXt"));
}

#[test]
fn test_full_webp_cleaning() {
    let input = fixture_bytes("full.webp");
    let parsed = webp::parse(&input).expect("full.webp must parse");
    assert!(parsed.exif().is_some());

    // Case 1: with kept_exif
    let stripped = webp::strip(&parsed, Some(&KEPT_EXIF));
    let reparsed = webp::parse(&stripped).expect("strip output must parse again");

    assert_eq!(reparsed.image_chunks(), parsed.image_chunks());

    // XMP chunk stripped
    assert!(!stripped.windows(4).any(|w| w == b"XMP "));

    // EXIF is the last chunk
    assert_eq!(reparsed.exif(), Some(KEPT_EXIF.as_slice()));
    // RIFF body: ends with EXIF chunk
    let exif_pos = stripped.windows(4).rposition(|w| w == b"EXIF").unwrap();
    assert!(exif_pos > stripped.windows(4).position(|w| w == b"VP8L").unwrap());

    // VP8X flag: XMP (0x04) cleared, EXIF (0x08) set
    let vp8x_pos = stripped.windows(4).position(|w| w == b"VP8X").unwrap();
    let flags = stripped[vp8x_pos + 8]; // chunk id (4) + len (4) -> flags at offset 8
    assert_eq!(flags & 0x04, 0);
    assert_ne!(flags & 0x08, 0);

    // Case 2: without kept_exif (None)
    let stripped_no_exif = webp::strip(&parsed, None);
    let reparsed_no_exif = webp::parse(&stripped_no_exif).expect("strip output must parse again");

    assert_eq!(reparsed_no_exif.image_chunks(), parsed.image_chunks());
    assert_eq!(reparsed_no_exif.exif(), None);
    assert!(!stripped_no_exif.windows(4).any(|w| w == b"EXIF"));

    let flags_no_exif = stripped_no_exif[vp8x_pos + 8];
    assert_eq!(flags_no_exif & 0x04, 0);
    assert_eq!(flags_no_exif & 0x08, 0);

    // Fictional metadata constants stripped
    for &val in FICTIONAL_VALUES {
        assert!(
            !stripped.windows(val.len()).any(|w| w == val.as_bytes()),
            "cleaned WebP contains fictional string: {val}"
        );
    }
}

#[test]
fn test_anim_webp_cleaning() {
    let input = fixture_bytes("anim.webp");
    let parsed = webp::parse(&input).expect("anim.webp must parse");

    let stripped = webp::strip(&parsed, None);
    let reparsed = webp::parse(&stripped).expect("strip output must parse again");

    assert_eq!(reparsed.image_chunks(), parsed.image_chunks());

    // Animation chunks ANIM and ANMF kept
    assert!(stripped.windows(4).any(|w| w == b"ANIM"));
    assert!(stripped.windows(4).any(|w| w == b"ANMF"));

    // XMP chunk stripped
    assert!(!stripped.windows(4).any(|w| w == b"XMP "));
}
