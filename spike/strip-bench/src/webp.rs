//! WebP (RIFF) chunk walker.

pub struct Chunk {
    pub id: [u8; 4],
    pub data: Vec<u8>,
}

pub fn parse(d: &[u8]) -> Result<(Vec<Chunk>, usize), String> {
    if d.len() < 12 || &d[0..4] != b"RIFF" || &d[8..12] != b"WEBP" {
        return Err("not a WebP".into());
    }
    let riff_end = 8 + u32::from_le_bytes(d[4..8].try_into().unwrap()) as usize;
    if riff_end > d.len() {
        return Err("truncated RIFF".into());
    }
    let mut pos = 12;
    let mut chunks = Vec::new();
    while pos + 8 <= riff_end {
        let id: [u8; 4] = d[pos..pos + 4].try_into().unwrap();
        let len = u32::from_le_bytes(d[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let end = pos + 8 + len;
        if end > riff_end {
            return Err("truncated chunk".into());
        }
        chunks.push(Chunk { id, data: d[pos + 8..end].to_vec() });
        pos = end + (len & 1);
    }
    Ok((chunks, d.len() - riff_end))
}

const KEEP: [&[u8; 4]; 7] = [b"VP8 ", b"VP8L", b"VP8X", b"ALPH", b"ANIM", b"ANMF", b"ICCP"];
const FLAG_EXIF: u8 = 0x08;
const FLAG_XMP: u8 = 0x04;

fn exif_tiff(data: &[u8]) -> &[u8] {
    data.strip_prefix(crate::jpeg::EXIF_PREFIX).unwrap_or(data)
}

pub fn build(chunks: &[Chunk]) -> Vec<u8> {
    let mut body = b"WEBP".to_vec();
    for c in chunks {
        body.extend(c.id);
        body.extend((c.data.len() as u32).to_le_bytes());
        body.extend(&c.data);
        if c.data.len() & 1 == 1 {
            body.push(0);
        }
    }
    let mut out = b"RIFF".to_vec();
    out.extend((body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

pub fn strip(d: &[u8]) -> Result<Vec<u8>, String> {
    let (chunks, _) = parse(d)?;
    let mut out = Vec::new();
    let mut has_exif = false;
    for c in chunks {
        if &c.id == b"EXIF" {
            if let Some(o) = crate::tiff::orientation(exif_tiff(&c.data)).filter(|&o| o != 1) {
                out.push(Chunk { id: *b"EXIF", data: crate::tiff::orientation_only(o) });
                has_exif = true;
            }
        } else if KEEP.contains(&&c.id) {
            out.push(c);
        }
    }
    if let Some(x) = out.iter_mut().find(|c| &c.id == b"VP8X") {
        x.data[0] &= !(FLAG_EXIF | FLAG_XMP);
        if has_exif {
            x.data[0] |= FLAG_EXIF;
        }
    }
    // EXIF must come after the image data.
    out.sort_by_key(|c| &c.id == b"EXIF");
    Ok(build(&out))
}

pub fn describe(d: &[u8]) -> Result<Vec<String>, String> {
    let (chunks, trailer) = parse(d)?;
    let mut found = Vec::new();
    for c in &chunks {
        match &c.id {
            b"EXIF" => found.extend(crate::tiff::categories(exif_tiff(&c.data))),
            b"XMP " => found.push("XMP".into()),
            b"ICCP" => found.push("ICC(kept)".into()),
            k if !KEEP.contains(&k) => found.push(format!("chunk {}", String::from_utf8_lossy(k))),
            _ => {}
        }
    }
    if trailer > 0 {
        found.push(format!("{trailer} bytes after RIFF"));
    }
    Ok(found)
}

pub fn image_chunks(d: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    Ok(parse(d)?
        .0
        .into_iter()
        .filter(|c| KEEP.contains(&&c.id) && &c.id != b"VP8X")
        .map(|c| c.data)
        .collect())
}
