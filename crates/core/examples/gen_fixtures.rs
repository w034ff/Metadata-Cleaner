//! Generates the test fixtures of design §11.1 into `crates/core/tests/fixtures/`.
//!
//! Run with `cargo run -p mcleaner-core --example gen_fixtures` and commit the
//! result. The output depends only on this file and the pinned encoders; every
//! value comes from deterministic integer arithmetic, so it is identical on
//! every platform. `tests/fixtures.rs` regenerates it in memory and fails when it
//! differs from the committed files.

use std::error::Error;
use std::io::Cursor;
use std::path::Path;

use image::ExtendedColorType;
use image::ImageEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::webp::WebPEncoder;
use md5::{Digest, Md5};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Relative to the crate root.
pub const FIXTURES_DIR: &str = "tests/fixtures";

// Fictional metadata constants (design §11.1, NFR-06).
// These are used across downstream tests to verify removal from raw bytes.
pub const AUTHOR: &str = "Example Author";
pub const EDITOR: &str = "Example Editor";
pub const SOFTWARE: &str = "Example Software";
pub const CAMERA_MAKE: &str = "Example Camera Make";
pub const CAMERA_MODEL: &str = "Example Camera Model";
pub const SERIAL_NUMBER: &str = "EX-12345678";
pub const DATE_TIME: &str = "2026:01:02 03:04:05";
pub const DATE_TIME_ISO: &str = "2026-01-02T03:04:05";
pub const DATE_TIME_PDF: &str = "D:20260102030405Z";
pub const COMMENT: &str = "Example Comment";
pub const TITLE: &str = "Example Title";
pub const SUBJECT: &str = "Example Subject";
pub const KEYWORDS: &str = "Example Keywords";
pub const COPYRIGHT: &str = "Copyright (C) 2026 Example Author";
pub const CITY: &str = "Example City";
pub const STATE: &str = "Example State";
pub const COUNTRY: &str = "Example Country";

/// The description of the ICC profile. The profile is kept when cleaning, so
/// its text uses none of the fictional values above, which tests look for in
/// cleaned output.
pub const ICC_DESCRIPTION: &str = "Fixture sRGB";
const ICC_COPYRIGHT: &str = "No copyright, use freely";

pub const LATITUDE_DEG: u32 = 12;
pub const LATITUDE_MIN: u32 = 34;
pub const LATITUDE_SEC: u32 = 0;
pub const LONGITUDE_DEG: u32 = 65;
pub const LONGITUDE_MIN: u32 = 43;
pub const LONGITUDE_SEC: u32 = 0;

pub const ENCRYPTED_USER_PASSWORD: &str = "fixture-user";
const OWNER_PASSWORD: &str = "fixture-owner";
const PERMISSIONS_ALL: i32 = -4;
const PERMISSIONS_RESTRICTED: i32 = -3904;
const PASSWORD_PADDING: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08,
    0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53, 0x69, 0x7A,
];
const KEY_HASH_ROUNDS: usize = 50;
const RC4_ROUNDS: u8 = 19;
const KEY_LENGTH_BYTES: usize = 16;

const IMAGE_WIDTH: u32 = 64;
const IMAGE_HEIGHT: u32 = 48;
const JPEG_QUALITY: u8 = 90;

/// One generated file: its lowercase name in [`FIXTURES_DIR`] and its bytes.
pub struct Fixture {
    pub name: String,
    pub bytes: Vec<u8>,
}

fn main() -> Result<()> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES_DIR);
    std::fs::create_dir_all(&dir)?;
    for fixture in fixtures()? {
        std::fs::write(dir.join(&fixture.name), &fixture.bytes)?;
    }
    Ok(())
}

/// Every fixture of design §11.1, in memory.
pub fn fixtures() -> Result<Vec<Fixture>> {
    let mut out = Vec::new();
    let mut add = |name: &str, bytes: Vec<u8>| {
        out.push(Fixture {
            name: name.to_owned(),
            bytes,
        });
    };

    let rgb_pixels = pattern_rgb(IMAGE_WIDTH, IMAGE_HEIGHT);
    let rgba_pixels = pattern_rgba(IMAGE_WIDTH, IMAGE_HEIGHT);
    let cmyk_pixels = pattern_cmyk(IMAGE_WIDTH, IMAGE_HEIGHT);

    // 1. full.jpg
    let full_jpg = build_full_jpeg(&rgb_pixels, IMAGE_WIDTH, IMAGE_HEIGHT)?;
    add("corrupt.jpg", full_jpg[..full_jpg.len() / 2].to_vec());
    add("full.jpg", full_jpg.clone());

    // 2. orient1.jpg..orient8.jpg
    for orientation in 1..=8 {
        let name = format!("orient{orientation}.jpg");
        add(
            &name,
            build_orient_jpeg(&rgb_pixels, IMAGE_WIDTH, IMAGE_HEIGHT, orientation)?,
        );
    }

    // 3. cmyk.jpg
    add(
        "cmyk.jpg",
        build_cmyk_jpeg(&cmyk_pixels, IMAGE_WIDTH, IMAGE_HEIGHT)?,
    );

    // 4. progressive.jpg
    add(
        "progressive.jpg",
        build_progressive_jpeg(&rgb_pixels, IMAGE_WIDTH, IMAGE_HEIGHT)?,
    );

    // 5. no_jfif_dpi.jpg
    add(
        "no_jfif_dpi.jpg",
        build_no_jfif_dpi_jpeg(&rgb_pixels, IMAGE_WIDTH, IMAGE_HEIGHT)?,
    );

    // 6. clean.jpg
    add(
        "clean.jpg",
        encode_plain_jpeg(&rgb_pixels, IMAGE_WIDTH, IMAGE_HEIGHT)?,
    );

    // 7. full.png
    let full_png = build_full_png(&rgba_pixels, IMAGE_WIDTH, IMAGE_HEIGHT)?;
    add("corrupt.png", full_png[..full_png.len() / 2].to_vec());
    add("full.png", full_png);

    // 8. anim.png
    add(
        "anim.png",
        build_anim_png(&rgba_pixels, IMAGE_WIDTH, IMAGE_HEIGHT)?,
    );

    // 9. clean.png
    add(
        "clean.png",
        encode_plain_png(&rgba_pixels, IMAGE_WIDTH, IMAGE_HEIGHT)?,
    );

    // 10. full.webp
    let full_webp = build_full_webp(&rgba_pixels, IMAGE_WIDTH, IMAGE_HEIGHT)?;
    add("corrupt.webp", full_webp[..full_webp.len() / 2].to_vec());
    add("webp_named.jpg", full_webp.clone());
    add("full.webp", full_webp);

    // 11. anim.webp
    add(
        "anim.webp",
        build_anim_webp(&rgba_pixels, IMAGE_WIDTH, IMAGE_HEIGHT)?,
    );

    // 12. clean.webp
    add(
        "clean.webp",
        encode_plain_webp(&rgba_pixels, IMAGE_WIDTH, IMAGE_HEIGHT)?,
    );

    // 13. full.pdf
    let full_pdf = build_full_pdf(&full_jpg, IMAGE_WIDTH, IMAGE_HEIGHT)?;
    add("corrupt.pdf", full_pdf[..full_pdf.len() / 2].to_vec());
    add("full.pdf", full_pdf);

    // 14. linearized.pdf
    add("linearized.pdf", build_linearized_pdf()?);

    // 15. encrypted.pdf / restricted.pdf
    add(
        "encrypted.pdf",
        encrypted_pdf("encrypted", ENCRYPTED_USER_PASSWORD, PERMISSIONS_ALL),
    );
    add(
        "restricted.pdf",
        encrypted_pdf("restricted", "", PERMISSIONS_RESTRICTED),
    );

    // 16. signed.pdf
    add("signed.pdf", build_signed_pdf()?);

    Ok(out)
}

// ---------------------------------------------------------------------------
// Pixel Patterns

fn pattern_rgb(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let r = ((x * 255) / (w - 1).max(1)) as u8;
            let g = ((y * 255) / (h - 1).max(1)) as u8;
            let b = (((x + y) * 128) / (w + h).max(1)) as u8;
            out.extend_from_slice(&[r, g, b]);
        }
    }
    out
}

fn pattern_rgba(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let r = ((x * 255) / (w - 1).max(1)) as u8;
            let g = ((y * 255) / (h - 1).max(1)) as u8;
            let b = (((x + y) * 128) / (w + h).max(1)) as u8;
            let a = if (x / 8 + y / 8) % 2 == 0 { 255 } else { 180 };
            out.extend_from_slice(&[r, g, b, a]);
        }
    }
    out
}

fn pattern_cmyk(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let c = ((x * 255) / (w - 1).max(1)) as u8;
            let m = ((y * 255) / (h - 1).max(1)) as u8;
            let y_col = 128u8;
            let k = if (x / 16 + y / 16) % 2 == 0 { 0 } else { 40 };
            out.extend_from_slice(&[c, m, y_col, k]);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Standard Encoders

fn encode_plain_jpeg(data: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut out, JPEG_QUALITY);
    encoder.encode(
        data,
        u16::try_from(w)?,
        u16::try_from(h)?,
        jpeg_encoder::ColorType::Rgb,
    )?;
    Ok(out)
}

fn encode_plain_png(data: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    PngEncoder::new(&mut out).write_image(data, w, h, ExtendedColorType::Rgba8)?;
    Ok(out)
}

fn encode_plain_webp(data: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    WebPEncoder::new_lossless(&mut out).encode(data, w, h, ExtendedColorType::Rgba8)?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// XMP, ICC, IPTC and EXIF Metadata Generators

/// Valid RDF XMP packet containing all metadata names of design §4.5 table.
pub fn generate_xmp_packet() -> String {
    format!(
        concat!(
            "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n",
            "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"Adobe XMP Core 5.6-c140\">\n",
            " <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n",
            "  <rdf:Description rdf:about=\"\"\n",
            "    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n",
            "    xmlns:photoshop=\"http://ns.adobe.com/photoshop/1.0/\"\n",
            "    xmlns:tiff=\"http://ns.adobe.com/tiff/1.0/\"\n",
            "    xmlns:exif=\"http://ns.adobe.com/exif/1.0/\"\n",
            "    xmlns:aux=\"http://ns.adobe.com/exif/1.0/aux/\"\n",
            "    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n",
            "   xmp:CreateDate=\"{create_date}\"\n",
            "   xmp:ModifyDate=\"{create_date}\"\n",
            "   xmp:MetadataDate=\"{create_date}\"\n",
            "   xmp:CreatorTool=\"{software}\"\n",
            "   photoshop:DateCreated=\"{create_date}\"\n",
            "   tiff:Make=\"{make}\"\n",
            "   tiff:Model=\"{model}\"\n",
            "   aux:SerialNumber=\"{serial}\"\n",
            "   exif:GPSLatitude=\"{lat_deg},{lat_min}.000000N\"\n",
            "   exif:GPSLongitude=\"{lon_deg},{lon_min}.000000E\">\n",
            "   <dc:creator>\n",
            "    <rdf:Seq>\n",
            "     <rdf:li>{author}</rdf:li>\n",
            "    </rdf:Seq>\n",
            "   </dc:creator>\n",
            "   <dc:rights>\n",
            "    <rdf:Alt>\n",
            "     <rdf:li xml:lang=\"x-default\">{copyright}</rdf:li>\n",
            "    </rdf:Alt>\n",
            "   </dc:rights>\n",
            "   <dc:title>\n",
            "    <rdf:Alt>\n",
            "     <rdf:li xml:lang=\"x-default\">{title}</rdf:li>\n",
            "    </rdf:Alt>\n",
            "   </dc:title>\n",
            "   <dc:description>\n",
            "    <rdf:Alt>\n",
            "     <rdf:li xml:lang=\"x-default\">{comment}</rdf:li>\n",
            "    </rdf:Alt>\n",
            "   </dc:description>\n",
            "  </rdf:Description>\n",
            " </rdf:RDF>\n",
            "</x:xmpmeta>\n",
            "<?xpacket end=\"w\"?>\n"
        ),
        create_date = DATE_TIME_ISO,
        software = SOFTWARE,
        make = CAMERA_MAKE,
        model = CAMERA_MODEL,
        serial = SERIAL_NUMBER,
        lat_deg = LATITUDE_DEG,
        lat_min = LATITUDE_MIN,
        lon_deg = LONGITUDE_DEG,
        lon_min = LONGITUDE_MIN,
        author = AUTHOR,
        copyright = COPYRIGHT,
        title = TITLE,
        comment = COMMENT,
    )
}

/// Minimal valid ICC v2 profile with D50, sRGB primaries and gamma.
pub fn generate_icc_profile() -> Vec<u8> {
    const HEADER_SIZE: usize = 128;
    const TAG_COUNT: usize = 9;
    const TAG_TABLE_SIZE: usize = 4 + TAG_COUNT * 12;

    // Build tag data payloads.
    // 1. desc: textDescriptionType
    let desc_bytes = ICC_DESCRIPTION.as_bytes();
    let desc_ascii_count = (desc_bytes.len() + 1) as u32;
    let mut desc_payload = Vec::new();
    desc_payload.extend_from_slice(b"desc");
    desc_payload.extend_from_slice(&0u32.to_be_bytes()); // reserved
    desc_payload.extend_from_slice(&desc_ascii_count.to_be_bytes());
    desc_payload.extend_from_slice(desc_bytes);
    desc_payload.push(0); // null terminator
    desc_payload.extend_from_slice(&0u32.to_be_bytes()); // unicode language
    desc_payload.extend_from_slice(&0u32.to_be_bytes()); // unicode count
    desc_payload.extend_from_slice(&0u16.to_be_bytes()); // scriptcode code
    desc_payload.push(0); // scriptcode count
    desc_payload.extend_from_slice(&[0u8; 67]); // scriptcode string
    pad_to_4(&mut desc_payload);

    // 2. cprt: textType
    let cprt_bytes = ICC_COPYRIGHT.as_bytes();
    let mut cprt_payload = Vec::new();
    cprt_payload.extend_from_slice(b"text");
    cprt_payload.extend_from_slice(&0u32.to_be_bytes());
    cprt_payload.extend_from_slice(cprt_bytes);
    cprt_payload.push(0);
    pad_to_4(&mut cprt_payload);

    // 3. wtpt: XYZType (D50)
    let mut wtpt_payload = Vec::new();
    wtpt_payload.extend_from_slice(b"XYZ ");
    wtpt_payload.extend_from_slice(&0u32.to_be_bytes());
    wtpt_payload.extend_from_slice(&0x0000F6D6u32.to_be_bytes()); // X = 0.9642
    wtpt_payload.extend_from_slice(&0x00010000u32.to_be_bytes()); // Y = 1.0000
    wtpt_payload.extend_from_slice(&0x0000D32Du32.to_be_bytes()); // Z = 0.8249

    // 4. rXYZ, gXYZ, bXYZ: XYZType
    let mut rxyz_payload = Vec::new();
    rxyz_payload.extend_from_slice(b"XYZ ");
    rxyz_payload.extend_from_slice(&0u32.to_be_bytes());
    rxyz_payload.extend_from_slice(&0x00006FA2u32.to_be_bytes());
    rxyz_payload.extend_from_slice(&0x000038F5u32.to_be_bytes());
    rxyz_payload.extend_from_slice(&0x00000390u32.to_be_bytes());

    let mut gxyz_payload = Vec::new();
    gxyz_payload.extend_from_slice(b"XYZ ");
    gxyz_payload.extend_from_slice(&0u32.to_be_bytes());
    gxyz_payload.extend_from_slice(&0x00006299u32.to_be_bytes());
    gxyz_payload.extend_from_slice(&0x0000B785u32.to_be_bytes());
    gxyz_payload.extend_from_slice(&0x000018DAu32.to_be_bytes());

    let mut bxyz_payload = Vec::new();
    bxyz_payload.extend_from_slice(b"XYZ ");
    bxyz_payload.extend_from_slice(&0u32.to_be_bytes());
    bxyz_payload.extend_from_slice(&0x000024A0u32.to_be_bytes());
    bxyz_payload.extend_from_slice(&0x00000F83u32.to_be_bytes());
    bxyz_payload.extend_from_slice(&0x0000B6D0u32.to_be_bytes());

    // 5. rTRC, gTRC, bTRC: curvType (gamma 2.2 = 0x0233)
    let mut trc_payload = Vec::new();
    trc_payload.extend_from_slice(b"curv");
    trc_payload.extend_from_slice(&0u32.to_be_bytes());
    trc_payload.extend_from_slice(&1u32.to_be_bytes()); // 1 entry
    trc_payload.extend_from_slice(&0x0233u16.to_be_bytes()); // gamma 2.2
    trc_payload.extend_from_slice(&[0u8; 2]); // pad to 4 bytes

    let tags: [(&[u8; 4], &[u8]); TAG_COUNT] = [
        (b"desc", &desc_payload),
        (b"cprt", &cprt_payload),
        (b"wtpt", &wtpt_payload),
        (b"rXYZ", &rxyz_payload),
        (b"gXYZ", &gxyz_payload),
        (b"bXYZ", &bxyz_payload),
        (b"rTRC", &trc_payload),
        (b"gTRC", &trc_payload),
        (b"bTRC", &trc_payload),
    ];

    let mut data_offset = (HEADER_SIZE + TAG_TABLE_SIZE) as u32;
    let mut tag_table = Vec::with_capacity(TAG_TABLE_SIZE);
    tag_table.extend_from_slice(&(TAG_COUNT as u32).to_be_bytes());

    let mut tags_data = Vec::new();
    for (sig, payload) in tags {
        tag_table.extend_from_slice(sig);
        tag_table.extend_from_slice(&data_offset.to_be_bytes());
        tag_table.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        tags_data.extend_from_slice(payload);
        data_offset += payload.len() as u32;
    }

    let total_size = HEADER_SIZE + TAG_TABLE_SIZE + tags_data.len();

    let mut header = vec![0u8; HEADER_SIZE];
    header[0..4].copy_from_slice(&(total_size as u32).to_be_bytes());
    header[4..8].copy_from_slice(b"none");
    header[8..12].copy_from_slice(&0x02100000u32.to_be_bytes()); // v2.1.0
    header[12..16].copy_from_slice(b"mntr");
    header[16..20].copy_from_slice(b"RGB ");
    header[20..24].copy_from_slice(b"XYZ ");
    // The creation date (bytes 24..36) stays zero: the profile is kept when
    // cleaning, so it must not carry the fixtures' DATE_TIME.
    header[36..40].copy_from_slice(b"acsp");
    header[40..44].copy_from_slice(b"APPL");
    // Illuminant D50
    header[68..72].copy_from_slice(&0x0000F6D6u32.to_be_bytes());
    header[72..76].copy_from_slice(&0x00010000u32.to_be_bytes());
    header[76..80].copy_from_slice(&0x0000D32Du32.to_be_bytes());

    let mut profile = Vec::with_capacity(total_size);
    profile.extend_from_slice(&header);
    profile.extend_from_slice(&tag_table);
    profile.extend_from_slice(&tags_data);
    profile
}

fn pad_to_4(buf: &mut Vec<u8>) {
    while !buf.len().is_multiple_of(4) {
        buf.push(0);
    }
}

/// IPTC-NAA data wrapped in Photoshop 3.0 8BIM format.
pub fn generate_iptc_app13() -> Vec<u8> {
    let mut iptc = Vec::new();
    let mut add_dataset = |rec: u8, num: u8, data: &[u8]| {
        iptc.push(0x1C);
        iptc.push(rec);
        iptc.push(num);
        iptc.extend_from_slice(&(data.len() as u16).to_be_bytes());
        iptc.extend_from_slice(data);
    };

    // 1:90 Character set (UTF-8 declaration: ESC % G)
    add_dataset(1, 90, &[0x1B, 0x25, 0x47]);
    // 2:00 Record version
    add_dataset(2, 0, &[0x00, 0x04]);
    // 2:05 Object name (Title)
    add_dataset(2, 5, TITLE.as_bytes());
    // 2:55 Date Created (CCYYMMDD)
    add_dataset(2, 55, b"20260102");
    // 2:60 Time Created (HHMMSS+0000)
    add_dataset(2, 60, b"030405+0000");
    // 2:80 By-line (Author)
    add_dataset(2, 80, AUTHOR.as_bytes());
    // 2:90 City
    add_dataset(2, 90, CITY.as_bytes());
    // 2:95 Province/State
    add_dataset(2, 95, STATE.as_bytes());
    // 2:101 Country
    add_dataset(2, 101, COUNTRY.as_bytes());
    // 2:116 Copyright Notice
    add_dataset(2, 116, COPYRIGHT.as_bytes());
    // 2:120 Caption/Abstract (Comment)
    add_dataset(2, 120, COMMENT.as_bytes());

    // 8BIM block for resource 0x0404 (IPTC-NAA)
    let mut bim = Vec::new();
    bim.extend_from_slice(b"8BIM");
    bim.extend_from_slice(&0x0404u16.to_be_bytes());
    bim.extend_from_slice(&[0, 0]); // Empty Pascal string (padded to even)
    bim.extend_from_slice(&(iptc.len() as u32).to_be_bytes());
    bim.extend_from_slice(&iptc);
    if iptc.len() % 2 != 0 {
        bim.push(0);
    }

    // Wrap into JPEG APP13
    let mut app13 = Vec::new();
    app13.extend_from_slice(&[0xFF, 0xED]);
    let len = 2 + 14 + bim.len();
    app13.extend_from_slice(&(len as u16).to_be_bytes());
    app13.extend_from_slice(b"Photoshop 3.0\0");
    app13.extend_from_slice(&bim);
    app13
}

/// Raw TIFF EXIF byte structure encoded via kamadak-exif Writer.
pub fn generate_exif_tiff(orientation: Option<u16>, gps: bool, full: bool) -> Result<Vec<u8>> {
    let mut writer = exif::experimental::Writer::new();
    let mut fields = Vec::new();

    if let Some(orient) = orientation {
        fields.push(exif::Field {
            tag: exif::Tag::Orientation,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Short(vec![orient]),
        });
    }

    if full {
        fields.push(exif::Field {
            tag: exif::Tag::Make,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![CAMERA_MAKE.as_bytes().to_vec()]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::Model,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![CAMERA_MODEL.as_bytes().to_vec()]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::Software,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![SOFTWARE.as_bytes().to_vec()]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::Artist,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![AUTHOR.as_bytes().to_vec()]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::Copyright,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![COPYRIGHT.as_bytes().to_vec()]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::DateTime,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![DATE_TIME.as_bytes().to_vec()]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::ImageDescription,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![TITLE.as_bytes().to_vec()]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::XResolution,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Rational(vec![exif::Rational { num: 300, denom: 1 }]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::YResolution,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Rational(vec![exif::Rational { num: 300, denom: 1 }]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::ResolutionUnit,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Short(vec![2]),
        });

        // Exif IFD
        fields.push(exif::Field {
            tag: exif::Tag::DateTimeOriginal,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![DATE_TIME.as_bytes().to_vec()]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::BodySerialNumber,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![SERIAL_NUMBER.as_bytes().to_vec()]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::MakerNote,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Undefined(b"Example MakerNote".to_vec(), 0),
        });
        let mut user_comment = b"ASCII\0\0\0".to_vec();
        user_comment.extend_from_slice(COMMENT.as_bytes());
        fields.push(exif::Field {
            tag: exif::Tag::UserComment,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Undefined(user_comment, 0),
        });

        // IFD1 fields
        fields.push(exif::Field {
            tag: exif::Tag::Compression,
            ifd_num: exif::In::THUMBNAIL,
            value: exif::Value::Short(vec![6]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::XResolution,
            ifd_num: exif::In::THUMBNAIL,
            value: exif::Value::Rational(vec![exif::Rational { num: 72, denom: 1 }]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::YResolution,
            ifd_num: exif::In::THUMBNAIL,
            value: exif::Value::Rational(vec![exif::Rational { num: 72, denom: 1 }]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::ResolutionUnit,
            ifd_num: exif::In::THUMBNAIL,
            value: exif::Value::Short(vec![2]),
        });
    }

    if gps {
        fields.push(exif::Field {
            tag: exif::Tag::GPSLatitudeRef,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![b"N".to_vec()]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::GPSLatitude,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Rational(vec![
                exif::Rational {
                    num: LATITUDE_DEG,
                    denom: 1,
                },
                exif::Rational {
                    num: LATITUDE_MIN,
                    denom: 1,
                },
                exif::Rational {
                    num: LATITUDE_SEC,
                    denom: 1,
                },
            ]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::GPSLongitudeRef,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![b"E".to_vec()]),
        });
        fields.push(exif::Field {
            tag: exif::Tag::GPSLongitude,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Rational(vec![
                exif::Rational {
                    num: LONGITUDE_DEG,
                    denom: 1,
                },
                exif::Rational {
                    num: LONGITUDE_MIN,
                    denom: 1,
                },
                exif::Rational {
                    num: LONGITUDE_SEC,
                    denom: 1,
                },
            ]),
        });
    }

    for f in &fields {
        writer.push_field(f);
    }

    let thumb_jpeg = if full {
        Some(encode_plain_jpeg(&pattern_rgb(16, 12), 16, 12)?)
    } else {
        None
    };

    if let Some(ref thumb) = thumb_jpeg {
        writer.set_jpeg(thumb, exif::In::THUMBNAIL);
    }

    let mut cur = Cursor::new(Vec::new());
    writer.write(&mut cur, true)?;
    Ok(cur.into_inner())
}

// ---------------------------------------------------------------------------
// JPEG Assembly

fn build_full_jpeg(pixels: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let base_jpeg = encode_plain_jpeg(pixels, w, h)?;
    let segments = split_jpeg_segments(&base_jpeg)?;

    // 1. JFIF APP0 (300 dpi, 2x2 RGB thumbnail)
    let mut jfif_app0 = Vec::new();
    jfif_app0.extend_from_slice(&[0xFF, 0xE0]);
    let jfif_len = 2 + 5 + 2 + 1 + 2 + 2 + 1 + 1 + 12; // 28 bytes
    jfif_app0.extend_from_slice(&(jfif_len as u16).to_be_bytes());
    jfif_app0.extend_from_slice(b"JFIF\0");
    jfif_app0.extend_from_slice(&[0x01, 0x02]); // Version 1.2
    jfif_app0.push(1); // dots per inch
    jfif_app0.extend_from_slice(&300u16.to_be_bytes());
    jfif_app0.extend_from_slice(&300u16.to_be_bytes());
    jfif_app0.push(2); // thumbnail width
    jfif_app0.push(2); // thumbnail height
    jfif_app0.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]); // 4 RGB pixels

    // 2. EXIF APP1 (orientation 6, GPS, all tags)
    let exif_tiff = generate_exif_tiff(Some(6), true, true)?;
    let mut exif_app1 = Vec::new();
    exif_app1.extend_from_slice(&[0xFF, 0xE1]);
    let exif_len = 2 + 6 + exif_tiff.len();
    exif_app1.extend_from_slice(&(exif_len as u16).to_be_bytes());
    exif_app1.extend_from_slice(b"Exif\0\0");
    exif_app1.extend_from_slice(&exif_tiff);

    // 3. XMP APP1
    let xmp_str = generate_xmp_packet();
    let mut xmp_app1 = Vec::new();
    xmp_app1.extend_from_slice(&[0xFF, 0xE1]);
    let xmp_len = 2 + 29 + xmp_str.len();
    xmp_app1.extend_from_slice(&(xmp_len as u16).to_be_bytes());
    xmp_app1.extend_from_slice(b"http://ns.adobe.com/xap/1.0/\0");
    xmp_app1.extend_from_slice(xmp_str.as_bytes());

    // 4. ICC APP2
    let icc_prof = generate_icc_profile();
    let mut icc_app2 = Vec::new();
    icc_app2.extend_from_slice(&[0xFF, 0xE2]);
    let icc_len = 2 + 14 + icc_prof.len();
    icc_app2.extend_from_slice(&(icc_len as u16).to_be_bytes());
    icc_app2.extend_from_slice(b"ICC_PROFILE\0");
    icc_app2.extend_from_slice(&[1, 1]); // chunk 1 of 1
    icc_app2.extend_from_slice(&icc_prof);

    // 5. IPTC APP13
    let iptc_app13 = generate_iptc_app13();

    // 6. COM segment
    let mut com_seg = Vec::new();
    com_seg.extend_from_slice(&[0xFF, 0xFE]);
    let com_len = 2 + COMMENT.len();
    com_seg.extend_from_slice(&(com_len as u16).to_be_bytes());
    com_seg.extend_from_slice(COMMENT.as_bytes());

    // 7. Small 2nd JPEG for MPF (32x24)
    let second_jpeg = encode_plain_jpeg(&pattern_rgb(32, 24), 32, 24)?;

    // Assemble the JPEG without MPF first to compute the exact offsets
    let mut prefix = Vec::new();
    prefix.extend_from_slice(&[0xFF, 0xD8]); // SOI
    prefix.extend_from_slice(&jfif_app0);
    prefix.extend_from_slice(&exif_app1);
    prefix.extend_from_slice(&xmp_app1);
    prefix.extend_from_slice(&icc_app2);

    let mut suffix = Vec::new();
    suffix.extend_from_slice(&iptc_app13);
    suffix.extend_from_slice(&com_seg);
    for seg in &segments {
        if !seg.starts_with(&[0xFF, 0xE0]) {
            suffix.extend_from_slice(seg);
        }
    }

    // Build MPF APP2 segment
    // MPF TIFF Header size = 8 bytes
    // MP Index IFD: 2 (count) + 3 * 12 (entries) + 4 (next IFD) + 32 (MPEntry values) = 74 bytes
    // Total MPF payload = 4 ("MPF\0") + 8 + 74 = 86 bytes
    // Marker + Length = 4 bytes -> total MPF segment = 90 bytes
    let mpf_tiff_len = 8 + 74;
    let mpf_seg_len = 2 + 4 + mpf_tiff_len;
    let distance_to_second_jpeg = (4 + mpf_tiff_len) + suffix.len();
    let image1_size = prefix.len() + (2 + mpf_seg_len) + suffix.len();

    let mpf_app2 = build_mpf_app2(
        image1_size as u32,
        second_jpeg.len() as u32,
        distance_to_second_jpeg as u32,
    );

    let mut full = Vec::new();
    full.extend_from_slice(&prefix);
    full.extend_from_slice(&mpf_app2);
    full.extend_from_slice(&suffix);
    full.extend_from_slice(&second_jpeg);
    Ok(full)
}

fn build_mpf_app2(image1_len: u32, image2_len: u32, image2_offset: u32) -> Vec<u8> {
    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II");
    tiff.extend_from_slice(&42u16.to_le_bytes());
    tiff.extend_from_slice(&8u32.to_le_bytes()); // Offset of MP Index IFD

    // MP Index IFD: 3 tags
    tiff.extend_from_slice(&3u16.to_le_bytes());

    // Tag 0xB000: MPFVersion (UNDEFINED, count 4, "0100")
    tiff.extend_from_slice(&0xB000u16.to_le_bytes());
    tiff.extend_from_slice(&7u16.to_le_bytes());
    tiff.extend_from_slice(&4u32.to_le_bytes());
    tiff.extend_from_slice(b"0100");

    // Tag 0xB001: NumberOfImages (LONG, count 1, 2)
    tiff.extend_from_slice(&0xB001u16.to_le_bytes());
    tiff.extend_from_slice(&4u16.to_le_bytes());
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&2u32.to_le_bytes());

    // Tag 0xB002: MPEntry (UNDEFINED, count 32)
    // Offset of MPEntry value from TIFF header: 8 + 2 + 3*12 + 4 = 50
    tiff.extend_from_slice(&0xB002u16.to_le_bytes());
    tiff.extend_from_slice(&7u16.to_le_bytes());
    tiff.extend_from_slice(&32u32.to_le_bytes());
    tiff.extend_from_slice(&50u32.to_le_bytes());

    // Next IFD offset: 0
    tiff.extend_from_slice(&0u32.to_le_bytes());

    // MPEntry 1 (Primary Image)
    tiff.extend_from_slice(&0x00030000u32.to_le_bytes()); // Type: Baseline MP Primary Image
    tiff.extend_from_slice(&image1_len.to_le_bytes()); // Image size
    tiff.extend_from_slice(&0u32.to_le_bytes()); // Data offset: 0
    tiff.extend_from_slice(&0u16.to_le_bytes()); // Dep 1
    tiff.extend_from_slice(&0u16.to_le_bytes()); // Dep 2

    // MPEntry 2 (Multi-frame Disparity / Sub-image)
    tiff.extend_from_slice(&0x00020002u32.to_le_bytes()); // Type: Multi-frame Disparity
    tiff.extend_from_slice(&image2_len.to_le_bytes()); // Image size
    tiff.extend_from_slice(&image2_offset.to_le_bytes()); // Offset from MPF TIFF header
    tiff.extend_from_slice(&0u16.to_le_bytes());
    tiff.extend_from_slice(&0u16.to_le_bytes());

    let mut app2 = Vec::new();
    app2.extend_from_slice(&[0xFF, 0xE2]);
    let len = 2 + 4 + tiff.len();
    app2.extend_from_slice(&(len as u16).to_be_bytes());
    app2.extend_from_slice(b"MPF\0");
    app2.extend_from_slice(&tiff);
    app2
}

fn build_orient_jpeg(pixels: &[u8], w: u32, h: u32, orientation: u16) -> Result<Vec<u8>> {
    let base = encode_plain_jpeg(pixels, w, h)?;
    let exif_tiff = generate_exif_tiff(Some(orientation), true, false)?;

    let mut exif_app1 = Vec::new();
    exif_app1.extend_from_slice(&[0xFF, 0xE1]);
    let len = 2 + 6 + exif_tiff.len();
    exif_app1.extend_from_slice(&(len as u16).to_be_bytes());
    exif_app1.extend_from_slice(b"Exif\0\0");
    exif_app1.extend_from_slice(&exif_tiff);

    let mut out = Vec::new();
    out.extend_from_slice(&[0xFF, 0xD8]);
    out.extend_from_slice(&exif_app1);
    out.extend_from_slice(&base[2..]);
    Ok(out)
}

fn build_cmyk_jpeg(pixels: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let mut base = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut base, JPEG_QUALITY);
    encoder.encode(
        pixels,
        u16::try_from(w)?,
        u16::try_from(h)?,
        jpeg_encoder::ColorType::Cmyk,
    )?;

    // Add EXIF metadata segment
    let exif_tiff = generate_exif_tiff(Some(1), false, false)?;
    let mut exif_app1 = Vec::new();
    exif_app1.extend_from_slice(&[0xFF, 0xE1]);
    let len = 2 + 6 + exif_tiff.len();
    exif_app1.extend_from_slice(&(len as u16).to_be_bytes());
    exif_app1.extend_from_slice(b"Exif\0\0");
    exif_app1.extend_from_slice(&exif_tiff);

    let mut out = Vec::new();
    out.extend_from_slice(&[0xFF, 0xD8]);
    out.extend_from_slice(&exif_app1);
    out.extend_from_slice(&base[2..]);
    Ok(out)
}

fn build_progressive_jpeg(pixels: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let mut base = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut base, JPEG_QUALITY);
    encoder.set_progressive(true);
    encoder.encode(
        pixels,
        u16::try_from(w)?,
        u16::try_from(h)?,
        jpeg_encoder::ColorType::Rgb,
    )?;

    // Find the first Start of Scan (0xFF 0xDA)
    let mut sos_pos = None;
    let mut i = 0;
    while i + 1 < base.len() {
        if base[i] == 0xFF && base[i + 1] == 0xDA {
            sos_pos = Some(i);
            break;
        }
        i += 1;
    }
    let sos_start = sos_pos.ok_or("progressive JPEG missing SOS marker")?;
    let sos_len = u16::from_be_bytes([base[sos_start + 2], base[sos_start + 3]]) as usize;
    let scan_start = sos_start + 2 + sos_len;

    // Scan until the next marker (excluding 0xFF 0x00 and RSTn 0xFF 0xD0..0xD7)
    let mut insert_pos = base.len();
    let mut j = scan_start;
    while j + 1 < base.len() {
        if base[j] == 0xFF {
            let next_byte = base[j + 1];
            if next_byte == 0x00 {
                j += 2;
                continue;
            }
            if (0xD0..=0xD7).contains(&next_byte) {
                j += 2;
                continue;
            }
            if next_byte == 0xFF {
                j += 1;
                continue;
            }
            // Found the next real marker between scans!
            insert_pos = j;
            break;
        }
        j += 1;
    }

    // Prepare COM and XMP APP1 segments to insert between scans
    let mut com_seg = Vec::new();
    com_seg.extend_from_slice(&[0xFF, 0xFE]);
    let com_len = 2 + COMMENT.len();
    com_seg.extend_from_slice(&(com_len as u16).to_be_bytes());
    com_seg.extend_from_slice(COMMENT.as_bytes());

    let xmp_str = generate_xmp_packet();
    let mut xmp_app1 = Vec::new();
    xmp_app1.extend_from_slice(&[0xFF, 0xE1]);
    let xmp_len = 2 + 29 + xmp_str.len();
    xmp_app1.extend_from_slice(&(xmp_len as u16).to_be_bytes());
    xmp_app1.extend_from_slice(b"http://ns.adobe.com/xap/1.0/\0");
    xmp_app1.extend_from_slice(xmp_str.as_bytes());

    let mut out = Vec::new();
    out.extend_from_slice(&base[..insert_pos]);
    out.extend_from_slice(&com_seg);
    out.extend_from_slice(&xmp_app1);
    out.extend_from_slice(&base[insert_pos..]);
    Ok(out)
}

fn build_no_jfif_dpi_jpeg(pixels: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let base = encode_plain_jpeg(pixels, w, h)?;
    let segments = split_jpeg_segments(&base)?;

    // EXIF with only resolution tags in IFD0
    let mut writer = exif::experimental::Writer::new();
    let xres = exif::Field {
        tag: exif::Tag::XResolution,
        ifd_num: exif::In::PRIMARY,
        value: exif::Value::Rational(vec![exif::Rational { num: 300, denom: 1 }]),
    };
    let yres = exif::Field {
        tag: exif::Tag::YResolution,
        ifd_num: exif::In::PRIMARY,
        value: exif::Value::Rational(vec![exif::Rational { num: 300, denom: 1 }]),
    };
    let unit = exif::Field {
        tag: exif::Tag::ResolutionUnit,
        ifd_num: exif::In::PRIMARY,
        value: exif::Value::Short(vec![2]),
    };
    writer.push_field(&xres);
    writer.push_field(&yres);
    writer.push_field(&unit);

    let mut cur = Cursor::new(Vec::new());
    writer.write(&mut cur, true)?;
    let tiff_bytes = cur.into_inner();

    let mut exif_app1 = Vec::new();
    exif_app1.extend_from_slice(&[0xFF, 0xE1]);
    let len = 2 + 6 + tiff_bytes.len();
    exif_app1.extend_from_slice(&(len as u16).to_be_bytes());
    exif_app1.extend_from_slice(b"Exif\0\0");
    exif_app1.extend_from_slice(&tiff_bytes);

    let mut out = Vec::new();
    out.extend_from_slice(&[0xFF, 0xD8]);
    out.extend_from_slice(&exif_app1);
    for seg in &segments {
        if !seg.starts_with(&[0xFF, 0xE0]) {
            out.extend_from_slice(seg);
        }
    }
    Ok(out)
}

fn split_jpeg_segments(data: &[u8]) -> Result<Vec<&[u8]>> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err("invalid JPEG SOI".into());
    }
    let mut segments = Vec::new();
    let mut i = 2;
    while i < data.len() {
        if data[i] != 0xFF {
            return Err("corrupted marker in JPEG".into());
        }
        let marker = data[i + 1];
        if marker == 0xD9 {
            // EOI
            segments.push(&data[i..i + 2]);
            break;
        }
        if marker == 0xDA {
            // SOS: scan data continues until EOI
            let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
            let mut j = i + 2 + len;
            while j + 1 < data.len() {
                if data[j] == 0xFF && data[j + 1] == 0xD9 {
                    break;
                }
                j += 1;
            }
            segments.push(&data[i..j]);
            i = j;
            continue;
        }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        segments.push(&data[i..i + 2 + len]);
        i += 2 + len;
    }
    Ok(segments)
}

// ---------------------------------------------------------------------------
// PNG Assembly

fn make_png_chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = Vec::with_capacity(12 + data.len());
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(tag);
    chunk.extend_from_slice(data);

    let mut hasher = crc32fast::Hasher::new();
    hasher.update(tag);
    hasher.update(data);
    let crc = hasher.finalize();
    chunk.extend_from_slice(&crc.to_be_bytes());
    chunk
}

/// Creates a stored (uncompressed) zlib stream (header 78 01 + stored blocks + adler32).
fn zlib_deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0x78, 0x01]); // zlib header: deflate, 32K window, check bits

    // Split data into blocks of at most 65535 bytes
    let chunks: Vec<&[u8]> = if data.is_empty() {
        vec![&[]]
    } else {
        data.chunks(65535).collect()
    };

    for (idx, chunk) in chunks.iter().enumerate() {
        let is_last = idx == chunks.len() - 1;
        let bfinal_btype = if is_last { 0x01u8 } else { 0x00u8 };
        out.push(bfinal_btype);
        let len = chunk.len() as u16;
        let nlen = !len;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(chunk);
    }

    // Adler-32 checksum
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for &b in data {
        s1 = (s1 + u32::from(b)) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s2 << 16) | s1
}

fn build_full_png(pixels: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let base_png = encode_plain_png(pixels, w, h)?;
    let (ihdr, idat_chunks, iend) = parse_png_chunks(&base_png)?;

    // 1. iCCP
    let icc = generate_icc_profile();
    let mut iccp_data = [ICC_DESCRIPTION.as_bytes(), b"\0\0"].concat();
    iccp_data.extend_from_slice(&zlib_deflate_stored(&icc));
    let iccp_chunk = make_png_chunk(b"iCCP", &iccp_data);

    // 2. pHYs (300 dpi = 11811 pixels per meter)
    let ppm = 11811u32;
    let mut phys_data = Vec::new();
    phys_data.extend_from_slice(&ppm.to_be_bytes());
    phys_data.extend_from_slice(&ppm.to_be_bytes());
    phys_data.push(1); // unit: meter
    let phys_chunk = make_png_chunk(b"pHYs", &phys_data);

    // 3. eXIf (raw TIFF bytes)
    let exif_tiff = generate_exif_tiff(Some(6), true, false)?;
    let exif_chunk = make_png_chunk(b"eXIf", &exif_tiff);

    // 4. tEXt (Author, Software)
    let text_author = make_png_chunk(b"tEXt", &[b"Author\0", AUTHOR.as_bytes()].concat());
    let text_software = make_png_chunk(b"tEXt", &[b"Software\0", SOFTWARE.as_bytes()].concat());

    // 5. zTXt
    let mut ztxt_data = b"Description\0\0".to_vec();
    ztxt_data.extend_from_slice(&zlib_deflate_stored(COMMENT.as_bytes()));
    let ztxt_chunk = make_png_chunk(b"zTXt", &ztxt_data);

    // 6. iTXt (XMP)
    let xmp_str = generate_xmp_packet();
    let mut itxt_data = b"XML:com.adobe.xmp\0\0\0\0\0".to_vec();
    itxt_data.extend_from_slice(xmp_str.as_bytes());
    let itxt_chunk = make_png_chunk(b"iTXt", &itxt_data);

    // 7. tIME (2026-01-02 03:04:05)
    let mut time_data = Vec::new();
    time_data.extend_from_slice(&2026u16.to_be_bytes());
    time_data.extend_from_slice(&[1, 2, 3, 4, 5]);
    let time_chunk = make_png_chunk(b"tIME", &time_data);

    // 8. tEXt after IDAT (Comment)
    let text_comment = make_png_chunk(b"tEXt", &[b"Comment\0", COMMENT.as_bytes()].concat());

    let mut out = Vec::new();
    out.extend_from_slice(&base_png[..8]); // PNG signature
    out.extend_from_slice(&ihdr);
    out.extend_from_slice(&iccp_chunk);
    out.extend_from_slice(&phys_chunk);
    out.extend_from_slice(&exif_chunk);
    out.extend_from_slice(&text_author);
    out.extend_from_slice(&text_software);
    out.extend_from_slice(&ztxt_chunk);
    out.extend_from_slice(&itxt_chunk);
    out.extend_from_slice(&time_chunk);
    for idat in &idat_chunks {
        out.extend_from_slice(idat);
    }
    out.extend_from_slice(&text_comment);
    out.extend_from_slice(&iend);
    out.extend_from_slice(b"Trailing arbitrary data after IEND\n");
    Ok(out)
}

fn build_anim_png(pixels: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let frame1_png = encode_plain_png(pixels, w, h)?;
    let (ihdr, idat1_chunks, _) = parse_png_chunks(&frame1_png)?;

    // Second frame: slight color shift
    let (pixel_chunks, _) = pixels.as_chunks::<4>();
    let frame2_pixels: Vec<u8> = pixel_chunks
        .iter()
        .flat_map(|px| [px[1], px[2], px[0], px[3]])
        .collect();
    let frame2_png = encode_plain_png(&frame2_pixels, w, h)?;
    let (_, idat2_chunks, _) = parse_png_chunks(&frame2_png)?;

    // Chunk sequence: IHDR, acTL (2 frames), fcTL (seq 0), IDAT, fcTL (seq 1), fdAT (seq 2), tEXt, IEND
    let mut actl_data = Vec::new();
    actl_data.extend_from_slice(&2u32.to_be_bytes()); // num_frames
    actl_data.extend_from_slice(&0u32.to_be_bytes()); // num_plays (infinite)
    let actl_chunk = make_png_chunk(b"acTL", &actl_data);

    let mut fctl0_data = Vec::new();
    fctl0_data.extend_from_slice(&0u32.to_be_bytes()); // sequence 0
    fctl0_data.extend_from_slice(&w.to_be_bytes());
    fctl0_data.extend_from_slice(&h.to_be_bytes());
    fctl0_data.extend_from_slice(&0u32.to_be_bytes()); // x_offset
    fctl0_data.extend_from_slice(&0u32.to_be_bytes()); // y_offset
    fctl0_data.extend_from_slice(&1u16.to_be_bytes()); // delay_num
    fctl0_data.extend_from_slice(&10u16.to_be_bytes()); // delay_den
    fctl0_data.push(0); // dispose_op
    fctl0_data.push(0); // blend_op
    let fctl0_chunk = make_png_chunk(b"fcTL", &fctl0_data);

    let mut fctl1_data = Vec::new();
    fctl1_data.extend_from_slice(&1u32.to_be_bytes()); // sequence 1
    fctl1_data.extend_from_slice(&w.to_be_bytes());
    fctl1_data.extend_from_slice(&h.to_be_bytes());
    fctl1_data.extend_from_slice(&0u32.to_be_bytes());
    fctl1_data.extend_from_slice(&0u32.to_be_bytes());
    fctl1_data.extend_from_slice(&1u16.to_be_bytes());
    fctl1_data.extend_from_slice(&10u16.to_be_bytes());
    fctl1_data.push(0);
    fctl1_data.push(0);
    let fctl1_chunk = make_png_chunk(b"fcTL", &fctl1_data);

    // Concatenate frame 2 IDAT payloads into fdAT chunk(s)
    let mut frame2_payload = Vec::new();
    for chunk in &idat2_chunks {
        frame2_payload.extend_from_slice(&chunk[8..chunk.len() - 4]);
    }
    let mut fdat_data = Vec::new();
    fdat_data.extend_from_slice(&2u32.to_be_bytes()); // sequence 2
    fdat_data.extend_from_slice(&frame2_payload);
    let fdat_chunk = make_png_chunk(b"fdAT", &fdat_data);

    let text_comment = make_png_chunk(b"tEXt", &[b"Comment\0", COMMENT.as_bytes()].concat());
    let iend = make_png_chunk(b"IEND", &[]);

    let mut out = Vec::new();
    out.extend_from_slice(&frame1_png[..8]);
    out.extend_from_slice(&ihdr);
    out.extend_from_slice(&actl_chunk);
    out.extend_from_slice(&fctl0_chunk);
    for chunk in &idat1_chunks {
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&fctl1_chunk);
    out.extend_from_slice(&fdat_chunk);
    out.extend_from_slice(&text_comment);
    out.extend_from_slice(&iend);
    Ok(out)
}

type PngChunkSplit = (Vec<u8>, Vec<Vec<u8>>, Vec<u8>);

fn parse_png_chunks(data: &[u8]) -> Result<PngChunkSplit> {
    if data.len() < 8 || &data[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("invalid PNG signature".into());
    }
    let mut ihdr = Vec::new();
    let mut idats = Vec::new();
    let mut iend = Vec::new();

    let mut i = 8;
    while i + 8 <= data.len() {
        let len = u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
        let chunk_total = 12 + len;
        if i + chunk_total > data.len() {
            return Err("truncated PNG chunk".into());
        }
        let chunk_type = &data[i + 4..i + 8];
        let chunk = data[i..i + chunk_total].to_vec();
        match chunk_type {
            b"IHDR" => ihdr = chunk,
            b"IDAT" => idats.push(chunk),
            b"IEND" => iend = chunk,
            _ => {}
        }
        i += chunk_total;
    }
    Ok((ihdr, idats, iend))
}

// ---------------------------------------------------------------------------
// WebP Assembly

fn extract_vp8l_payload(webp_data: &[u8]) -> Result<Vec<u8>> {
    if webp_data.len() < 12 || &webp_data[..4] != b"RIFF" || &webp_data[8..12] != b"WEBP" {
        return Err("invalid WebP RIFF header".into());
    }
    let mut i = 12;
    while i + 8 <= webp_data.len() {
        let tag = &webp_data[i..i + 4];
        let len = u32::from_le_bytes([
            webp_data[i + 4],
            webp_data[i + 5],
            webp_data[i + 6],
            webp_data[i + 7],
        ]) as usize;
        if tag == b"VP8L" {
            let start = i + 8;
            let end = start + len;
            if end > webp_data.len() {
                return Err("truncated VP8L chunk".into());
            }
            return Ok(webp_data[start..end].to_vec());
        }
        let padded_len = len + (len % 2);
        i += 8 + padded_len;
    }
    Err("VP8L chunk not found".into())
}

fn build_full_webp(pixels: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let plain = encode_plain_webp(pixels, w, h)?;
    let vp8l_data = extract_vp8l_payload(&plain)?;

    let icc = generate_icc_profile();
    let exif_tiff = generate_exif_tiff(Some(6), true, false)?;
    let xmp_str = generate_xmp_packet();

    // VP8X payload: 10 bytes
    // Flags: ICC (0x20) | Alpha (0x10) | EXIF (0x08) | XMP (0x04) = 0x3C
    let mut vp8x = Vec::new();
    vp8x.extend_from_slice(&[0x3C, 0, 0, 0]);
    let canvas_w = w - 1;
    let canvas_h = h - 1;
    vp8x.extend_from_slice(&canvas_w.to_le_bytes()[..3]);
    vp8x.extend_from_slice(&canvas_h.to_le_bytes()[..3]);

    let mut chunks = Vec::new();
    append_riff_chunk(&mut chunks, b"VP8X", &vp8x);
    append_riff_chunk(&mut chunks, b"ICCP", &icc);
    append_riff_chunk(&mut chunks, b"VP8L", &vp8l_data);
    append_riff_chunk(&mut chunks, b"EXIF", &exif_tiff);
    append_riff_chunk(&mut chunks, b"XMP ", xmp_str.as_bytes());

    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    let riff_size = (4 + chunks.len()) as u32;
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(&chunks);
    Ok(out)
}

fn build_anim_webp(pixels: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let plain1 = encode_plain_webp(pixels, w, h)?;
    let vp8l1 = extract_vp8l_payload(&plain1)?;

    let (pixel_chunks, _) = pixels.as_chunks::<4>();
    let frame2_pixels: Vec<u8> = pixel_chunks
        .iter()
        .flat_map(|px| [px[1], px[2], px[0], px[3]])
        .collect();
    let plain2 = encode_plain_webp(&frame2_pixels, w, h)?;
    let vp8l2 = extract_vp8l_payload(&plain2)?;

    let xmp_str = generate_xmp_packet();

    // VP8X payload: Flags: Animation (0x02) | Alpha (0x10) | XMP (0x04) = 0x16
    let mut vp8x = Vec::new();
    vp8x.extend_from_slice(&[0x16, 0, 0, 0]);
    let canvas_w = w - 1;
    let canvas_h = h - 1;
    vp8x.extend_from_slice(&canvas_w.to_le_bytes()[..3]);
    vp8x.extend_from_slice(&canvas_h.to_le_bytes()[..3]);

    // ANIM payload: BGRA color (4 bytes) + loop count (2 bytes)
    let mut anim = Vec::new();
    anim.extend_from_slice(&[0, 0, 0, 0]); // background
    anim.extend_from_slice(&0u16.to_le_bytes()); // loop count: 0 (infinite)

    // Build ANMF chunks
    let anmf1 = build_anmf_chunk(w, h, &vp8l1);
    let anmf2 = build_anmf_chunk(w, h, &vp8l2);

    let mut chunks = Vec::new();
    append_riff_chunk(&mut chunks, b"VP8X", &vp8x);
    append_riff_chunk(&mut chunks, b"ANIM", &anim);
    append_riff_chunk(&mut chunks, b"ANMF", &anmf1);
    append_riff_chunk(&mut chunks, b"ANMF", &anmf2);
    append_riff_chunk(&mut chunks, b"XMP ", xmp_str.as_bytes());

    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    let riff_size = (4 + chunks.len()) as u32;
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(&chunks);
    Ok(out)
}

fn build_anmf_chunk(w: u32, h: u32, vp8l: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&[0, 0, 0]); // frame x: 0
    payload.extend_from_slice(&[0, 0, 0]); // frame y: 0
    let fw = w - 1;
    let fh = h - 1;
    payload.extend_from_slice(&fw.to_le_bytes()[..3]);
    payload.extend_from_slice(&fh.to_le_bytes()[..3]);
    payload.extend_from_slice(&100u32.to_le_bytes()[..3]); // duration: 100 ms
    payload.push(0); // flags: blend=0, dispose=0

    append_riff_chunk(&mut payload, b"VP8L", vp8l);
    payload
}

fn append_riff_chunk(buf: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    buf.extend_from_slice(tag);
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
    buf.extend_from_slice(data);
    if !data.len().is_multiple_of(2) {
        buf.push(0);
    }
}

// ---------------------------------------------------------------------------
// PDF Generation

fn build_full_pdf(full_jpg: &[u8], img_w: u32, img_h: u32) -> Result<Vec<u8>> {
    let xmp = generate_xmp_packet();
    let file_id = "0123456789ABCDEF0123456789ABCDEF";

    // 4x4 raw RGB thumb pixels (48 bytes)
    let thumb_pixels = pattern_rgb(4, 4);

    let objects: Vec<Vec<u8>> = vec![
        // 1: Catalog
        b"<< /Type /Catalog /Pages 2 0 R /Metadata 3 0 R >>".to_vec(),
        // 2: Pages
        b"<< /Type /Pages /Kids [4 0 R] /Count 1 >>".to_vec(),
        // 3: XMP Metadata stream
        format!(
            "<< /Type /Metadata /Subtype /XML /Length {} >>\nstream\n{}\nendstream",
            xmp.len(),
            xmp
        )
        .into_bytes(),
        // 4: Page
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.28 841.89] /Contents 5 0 R /Resources << /XObject << /Im0 6 0 R >> >> /Metadata 3 0 R /PieceInfo << /Illustrator << /Private (Example Private Data) >> >> /Thumb 7 0 R >>".to_vec(),
        // 5: Contents
        b"<< /Length 39 >>\nstream\nq 200 0 0 150 100 500 cm /Im0 Do Q\nendstream".to_vec(),
        // 6: DCTDecode image
        [
            format!(
                "<< /Type /XObject /Subtype /Image /Width {img_w} /Height {img_h} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
                full_jpg.len()
            )
            .into_bytes(),
            full_jpg.to_vec(),
            b"\nendstream".to_vec(),
        ]
        .concat(),
        // 7: Thumb image
        [
            b"<< /Type /XObject /Subtype /Image /Width 4 /Height 4 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length 48 >>\nstream\n".to_vec(),
            thumb_pixels,
            b"\nendstream".to_vec(),
        ]
        .concat(),
        // 8: Initial Info dict (Author: Example Author)
        format!(
            "<< /Title ({TITLE}) /Author ({AUTHOR}) /Subject ({SUBJECT}) /Keywords ({KEYWORDS}) /Creator ({SOFTWARE}) /Producer ({SOFTWARE}) /CreationDate ({DATE_TIME_PDF}) /ModDate ({DATE_TIME_PDF}) >>"
        )
        .into_bytes(),
    ];

    let mut pdf = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (index, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        pdf.extend_from_slice(body);
        pdf.extend_from_slice(b"\nendobj\n");
    }

    let xref1_offset = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in &offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 8 0 R /ID [<{file_id}> <{file_id}>] >>\nstartxref\n{xref1_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );

    // Second incremental version (re-write object 8 with Author: Example Editor)
    let obj8_v2_offset = pdf.len();
    pdf.extend_from_slice(b"8 0 obj\n");
    let info_v2 = format!(
        "<< /Title ({TITLE}) /Author ({EDITOR}) /Subject ({SUBJECT}) /Keywords ({KEYWORDS}) /Creator ({SOFTWARE}) /Producer ({SOFTWARE}) /CreationDate ({DATE_TIME_PDF}) /ModDate ({DATE_TIME_PDF}) >>\nendobj\n"
    );
    pdf.extend_from_slice(info_v2.as_bytes());

    let xref2_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n8 1\n{obj8_v2_offset:010} 00000 n \n").as_bytes());
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 8 0 R /ID [<{file_id}> <{file_id}>] /Prev {xref1_offset} >>\nstartxref\n{xref2_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );

    Ok(pdf)
}

fn build_linearized_pdf() -> Result<Vec<u8>> {
    // Generate linearized PDF without metadata
    // Assembled twice so /L exactly matches the final file size.
    let mut file_len = 1000usize;
    for _ in 0..2 {
        let pdf = assemble_linearized_pdf(file_len);
        file_len = pdf.len();
    }
    Ok(assemble_linearized_pdf(file_len))
}

fn assemble_linearized_pdf(total_len: usize) -> Vec<u8> {
    let mut pdf = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();

    // Object 1: Linearized dictionary
    let obj1_offset = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n");
    pdf.extend_from_slice(
        format!("<< /Linearized 1.0 /L {total_len} /H [0 0] /O 4 /E 0 /N 1 /T 0 >>\nendobj\n")
            .as_bytes(),
    );

    // First page xref and trailer
    let first_xref_offset = pdf.len();
    pdf.extend_from_slice(b"xref\n0 2\n0000000000 65535 f \n");
    pdf.extend_from_slice(format!("{obj1_offset:010} 00000 n \n").as_bytes());
    // Trailer points to the main xref at the end of the file via /Prev
    let trailer_placeholder_pos = pdf.len();
    // We will patch the /Prev offset once main_xref_offset is known
    pdf.extend_from_slice(
        b"trailer\n<< /Size 6 /Root 2 0 R /Prev 0000000000 >>\nstartxref\n0\n%%EOF\n",
    );

    // Main objects
    let obj2_offset = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Catalog /Pages 3 0 R >>\nendobj\n");

    let obj3_offset = pdf.len();
    pdf.extend_from_slice(b"3 0 obj\n<< /Type /Pages /Kids [4 0 R] /Count 1 >>\nendobj\n");

    let obj4_offset = pdf.len();
    pdf.extend_from_slice(b"4 0 obj\n<< /Type /Page /Parent 3 0 R /MediaBox [0 0 595.28 841.89] /Contents 5 0 R /Resources << >> >>\nendobj\n");

    let obj5_offset = pdf.len();
    pdf.extend_from_slice(b"5 0 obj\n<< /Length 0 >>\nstream\n\nendstream\nendobj\n");

    let main_xref_offset = pdf.len();

    // Patch the /Prev in the first trailer
    let prev_str = format!("{main_xref_offset:010}");
    let prev_target = trailer_placeholder_pos + "trailer\n<< /Size 6 /Root 2 0 R /Prev ".len();
    pdf[prev_target..prev_target + 10].copy_from_slice(prev_str.as_bytes());

    // Main xref
    pdf.extend_from_slice(b"xref\n2 4\n");
    pdf.extend_from_slice(format!("{obj2_offset:010} 00000 n \n").as_bytes());
    pdf.extend_from_slice(format!("{obj3_offset:010} 00000 n \n").as_bytes());
    pdf.extend_from_slice(format!("{obj4_offset:010} 00000 n \n").as_bytes());
    pdf.extend_from_slice(format!("{obj5_offset:010} 00000 n \n").as_bytes());

    pdf.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 2 0 R >>\nstartxref\n{first_xref_offset}\n%%EOF\n")
            .as_bytes(),
    );
    pdf
}

fn build_signed_pdf() -> Result<Vec<u8>> {
    let file_id = "9876543210ABCDEF9876543210ABCDEF";
    let objects: Vec<Vec<u8>> = vec![
        // 1: Catalog with /AcroForm
        b"<< /Type /Catalog /Pages 2 0 R /AcroForm << /SigFlags 3 /Fields [5 0 R] >> >>".to_vec(),
        // 2: Pages
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        // 3: Page with signature widget annotation
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.28 841.89] /Contents 4 0 R /Annots [5 0 R] /Resources << >> >>".to_vec(),
        // 4: Contents
        b"<< /Length 0 >>\nstream\n\nendstream".to_vec(),
        // 5: Signature field widget
        b"<< /Type /Annot /Subtype /Widget /FT /Sig /T (Signature1) /V 6 0 R /Rect [0 0 0 0] >>".to_vec(),
        // 6: Signature dictionary
        b"<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /adbe.pkcs7.detached /ByteRange [0 100 200 300] /Contents <00000000000000000000000000000000> /Name (Example Signer) >>".to_vec(),
    ];

    let mut pdf = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (index, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        pdf.extend_from_slice(body);
        pdf.extend_from_slice(b"\nendobj\n");
    }

    let xref_offset = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in &offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /ID [<{file_id}> <{file_id}>] >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    Ok(pdf)
}

// ---------------------------------------------------------------------------
// Encrypted PDFs (standard security handler, revision 3, 128-bit RC4)

fn encrypted_pdf(label: &str, user_password: &str, permissions: i32) -> Vec<u8> {
    let file_id = md5(&[b"mcleaner-fixture:", label.as_bytes()].concat());
    let owner_entry = owner_entry(OWNER_PASSWORD.as_bytes(), user_password.as_bytes());
    let key = file_key(
        user_password.as_bytes(),
        &owner_entry,
        permissions,
        &file_id,
    );
    let user_entry = user_entry(&key, &file_id);

    const CONTENT_OBJECT: u32 = 4;
    let content = b"0.16 0.31 0.78 rg 100 400 300 200 re f\n";
    let encrypted_content = rc4(&object_key(&key, CONTENT_OBJECT, 0), content);

    let objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.28 841.89] /Contents 4 0 R /Resources << >> >>".to_vec(),
        [
            format!("<< /Length {} >>\nstream\n", encrypted_content.len()).into_bytes(),
            encrypted_content,
            b"\nendstream".to_vec(),
        ]
        .concat(),
        format!(
            "<< /Filter /Standard /V 2 /R 3 /Length {} /O <{}> /U <{}> /P {permissions} >>",
            KEY_LENGTH_BYTES * 8,
            hex(&owner_entry),
            hex(&user_entry)
        )
        .into_bytes(),
    ];

    let mut pdf = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (index, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        pdf.extend_from_slice(body);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    let id = hex(&file_id);
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Encrypt {} 0 R /ID [<{id}> <{id}>] >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1,
            objects.len()
        )
        .as_bytes(),
    );
    pdf
}

fn padded(password: &[u8]) -> Vec<u8> {
    password
        .iter()
        .chain(PASSWORD_PADDING.iter())
        .take(PASSWORD_PADDING.len())
        .copied()
        .collect()
}

fn owner_entry(owner_password: &[u8], user_password: &[u8]) -> Vec<u8> {
    let mut hash = md5(&padded(owner_password));
    for _ in 0..KEY_HASH_ROUNDS {
        hash = md5(&hash[..KEY_LENGTH_BYTES]);
    }
    rc4_rounds(&hash[..KEY_LENGTH_BYTES], padded(user_password))
}

fn file_key(user_password: &[u8], owner_entry: &[u8], permissions: i32, file_id: &[u8]) -> Vec<u8> {
    let input = [
        &padded(user_password)[..],
        owner_entry,
        &permissions.to_le_bytes(),
        file_id,
    ]
    .concat();
    let mut hash = md5(&input);
    for _ in 0..KEY_HASH_ROUNDS {
        hash = md5(&hash[..KEY_LENGTH_BYTES]);
    }
    hash[..KEY_LENGTH_BYTES].to_vec()
}

fn user_entry(key: &[u8], file_id: &[u8]) -> Vec<u8> {
    let hash = md5(&[&PASSWORD_PADDING[..], file_id].concat());
    let mut entry = rc4_rounds(key, hash.to_vec());
    entry.resize(32, 0);
    entry
}

fn rc4_rounds(key: &[u8], data: Vec<u8>) -> Vec<u8> {
    let mut data = rc4(key, &data);
    for round in 1..=RC4_ROUNDS {
        let round_key: Vec<u8> = key.iter().map(|b| b ^ round).collect();
        data = rc4(&round_key, &data);
    }
    data
}

fn object_key(key: &[u8], object: u32, generation: u16) -> Vec<u8> {
    let input = [key, &object.to_le_bytes()[..3], &generation.to_le_bytes()].concat();
    md5(&input)[..(key.len() + 5).min(16)].to_vec()
}

fn rc4(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut s: [u8; 256] = std::array::from_fn(|i| i as u8);
    let mut j: u8 = 0;
    for i in 0..256 {
        j = j.wrapping_add(s[i]).wrapping_add(key[i % key.len()]);
        s.swap(i, usize::from(j));
    }
    let (mut i, mut j) = (0u8, 0u8);
    data.iter()
        .map(|byte| {
            i = i.wrapping_add(1);
            j = j.wrapping_add(s[usize::from(i)]);
            s.swap(usize::from(i), usize::from(j));
            byte ^ s[usize::from(s[usize::from(i)].wrapping_add(s[usize::from(j)]))]
        })
        .collect()
}

fn md5(data: &[u8]) -> [u8; 16] {
    Md5::digest(data).into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}
