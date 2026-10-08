//! JPEG segment walker. Unlike img-parts it reads every scan of a
//! progressive file and stops at EOI, so segments between scans and data
//! appended after EOI are seen.

pub struct Segment {
    pub marker: u8,
    /// The whole segment: marker, length, payload and, for SOS, its
    /// entropy-coded data.
    pub bytes: Vec<u8>,
}

impl Segment {
    pub fn payload(&self) -> &[u8] {
        if self.bytes.len() >= 4 { &self.bytes[4..] } else { &[] }
    }
}

pub struct Parsed {
    pub segments: Vec<Segment>,
    pub trailer: usize,
}

const EXIF: &[u8] = b"Exif\0\0";
const XMP: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const ICC: &[u8] = b"ICC_PROFILE\0";

pub fn parse(d: &[u8]) -> Result<Parsed, String> {
    if !d.starts_with(&[0xFF, 0xD8]) {
        return Err("not a JPEG".into());
    }
    let mut pos = 2;
    let mut segments = Vec::new();
    loop {
        if pos >= d.len() || d[pos] != 0xFF {
            return Err(format!("no marker at {pos}"));
        }
        while pos < d.len() && d[pos] == 0xFF {
            pos += 1;
        }
        if pos >= d.len() {
            return Err("truncated".into());
        }
        let marker = d[pos];
        pos += 1;
        let start = pos - 2;
        match marker {
            0xD9 => return Ok(Parsed { segments, trailer: d.len() - pos }),
            0x01 | 0xD0..=0xD7 => {
                segments.push(Segment { marker, bytes: d[start..pos].to_vec() });
                continue;
            }
            _ => {}
        }
        if pos + 2 > d.len() {
            return Err("truncated length".into());
        }
        let len = u16::from_be_bytes([d[pos], d[pos + 1]]) as usize;
        if len < 2 || pos + len > d.len() {
            return Err(format!("bad length at {pos}"));
        }
        pos += len;
        if marker == 0xDA {
            loop {
                if pos + 1 >= d.len() {
                    return Err("truncated scan".into());
                }
                if d[pos] == 0xFF {
                    let n = d[pos + 1];
                    if n == 0x00 || (0xD0..=0xD7).contains(&n) {
                        pos += 2;
                        continue;
                    }
                    break;
                }
                pos += 1;
            }
        }
        segments.push(Segment { marker, bytes: d[start..pos].to_vec() });
    }
}

fn segment(marker: u8, payload: &[u8]) -> Segment {
    let mut bytes = vec![0xFF, marker];
    bytes.extend(((payload.len() + 2) as u16).to_be_bytes());
    bytes.extend(payload);
    Segment { marker, bytes }
}

pub fn build(segments: &[Segment], trailer: &[u8]) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8];
    for s in segments {
        out.extend(&s.bytes);
    }
    out.extend([0xFF, 0xD9]);
    out.extend(trailer);
    out
}

/// Segments that only carry pixel data or what decoding needs.
pub fn is_metadata(s: &Segment) -> bool {
    matches!(s.marker, 0xE0..=0xEF | 0xFE)
}

pub enum Keep {
    /// JFIF without its thumbnail.
    Jfif(Vec<u8>),
    Icc,
    Adobe,
    Drop,
}

fn classify(s: &Segment) -> Keep {
    let p = s.payload();
    match s.marker {
        0xE0 if p.starts_with(b"JFIF\0") && p.len() >= 14 => {
            let mut j = p[..14].to_vec();
            j[12] = 0;
            j[13] = 0;
            Keep::Jfif(j)
        }
        0xE2 if p.starts_with(ICC) => Keep::Icc,
        0xEE if p.starts_with(b"Adobe") => Keep::Adobe,
        _ => Keep::Drop,
    }
}

/// `keep_orientation` is false inside a PDF, where viewers ignore it.
pub fn strip(d: &[u8], keep_orientation: bool) -> Result<Vec<u8>, String> {
    let parsed = parse(d)?;
    let mut out = Vec::new();
    let mut orientation_written = false;
    for s in parsed.segments {
        if !is_metadata(&s) {
            out.push(s);
            continue;
        }
        if s.marker == 0xE1 && s.payload().starts_with(EXIF) {
            if keep_orientation && !orientation_written {
                if let Some(o) = crate::tiff::orientation(&s.payload()[EXIF.len()..]).filter(|&o| o != 1) {
                    let mut p = EXIF.to_vec();
                    p.extend(crate::tiff::orientation_only(o));
                    out.push(segment(0xE1, &p));
                    orientation_written = true;
                }
            }
            continue;
        }
        match classify(&s) {
            Keep::Jfif(p) => out.push(segment(0xE0, &p)),
            Keep::Icc | Keep::Adobe => out.push(s),
            Keep::Drop => {}
        }
    }
    Ok(build(&out, &[]))
}

pub fn describe(d: &[u8]) -> Result<Vec<String>, String> {
    let parsed = parse(d)?;
    let mut found = Vec::new();
    for s in &parsed.segments {
        let p = s.payload();
        match s.marker {
            0xE1 if p.starts_with(EXIF) => found.extend(crate::tiff::categories(&p[EXIF.len()..])),
            0xE1 if p.starts_with(XMP) => found.push("XMP".into()),
            0xE0 if p.starts_with(b"JFIF\0") => {
                if p.len() >= 14 && (p[12] != 0 || p[13] != 0) {
                    found.push("JFIF thumbnail".into());
                }
            }
            0xE2 if p.starts_with(ICC) => found.push("ICC(kept)".into()),
            0xE2 if p.starts_with(b"MPF\0") => found.push("MPF (extra images)".into()),
            0xED => found.push("Photoshop/IPTC".into()),
            0xEE if p.starts_with(b"Adobe") => {}
            0xFE => found.push("comment".into()),
            0xE0..=0xEF => found.push(format!("APP{}", s.marker - 0xE0)),
            _ => {}
        }
    }
    if parsed.trailer > 0 {
        found.push(format!("{} bytes after EOI", parsed.trailer));
    }
    Ok(found)
}

/// Everything except metadata segments, for the byte-for-byte check.
pub fn image_segments(d: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    Ok(parse(d)?
        .segments
        .into_iter()
        .filter(|s| !is_metadata(s) || matches!(classify(s), Keep::Icc | Keep::Adobe))
        .map(|s| s.bytes)
        .collect())
}

pub fn app(marker: u8, prefix: &[u8], body: &[u8]) -> Vec<u8> {
    let mut p = prefix.to_vec();
    p.extend(body);
    segment(marker, &p).bytes
}

pub const EXIF_PREFIX: &[u8] = EXIF;
pub const XMP_PREFIX: &[u8] = XMP;
pub const ICC_PREFIX: &[u8] = ICC;
