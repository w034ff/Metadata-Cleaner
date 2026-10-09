// Generates files used for manual acceptance testing (docs/work-plan.md T16b, docs/manual-test.md):
// 1. too-large.jpg exceeding MAX_IMAGE_FILE_BYTES (crates/core/src/detect.rs)
// 2. too-large.pdf exceeding MAX_PDF_FILE_BYTES (crates/core/src/detect.rs)
// 3. cancel-batch/ with 100 fixture copies to verify cancellation mid-flight
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
const FIXTURES_DIR = join(ROOT, "crates", "core", "tests", "fixtures");

// crates/core/src/detect.rs: MAX_IMAGE_FILE_BYTES (256 MiB)
const MAX_IMAGE_FILE_BYTES = 256 * 1024 * 1024;

// crates/core/src/detect.rs: MAX_PDF_FILE_BYTES (512 MiB)
const MAX_PDF_FILE_BYTES = 512 * 1024 * 1024;

const CANCEL_BATCH_COUNT = 100;

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

function createCancelBatch(batchDir: string, count: number): void {
  if (!existsSync(batchDir)) {
    mkdirSync(batchDir, { recursive: true });
  }

  const fullPdf = join(FIXTURES_DIR, "full.pdf");
  const fullJpg = join(FIXTURES_DIR, "full.jpg");

  if (!existsSync(fullPdf) || !existsSync(fullJpg)) {
    throw new Error(
      `Required fixtures not found in ${FIXTURES_DIR}. Run gen_fixtures first.`,
    );
  }

  const half = Math.floor(count / 2);
  for (let i = 1; i <= half; i++) {
    const padded = String(i).padStart(3, "0");
    copyFileSync(fullPdf, join(batchDir, `cancel_${padded}.pdf`));
  }
  for (let i = half + 1; i <= count; i++) {
    const padded = String(i).padStart(3, "0");
    copyFileSync(fullJpg, join(batchDir, `cancel_${padded}.jpg`));
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

  // 3. cancel-batch/: 100 fixture copies (50 PDF + 50 JPG)
  const cancelBatchDir = join(outputDir, "cancel-batch");
  createCancelBatch(cancelBatchDir, CANCEL_BATCH_COUNT);

  const jpegStat = statSync(tooLargeJpegPath);
  const pdfStat = statSync(tooLargePdfPath);

  console.log(
    `too-large.jpg: ${formatWithCommas(jpegStat.size)} bytes (> MAX_IMAGE_FILE_BYTES: ${formatWithCommas(MAX_IMAGE_FILE_BYTES)})`,
  );
  console.log(
    `too-large.pdf: ${formatWithCommas(pdfStat.size)} bytes (> MAX_PDF_FILE_BYTES: ${formatWithCommas(MAX_PDF_FILE_BYTES)})`,
  );
  console.log(
    `cancel-batch: ${CANCEL_BATCH_COUNT} fixture copies generated in ${cancelBatchDir}`,
  );
}

main();
