// Generates files used for manual acceptance testing (docs/work-plan.md T16b, docs/manual-test.md):
// 1. too-large.jpg exceeding MAX_IMAGE_FILE_BYTES (crates/core/src/detect.rs)
// 2. too-large.pdf exceeding MAX_PDF_FILE_BYTES (crates/core/src/detect.rs)
// 3. cancel-batch/ with slow PDF copies to verify cancellation mid-flight
//
// Usage: node scripts/make-manual-test-files.ts <output_dir>

import {
  copyFileSync,
  existsSync,
  mkdirSync,
  openSync,
  readFileSync,
  statSync,
  writeSync,
  closeSync,
  ftruncateSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const DETECT_RS = join(ROOT, "crates", "core", "src", "detect.rs");

// crates/core/src/detect.rs: MAX_IMAGE_FILE_BYTES (256 MiB)
const MAX_IMAGE_FILE_BYTES = 256 * 1024 * 1024;

// crates/core/src/detect.rs: MAX_PDF_FILE_BYTES (512 MiB)
const MAX_PDF_FILE_BYTES = 512 * 1024 * 1024;

// Number of indirect objects in the slow PDF.
// 150,000 objects takes ~12-13 seconds for a batch of 4 files on 4 concurrent workers (design §5.2).
// Each file alone takes ~7-8 seconds (well below CLEAN_TIMEOUT of 60 seconds).
const SLOW_PDF_OBJECT_COUNT = 150_000;

// Number of slow PDF copies in cancel-batch/.
const CANCEL_BATCH_COUNT = 4;

function readConstantFromRust(filePath: string, constantName: string): number {
  if (!existsSync(filePath)) {
    throw new Error(`File not found: ${filePath}`);
  }
  const content = readFileSync(filePath, "utf-8");
  // Matches e.g.: pub const MAX_IMAGE_FILE_BYTES: u64 = 256 * 1024 * 1024;
  const pattern = new RegExp(
    `pub\\s+const\\s+${constantName}\\s*:\\s*\\w+\\s*=\\s*([^;]+);`,
  );
  const match = content.match(pattern);
  if (!match || !match[1]) {
    throw new Error(
      `Constant '${constantName}' could not be read from ${filePath}`,
    );
  }
  const expr = match[1].replaceAll("_", "").trim();
  // Safe evaluation of arithmetic expression like "256 * 1024 * 1024"
  const tokens = expr.split("*").map((t) => Number.parseInt(t.trim(), 10));
  if (tokens.some((n) => Number.isNaN(n) || n <= 0)) {
    throw new Error(
      `Failed to parse arithmetic constant '${constantName}' from '${expr}'`,
    );
  }
  return tokens.reduce((acc, n) => acc * n, 1);
}

function formatWithCommas(value: number): string {
  return value.toLocaleString("en-US");
}

function createTooLargeJpeg(filePath: string, size: number): void {
  // Minimal valid JPEG header (SOI + APP0 JFIF marker)
  const header = Buffer.from([
    0xff,
    0xd8, // SOI
    0xff,
    0xe0, // APP0
    0x00,
    0x10, // Length: 16 bytes
    0x4a,
    0x46,
    0x49,
    0x46,
    0x00, // "JFIF\0"
    0x01,
    0x01, // Version 1.1
    0x00, // Density unit: none
    0x00,
    0x01, // Xdensity: 1
    0x00,
    0x01, // Ydensity: 1
    0x00,
    0x00, // Thumbnail: 0 x 0
  ]);

  const fd = openSync(filePath, "w");
  try {
    writeSync(fd, header, 0, header.length, 0);
    ftruncateSync(fd, size);
  } finally {
    closeSync(fd);
  }
}

function createTooLargePdf(filePath: string, size: number): void {
  // Minimal valid PDF header
  const header = Buffer.from("%PDF-1.4\n%\xe2\xe3\xcf\xd3\n", "latin1");

  const fd = openSync(filePath, "w");
  try {
    writeSync(fd, header, 0, header.length, 0);
    ftruncateSync(fd, size);
  } finally {
    closeSync(fd);
  }
}

function createSlowPdf(filePath: string, objectCount: number): void {
  const fd = openSync(filePath, "w");
  let currentOffset = 0;

  const CHUNK_SIZE = 64 * 1024;
  const buffer = Buffer.alloc(CHUNK_SIZE);
  let bufPos = 0;

  function flush(): void {
    if (bufPos > 0) {
      writeSync(fd, buffer, 0, bufPos, currentOffset);
      currentOffset += bufPos;
      bufPos = 0;
    }
  }

  function write(str: string): void {
    const strLen = Buffer.byteLength(str, "latin1");
    if (bufPos + strLen > CHUNK_SIZE) {
      flush();
      if (strLen > CHUNK_SIZE) {
        const bigBuf = Buffer.from(str, "latin1");
        writeSync(fd, bigBuf, 0, bigBuf.length, currentOffset);
        currentOffset += bigBuf.length;
        return;
      }
    }
    buffer.write(str, bufPos, strLen, "latin1");
    bufPos += strLen;
  }

  // 1. Header
  write("%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");

  const offsets: number[] = [];

  // Object 1: Catalog with /SlowList referencing 4..objectCount+3
  offsets[1] = currentOffset + bufPos;
  write("1 0 obj\n<< /Type /Catalog /Pages 2 0 R /SlowList [ ");
  for (let i = 4; i <= objectCount + 3; i++) {
    write(`${i} 0 R `);
  }
  write("] >>\nendobj\n");

  // Object 2: Pages
  offsets[2] = currentOffset + bufPos;
  write("2 0 obj\n<< /Type /Pages /Kids [ 3 0 R ] /Count 1 >>\nendobj\n");

  // Object 3: Page
  offsets[3] = currentOffset + bufPos;
  write(
    "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [ 0 0 612 792 ] >>\nendobj\n",
  );

  // Objects 4 to objectCount + 3
  for (let i = 4; i <= objectCount + 3; i++) {
    offsets[i] = currentOffset + bufPos;
    write(`${i} 0 obj\n<< /Type /Item /Index ${i} >>\nendobj\n`);
  }

  // Xref table
  const totalObjects = objectCount + 4;
  const startXref = currentOffset + bufPos;
  write(`xref\n0 ${totalObjects}\n`);
  write("0000000000 65535 f \r\n");
  for (let i = 1; i < totalObjects; i++) {
    const offStr = String(offsets[i]).padStart(10, "0");
    write(`${offStr} 00000 n \r\n`);
  }

  // Trailer
  write(
    `trailer\n<< /Size ${totalObjects} /Root 1 0 R >>\nstartxref\n${startXref}\n%%EOF\n`,
  );

  flush();
  closeSync(fd);
}

function createCancelBatch(
  batchDir: string,
  count: number,
  objectCount: number,
): void {
  if (!existsSync(batchDir)) {
    mkdirSync(batchDir, { recursive: true });
  }

  const firstPath = join(batchDir, "cancel_001.pdf");
  createSlowPdf(firstPath, objectCount);

  for (let i = 2; i <= count; i++) {
    const padded = String(i).padStart(3, "0");
    copyFileSync(firstPath, join(batchDir, `cancel_${padded}.pdf`));
  }
}

function main(): void {
  const targetArg = process.argv[2];
  if (!targetArg) {
    console.error("Usage: node scripts/make-manual-test-files.ts <output_dir>");
    process.exit(1);
  }

  const outputDir = resolve(process.cwd(), targetArg);
  if (!existsSync(outputDir)) {
    mkdirSync(outputDir, { recursive: true });
  }

  // Verify that the script constants match the Rust constants in detect.rs
  const rustMaxImageBytes = readConstantFromRust(
    DETECT_RS,
    "MAX_IMAGE_FILE_BYTES",
  );
  const rustMaxPdfBytes = readConstantFromRust(DETECT_RS, "MAX_PDF_FILE_BYTES");

  if (rustMaxImageBytes !== MAX_IMAGE_FILE_BYTES) {
    throw new Error(
      `MAX_IMAGE_FILE_BYTES mismatch: script has ${MAX_IMAGE_FILE_BYTES}, detect.rs has ${rustMaxImageBytes}`,
    );
  }
  if (rustMaxPdfBytes !== MAX_PDF_FILE_BYTES) {
    throw new Error(
      `MAX_PDF_FILE_BYTES mismatch: script has ${MAX_PDF_FILE_BYTES}, detect.rs has ${rustMaxPdfBytes}`,
    );
  }

  // 1. too-large.jpg: MAX_IMAGE_FILE_BYTES + 1
  const tooLargeJpegPath = join(outputDir, "too-large.jpg");
  const tooLargeJpegSize = MAX_IMAGE_FILE_BYTES + 1;
  createTooLargeJpeg(tooLargeJpegPath, tooLargeJpegSize);

  // 2. too-large.pdf: MAX_PDF_FILE_BYTES + 1
  const tooLargePdfPath = join(outputDir, "too-large.pdf");
  const tooLargePdfSize = MAX_PDF_FILE_BYTES + 1;
  createTooLargePdf(tooLargePdfPath, tooLargePdfSize);

  // 3. cancel-batch/: slow PDF copies
  const cancelBatchDir = join(outputDir, "cancel-batch");
  createCancelBatch(cancelBatchDir, CANCEL_BATCH_COUNT, SLOW_PDF_OBJECT_COUNT);

  const jpegStat = statSync(tooLargeJpegPath);
  const pdfStat = statSync(tooLargePdfPath);

  console.log(
    `too-large.jpg: ${formatWithCommas(jpegStat.size)} bytes (> MAX_IMAGE_FILE_BYTES: ${formatWithCommas(MAX_IMAGE_FILE_BYTES)})`,
  );
  console.log(
    `too-large.pdf: ${formatWithCommas(pdfStat.size)} bytes (> MAX_PDF_FILE_BYTES: ${formatWithCommas(MAX_PDF_FILE_BYTES)})`,
  );
  console.log(
    `cancel-batch: ${CANCEL_BATCH_COUNT} slow PDF copies (${formatWithCommas(SLOW_PDF_OBJECT_COUNT)} objects each) generated in ${cancelBatchDir}`,
  );
}

main();
