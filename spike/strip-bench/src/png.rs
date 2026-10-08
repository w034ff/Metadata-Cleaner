//! PNG chunk walker that stops at IEND.

pub const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";

pub struct Chunk {
    pub kind: [u8; 4],
    pub data: Vec<u8>,
}

pub fn parse(d: &[u8]) -> Result<(Vec<Chunk>, usize), String> {
    if !d.starts_with(SIG) {
        return Err("not a PNG".into());
    }
    let mut pos = SIG.len();
    let mut chunks = Vec::new();
    loop {
        if pos + 12 > d.len() {
            return Err("truncated".into());
        }
        let len = u32::from_be_bytes(d[pos..pos + 4].try_into().unwrap()) as usize;
        let kind: [u8; 4] = d[pos + 4..pos + 8].try_into().unwrap();
        let end = pos + 8 + len + 4;
        if end > d.len() {
            return Err("truncated chunk".into());
        }
        chunks.push(Chunk { kind, data: d[pos + 8..pos + 8 + len].to_vec() });
        pos = end;
        if &kind == b"IEND" {
            return Ok((chunks, d.len() - pos));
        }
    }
}

pub fn chunk_bytes(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend(kind);
    out.extend(data);
    let mut h = crc32fast::Hasher::new();
    h.update(kind);
    h.update(data);
    out.extend(h.finalize().to_be_bytes());
    out
}

/// Chunks needed to show the picture as before.
const KEEP: [&[u8; 4]; 17] = [
    b"IHDR", b"PLTE", b"IDAT", b"IEND", b"tRNS", b"gAMA", b"cHRM", b"sRGB", b"iCCP", b"cICP",
    b"mDCV", b"cLLI", b"sBIT", b"bKGD", b"acTL", b"fcTL", b"fdAT",
];

pub fn strip(d: &[u8]) -> Result<Vec<u8>, String> {
    let (chunks, _) = parse(d)?;
    let mut out = SIG.to_vec();
    for c in chunks {
        if &c.kind == b"eXIf" {
            if let Some(o) = crate::tiff::orientation(&c.data).filter(|&o| o != 1) {
                out.extend(chunk_bytes(b"eXIf", &crate::tiff::orientation_only(o)));
            }
        } else if KEEP.contains(&&c.kind) {
            out.extend(chunk_bytes(&c.kind, &c.data));
        }
    }
    Ok(out)
}

pub fn describe(d: &[u8]) -> Result<Vec<String>, String> {
    let (chunks, trailer) = parse(d)?;
    let mut found = Vec::new();
    for c in &chunks {
        match &c.kind {
            b"eXIf" => found.extend(crate::tiff::categories(&c.data)),
            b"iTXt" if c.data.starts_with(b"XML:com.adobe.xmp\0") => found.push("XMP".into()),
            b"tEXt" | b"zTXt" | b"iTXt" => {
                let key = c.data.split(|&b| b == 0).next().unwrap_or_default();
                found.push(format!("text:{}", String::from_utf8_lossy(key)));
            }
            b"tIME" => found.push("date".into()),
            b"iCCP" => found.push("ICC(kept)".into()),
            k if !KEEP.contains(&k) => found.push(format!("chunk {}", String::from_utf8_lossy(k))),
            _ => {}
        }
    }
    if trailer > 0 {
        found.push(format!("{trailer} bytes after IEND"));
    }
    Ok(found)
}

pub fn image_chunks(d: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    Ok(parse(d)?
        .0
        .into_iter()
        .filter(|c| KEEP.contains(&&c.kind))
        .map(|c| chunk_bytes(&c.kind, &c.data))
        .collect())
}
