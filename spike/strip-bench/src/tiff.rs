//! Little-endian TIFF (EXIF) builder for the fixtures and for the
//! orientation-only EXIF the cleaner writes back.

pub enum Val {
    Ascii(&'static str),
    Short(u16),
    Long(u32),
    Rational(Vec<(u32, u32)>),
    Undefined(Vec<u8>),
}

pub struct Entry(pub u16, pub Val);

fn encode(v: &Val) -> (u16, u32, Vec<u8>) {
    match v {
        Val::Ascii(s) => {
            let mut b = s.as_bytes().to_vec();
            b.push(0);
            (2, b.len() as u32, b)
        }
        Val::Short(n) => (3, 1, n.to_le_bytes().to_vec()),
        Val::Long(n) => (4, 1, n.to_le_bytes().to_vec()),
        Val::Rational(rs) => {
            let mut b = Vec::new();
            for (n, d) in rs {
                b.extend(n.to_le_bytes());
                b.extend(d.to_le_bytes());
            }
            (5, rs.len() as u32, b)
        }
        Val::Undefined(b) => (7, b.len() as u32, b.clone()),
    }
}

fn padded(n: usize) -> usize {
    n + (n & 1)
}

pub fn ifd_size(entries: &[Entry]) -> usize {
    let data: usize = entries
        .iter()
        .map(|e| encode(&e.1).2.len())
        .filter(|&n| n > 4)
        .map(padded)
        .sum();
    2 + 12 * entries.len() + 4 + data
}

/// Appends an IFD at `out.len()` (offsets count from the TIFF header, which
/// `out` starts with) with its out-of-line values right after it.
pub fn write_ifd(out: &mut Vec<u8>, entries: &[Entry], next: u32) {
    let start = out.len();
    let mut data_off = start + 2 + 12 * entries.len() + 4;
    let mut data = Vec::new();
    out.extend((entries.len() as u16).to_le_bytes());
    for Entry(tag, v) in entries {
        let (ty, count, bytes) = encode(v);
        out.extend(tag.to_le_bytes());
        out.extend(ty.to_le_bytes());
        out.extend(count.to_le_bytes());
        if bytes.len() <= 4 {
            let mut inline = bytes.clone();
            inline.resize(4, 0);
            out.extend(inline);
        } else {
            out.extend((data_off as u32).to_le_bytes());
            data.extend(&bytes);
            if bytes.len() & 1 == 1 {
                data.push(0);
            }
            data_off += padded(bytes.len());
        }
    }
    out.extend(next.to_le_bytes());
    out.extend(data);
}

fn header() -> Vec<u8> {
    let mut out = b"II*\0".to_vec();
    out.extend(8u32.to_le_bytes());
    out
}

pub fn orientation_only(orientation: u16) -> Vec<u8> {
    let mut out = header();
    write_ifd(&mut out, &[Entry(0x0112, Val::Short(orientation))], 0);
    out
}

/// EXIF with every kind of metadata the cleaner must remove. The location
/// is made up and the names are placeholders.
pub fn full(orientation: u16, thumbnail: &[u8]) -> Vec<u8> {
    let ifd0 = |exif: u32, gps: u32| {
        vec![
            Entry(0x010F, Val::Ascii("SpikeMake")),
            Entry(0x0110, Val::Ascii("SpikeModel X1")),
            Entry(0x0112, Val::Short(orientation)),
            Entry(0x0131, Val::Ascii("SpikeSoftware 1.0")),
            Entry(0x0132, Val::Ascii("2026:01:02 03:04:05")),
            Entry(0x013B, Val::Ascii("Spike Artist")),
            Entry(0x8298, Val::Ascii("Spike Copyright")),
            Entry(0x8769, Val::Long(exif)),
            Entry(0x8825, Val::Long(gps)),
        ]
    };
    let exif_ifd = vec![
        Entry(0x9003, Val::Ascii("2026:01:02 03:04:05")),
        Entry(0x927C, Val::Undefined(b"SpikeMakerNote-private".to_vec())),
        Entry(0xA431, Val::Ascii("SPIKE-SERIAL-0001")),
    ];
    let gps_ifd = vec![
        Entry(0x0001, Val::Ascii("N")),
        Entry(0x0002, Val::Rational(vec![(12, 1), (34, 1), (5600, 100)])),
        Entry(0x0003, Val::Ascii("E")),
        Entry(0x0004, Val::Rational(vec![(65, 1), (43, 1), (2100, 100)])),
    ];
    let ifd1 = |off: u32| {
        vec![
            Entry(0x0201, Val::Long(off)),
            Entry(0x0202, Val::Long(thumbnail.len() as u32)),
        ]
    };
    let s0 = ifd_size(&ifd0(0, 0));
    let se = ifd_size(&exif_ifd);
    let sg = ifd_size(&gps_ifd);
    let s1 = ifd_size(&ifd1(0));
    let exif_off = 8 + s0;
    let gps_off = exif_off + se;
    let ifd1_off = gps_off + sg;
    let thumb_off = ifd1_off + s1;
    let mut out = header();
    write_ifd(&mut out, &ifd0(exif_off as u32, gps_off as u32), ifd1_off as u32);
    write_ifd(&mut out, &exif_ifd, 0);
    write_ifd(&mut out, &gps_ifd, 0);
    write_ifd(&mut out, &ifd1(thumb_off as u32), 0);
    assert_eq!(out.len(), thumb_off);
    out.extend(thumbnail);
    out
}

/// Orientation (1 when absent) read with kamadak-exif.
pub fn orientation(tiff: &[u8]) -> Option<u16> {
    let exif = exif::Reader::new().read_raw(tiff.to_vec()).ok()?;
    let f = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    f.value.get_uint(0).map(|v| v as u16)
}

/// What a user would care about in an EXIF block.
pub fn categories(tiff: &[u8]) -> Vec<String> {
    let Ok(exif) = exif::Reader::new().read_raw(tiff.to_vec()) else {
        return vec!["EXIF (unreadable)".into()];
    };
    let mut cats = std::collections::BTreeSet::new();
    for f in exif.fields() {
        use exif::{Context, Tag};
        let c = match f.tag {
            _ if f.tag.context() == Context::Gps => "location",
            _ if f.ifd_num == exif::In::THUMBNAIL => "thumbnail",
            Tag::DateTime | Tag::DateTimeOriginal | Tag::DateTimeDigitized => "date",
            Tag::Make | Tag::Model | Tag::BodySerialNumber | Tag::LensModel => "device",
            Tag::MakerNote => "device",
            Tag::Software => "software",
            Tag::Artist | Tag::Copyright => "author",
            Tag::Orientation => "orientation(kept)",
            Tag::ExifIFDPointer | Tag::GPSInfoIFDPointer => continue,
            _ => "other",
        };
        cats.insert(c.to_string());
    }
    cats.into_iter().collect()
}
