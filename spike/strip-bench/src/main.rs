//! Spike: remove metadata from JPEG / PNG / WebP / PDF without touching the
//! picture, then check what is left and that the picture is unchanged.
//!
//! Usage:
//!   strip-bench images <in-imgs-dir> <out-dir>   build image fixtures, strip, check
//!   strip-bench pdf <base.pdf> <out-dir>         build a PDF fixture, strip, check
//!   strip-bench pdfs <dir>                       strip every PDF in a folder, check rendering
//! PDF rendering needs PDFIUM_LIB_DIR.

mod jpeg;
mod pdf;
mod png;
mod tiff;
mod webp;

use std::io::Cursor;
use std::path::Path;
use std::time::Instant;

use image::ImageDecoder;

const ICC_FAKE: &[u8] = b"spike-icc-profile-bytes";
const NEEDLES: [&[u8]; 5] = [b"Spike Author", b"Spike Editor", b"SpikePrivate", b"SpikeMake", b"Exif\0\0"];

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a[1].as_str() {
        "images" => images(Path::new(&a[2]), Path::new(&a[3])),
        "pdf" => pdf_fixture(Path::new(&a[2]), Path::new(&a[3])),
        "pdfs" => pdfs(Path::new(&a[2])),
        "one" => one(Path::new(&a[2])),
        "mutate" => mutate(Path::new(&a[2]), Path::new(&a[3])),
        "deep" => deep(a[2].parse().unwrap()),
        "big-jpeg" => big_jpeg(),
        other => panic!("unknown command {other}"),
    }
}

fn thumbnail() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(8, 8, image::Rgb([200, 30, 30]));
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Jpeg).unwrap();
    out
}

fn decode(d: &[u8]) -> (Vec<u8>, image::metadata::Orientation) {
    let mut dec = image::ImageReader::new(Cursor::new(d)).with_guessed_format().unwrap().into_decoder().unwrap();
    let o = dec.orientation().unwrap();
    (image::DynamicImage::from_decoder(dec).unwrap().to_rgba8().into_raw(), o)
}

fn report(name: &str, before: &[u8], after: &[u8], describe: fn(&[u8]) -> Result<Vec<String>, String>, same: bool, ms: f64) {
    let (px0, o0) = decode(before);
    let (px1, o1) = decode(after);
    println!("== {name} ({} -> {} bytes, {ms:.2} ms)", before.len(), after.len());
    println!("  before: {:?}", describe(before).unwrap());
    println!("  after:  {:?}", describe(after).unwrap());
    println!("  image data byte-identical: {same}");
    println!("  pixels identical: {}, orientation {o0:?} -> {o1:?}", px0 == px1);
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let t = Instant::now();
    let r = f();
    (r, t.elapsed().as_secs_f64() * 1000.0)
}

fn jpeg_fixture(base: &[u8], orientation: u16) -> Vec<u8> {
    let parsed = jpeg::parse(base).unwrap();
    let mut out = vec![0xFF, 0xD8];
    out.extend(jpeg::app(0xE1, jpeg::EXIF_PREFIX, &tiff::full(orientation, &thumbnail())));
    out.extend(jpeg::app(0xE1, jpeg::XMP_PREFIX, b"<x:xmpmeta><dc:creator>Spike Author</dc:creator></x:xmpmeta>"));
    out.extend(jpeg::app(0xE2, jpeg::ICC_PREFIX, &[&[1u8, 1][..], ICC_FAKE].concat()));
    out.extend(jpeg::app(0xE2, b"MPF\0", b"spike-mpf"));
    out.extend(jpeg::app(0xED, b"Photoshop 3.0\0", b"8BIM\x04\x04\0\0\0\0\0\x10\x1c\x02\x50\0\x0cSpike Byline"));
    out.extend(jpeg::app(0xFE, b"", b"Spike comment"));
    for s in &parsed.segments {
        if !jpeg::is_metadata(s) || matches!(s.marker, 0xE0 | 0xEE) {
            out.extend(&s.bytes);
        }
    }
    out.extend([0xFF, 0xD9]);
    out.extend(b"SPIKE-TRAILER motion photo video would be here");
    out
}

fn images(dir: &Path, out_dir: &Path) {
    std::fs::create_dir_all(out_dir).unwrap();
    let read = |n: &str| std::fs::read(dir.join(n)).unwrap();
    let write = |n: &str, d: &[u8]| std::fs::write(out_dir.join(n), d).unwrap();

    // JPEG: one upright, one that needs rotating, and the CMYK one with its
    // Adobe segment.
    for (name, base, o) in [("meta.jpg", read("color.jpg"), 1), ("meta6.jpg", read("color.jpg"), 6), ("meta-cmyk.jpg", read("cmyk.jpg"), 1)] {
        let fixture = jpeg_fixture(&base, o);
        let (clean, ms) = timed(|| jpeg::strip(&fixture, true).unwrap());
        write(name, &fixture);
        write(&format!("clean-{name}"), &clean);
        let same = jpeg::image_segments(&fixture).unwrap() == jpeg::image_segments(&clean).unwrap();
        report(name, &fixture, &clean, jpeg::describe, same, ms);
    }

    // PNG: text before and after IDAT, eXIf with GPS, tIME, bytes after IEND.
    let base = read("alpha.png");
    let (chunks, _) = png::parse(&base).unwrap();
    let mut fixture = png::SIG.to_vec();
    for c in &chunks {
        if &c.kind == b"IDAT" && !fixture.windows(4).any(|w| w == b"tEXt") {
            fixture.extend(png::chunk_bytes(b"tEXt", b"Author\0Spike Author"));
            fixture.extend(png::chunk_bytes(b"iTXt", b"XML:com.adobe.xmp\0\0\0\0\0<x:xmpmeta>Spike Author</x:xmpmeta>"));
            fixture.extend(png::chunk_bytes(b"eXIf", &tiff::full(6, &thumbnail())));
            fixture.extend(png::chunk_bytes(b"tIME", &[0x07, 0xEA, 1, 2, 3, 4, 5]));
        }
        if &c.kind == b"IEND" {
            fixture.extend(png::chunk_bytes(b"tEXt", b"Comment\0Spike comment after IDAT"));
        }
        fixture.extend(png::chunk_bytes(&c.kind, &c.data));
    }
    fixture.extend(b"SPIKE-TRAILER");
    let (clean, ms) = timed(|| png::strip(&fixture).unwrap());
    write("meta.png", &fixture);
    write("clean-meta.png", &clean);
    let same = png::image_chunks(&fixture).unwrap() == png::image_chunks(&clean).unwrap();
    report("meta.png", &fixture, &clean, png::describe, same, ms);

    // WebP: EXIF and XMP chunks with the VP8X flags set.
    let base = read("alpha.webp");
    let (chunks, _) = webp::parse(&base).unwrap();
    let mut out_chunks = Vec::new();
    if !chunks.iter().any(|c| &c.id == b"VP8X") {
        let (w, h) = image::load_from_memory(&base).map(|i| (i.width(), i.height())).unwrap();
        let mut x = vec![0x10 | 0x08 | 0x04, 0, 0, 0];
        x.extend(&(w - 1).to_le_bytes()[..3]);
        x.extend(&(h - 1).to_le_bytes()[..3]);
        out_chunks.push(webp::Chunk { id: *b"VP8X", data: x });
    }
    for c in chunks {
        let mut c = c;
        if &c.id == b"VP8X" {
            c.data[0] |= 0x08 | 0x04;
        }
        out_chunks.push(c);
    }
    out_chunks.push(webp::Chunk { id: *b"EXIF", data: tiff::full(3, &thumbnail()) });
    out_chunks.push(webp::Chunk { id: *b"XMP ", data: b"<x:xmpmeta>Spike Author</x:xmpmeta>".to_vec() });
    let fixture = webp::build(&out_chunks);
    let (clean, ms) = timed(|| webp::strip(&fixture).unwrap());
    write("meta.webp", &fixture);
    write("clean-meta.webp", &clean);
    let same = webp::image_chunks(&fixture).unwrap() == webp::image_chunks(&clean).unwrap();
    report("meta.webp", &fixture, &clean, webp::describe, same, ms);
}

fn pdfium() -> pdfium_render::prelude::Pdfium {
    use pdfium_render::prelude::*;
    let lib = std::env::var("PDFIUM_LIB_DIR").expect("PDFIUM_LIB_DIR");
    Pdfium::new(Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&lib)).unwrap())
}

const RENDER_SCALE: f32 = 0.5;
const MAX_RENDERED_PAGES: usize = 30;

/// Pages compared and pages that differ, or why rendering failed.
fn same_rendering(p: &pdfium_render::prelude::Pdfium, a: &[u8], b: &[u8]) -> Result<(usize, usize), String> {
    use pdfium_render::prelude::*;
    let da = p.load_pdf_from_byte_slice(a, None).map_err(|e| format!("open before: {e:?}"))?;
    let db = p.load_pdf_from_byte_slice(b, None).map_err(|e| format!("open after: {e:?}"))?;
    if da.pages().len() != db.pages().len() {
        return Err(format!("page count {} -> {}", da.pages().len(), db.pages().len()));
    }
    let cfg = PdfRenderConfig::new().scale_page_by_factor(RENDER_SCALE);
    let mut diff = 0;
    let n = (da.pages().len() as usize).min(MAX_RENDERED_PAGES);
    for i in 0..n {
        let ra = da.pages().get(i as i32).unwrap().render_with_config(&cfg).unwrap().as_raw_bytes();
        let rb = db.pages().get(i as i32).unwrap().render_with_config(&cfg).unwrap().as_raw_bytes();
        if ra != rb {
            diff += 1;
        }
    }
    Ok((n, diff))
}

fn pdf_fixture(base: &Path, out_dir: &Path) {
    std::fs::create_dir_all(out_dir).unwrap();
    let jpeg = jpeg_fixture(&std::fs::read("../../../../pdf-converter/task/spike/imgs/color.jpg").unwrap_or_else(|_| panic!("run from spike/strip-bench")), 1);
    let dims = image::load_from_memory(&jpeg).map(|i| (i.width(), i.height())).unwrap();
    let (first, second) = pdf::fixture(&std::fs::read(base).unwrap(), &jpeg, dims).unwrap();
    std::fs::write(out_dir.join("meta-1rev.pdf"), &first).unwrap();
    std::fs::write(out_dir.join("meta.pdf"), &second).unwrap();
    println!("fixture leftovers: {:?}", pdf::leftovers(&second, &NEEDLES).unwrap());
    let ((clean, stats), ms) = timed(|| pdf::strip(&second).unwrap());
    std::fs::write(out_dir.join("clean-meta.pdf"), &clean).unwrap();
    println!("strip {ms:.1} ms, {} -> {} bytes, {stats:?}", second.len(), clean.len());
    println!("after leftovers: {:?}", pdf::leftovers(&clean, &NEEDLES).unwrap());
    println!("rendering (pages, differing): {:?}", same_rendering(&pdfium(), &second, &clean));
}

fn pdfs(dir: &Path) {
    let p = pdfium();
    let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "pdf")).collect();
    files.sort();
    for f in files {
        let d = std::fs::read(&f).unwrap();
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let (r, ms) = timed(|| std::panic::catch_unwind(|| pdf::strip(&d)));
        match r {
            Err(_) => println!("{name}: PANIC ({ms:.0} ms)"),
            Ok(Err(e)) => println!("{name}: error {e} ({ms:.0} ms)"),
            Ok(Ok((clean, stats))) => {
                if let Ok(out) = std::env::var("SPIKE_OUT") {
                    std::fs::create_dir_all(&out).unwrap();
                    std::fs::write(Path::new(&out).join(&name), &clean).unwrap();
                }
                let left = pdf::leftovers(&clean, &[]).unwrap_or_else(|e| vec![format!("reload: {e}")]);
                println!(
                    "{name}: {ms:.0} ms, {} -> {} bytes, {stats:?}, left {left:?}, render {:?}",
                    d.len(),
                    clean.len(),
                    same_rendering(&p, &d, &clean)
                );
            }
        }
    }
}

/// Strip one PDF; run in a child process so aborts and hangs show up as an
/// exit status.
fn one(f: &Path) {
    let d = std::fs::read(f).unwrap();
    match pdf::strip(&d) {
        Ok((c, _)) => println!("ok {}", c.len()),
        Err(e) => println!("err {e}"),
    }
}

/// Write damaged copies of a PDF: truncated, and with bytes overwritten.
fn mutate(f: &Path, out_dir: &Path) {
    std::fs::create_dir_all(out_dir).unwrap();
    let d = std::fs::read(f).unwrap();
    let stem = f.file_stem().unwrap().to_string_lossy().to_string();
    let mut seed: u64 = 0x9E3779B97F4A7C15;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for (i, frac) in [10, 50, 90].iter().enumerate() {
        std::fs::write(out_dir.join(format!("{stem}-cut{i}.pdf")), &d[..d.len() * frac / 100]).unwrap();
    }
    for i in 0..20 {
        let mut m = d.clone();
        for _ in 0..(1 + i * 3) {
            let at = (rnd() as usize) % m.len();
            m[at] = rnd() as u8;
        }
        std::fs::write(out_dir.join(format!("{stem}-flip{i:02}.pdf")), &m).unwrap();
    }
}

/// A PDF whose catalog holds arrays nested `depth` deep.
fn deep(depth: usize) {
    let nested = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
    let body = format!(
        "%PDF-1.4\n1 0 obj << /Type /Catalog /Pages 2 0 R /X {nested} >> endobj\n2 0 obj << /Type /Pages /Kids [] /Count 0 >> endobj\ntrailer << /Root 1 0 R >>\n%%EOF\n"
    );
    let r = std::panic::catch_unwind(|| pdf::strip(body.as_bytes()).map(|(c, _)| c.len()));
    println!("depth {depth}: {r:?}");
}

const BIG_W: u32 = 4000;
const BIG_H: u32 = 3000;

fn big_jpeg() {
    let img = image::RgbImage::from_fn(BIG_W, BIG_H, |x, y| image::Rgb([(x % 251) as u8, (y % 241) as u8, ((x ^ y) % 239) as u8]));
    let mut base = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut base, 92).encode_image(&img).unwrap();
    let fixture = jpeg_fixture(&base, 6);
    let (clean, ms) = timed(|| jpeg::strip(&fixture, true).unwrap());
    let (_, check_ms) = timed(|| jpeg::describe(&clean).unwrap());
    println!("{}x{} JPEG {} bytes: strip {ms:.1} ms, re-check {check_ms:.1} ms, {} bytes out", BIG_W, BIG_H, fixture.len(), clean.len());
}
