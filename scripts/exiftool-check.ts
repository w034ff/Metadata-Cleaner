// Verifies that cleaned fixtures contain only allowed metadata according to
// design §4.2–§4.6 and §11.2, using ExifTool to detect any stripped metadata.

import { execFileSync } from "node:child_process";
import { readdirSync } from "node:fs";
import path from "node:path";

const MAX_EXIFTOOL_BUFFER_BYTES = 64 * 1024 * 1024;

// What may remain in a cleaned file: only what design §4.2–§4.6 keeps. Each
// group says which kept item its tags come from.
const ALLOWED: {
  readonly wholeGroups: readonly string[];
  readonly wholeGroupPrefixes: readonly string[];
  readonly restrictedGroups: Readonly<Record<string, readonly string[]>>;
} = {
  // ExifTool's own fields, the file system, values derived from the other
  // groups, and the ICC profile, which is kept byte for byte (§4.2–§4.4).
  // ExifTool:Warning and ExifTool:Error are rejected before this list.
  wholeGroups: ["ExifTool", "System", "Composite", "ICC_Profile"],
  wholeGroupPrefixes: ["ICC-"],
  restrictedGroups: {
    // Basic file properties. JPEG COM shows as File:Comment, so the group
    // is not allowed as a whole.
    File: [
      "FileType",
      "FileTypeExtension",
      "MIMEType",
      "ExifByteOrder",
      "ImageWidth",
      "ImageHeight",
      "EncodingProcess",
      "BitsPerSample",
      "ColorComponents",
      "YCbCrSubSampling",
    ],
    // The EXIF that §4.5 keeps.
    IFD0: ["Orientation", "XResolution", "YResolution", "ResolutionUnit"],
    // JFIF without its thumbnail (§4.2).
    JFIF: ["JFIFVersion", "ResolutionUnit", "XResolution", "YResolution"],
    // APP14 for CMYK and YCCK (§4.2).
    Adobe: ["DCTEncodeVersion", "APP14Flags0", "APP14Flags1", "ColorTransform"],
    // IHDR, acTL, PLTE, bKGD, gAMA, iCCP, sBIT, sRGB, tRNS, cHRM and cICP
    // (§4.3). ExifTool 12.76 has no tags for mDCV and cLLI.
    PNG: [
      "ImageWidth",
      "ImageHeight",
      "BitDepth",
      "ColorType",
      "Compression",
      "Filter",
      "Interlace",
      "AnimationFrames",
      "AnimationPlays",
      "Palette",
      "BackgroundColor",
      "Gamma",
      "ProfileName",
      "SignificantBits",
      "SRGBRendering",
      "Transparency",
      "WhitePointX",
      "WhitePointY",
      "RedX",
      "RedY",
      "GreenX",
      "GreenY",
      "BlueX",
      "BlueY",
      "ColorPrimaries",
      "TransferCharacteristics",
      "MatrixCoefficients",
      "VideoFullRangeFlag",
    ],
    // pHYs (§4.3).
    "PNG-pHYs": ["PixelsPerUnitX", "PixelsPerUnitY", "PixelUnits"],
    // VP8, VP8L, VP8X, ALPH, ANIM and ANMF (§4.4).
    RIFF: [
      "ImageWidth",
      "ImageHeight",
      "VP8Version",
      "HorizontalScale",
      "VerticalScale",
      "WebP_Flags",
      "AlphaPreprocessing",
      "AlphaFiltering",
      "AlphaCompression",
      "BackgroundColor",
      "AnimationLoopCount",
      "Duration",
    ],
    // The file structure only; /Info and /Metadata are removed (§4.6).
    PDF: ["PDFVersion", "Linearized", "PageCount"],
  },
};

type Violation = {
  readonly fileName: string;
  readonly item: string;
  readonly value: string;
  readonly reason: string;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function main(): void {
  const targetDir = process.argv[2];
  if (!targetDir) {
    console.error("Usage: node scripts/exiftool-check.ts <directory>");
    process.exit(1);
  }

  const entries = readdirSync(targetDir, { withFileTypes: true });
  const files = entries
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name)
    .sort();

  if (files.length === 0) {
    console.error(`No files found in directory: ${targetDir}`);
    process.exit(1);
  }

  const filePaths = files.map((file) => path.join(targetDir, file));
  const stdout = execFileSync("exiftool", ["-j", "-a", "-G1", ...filePaths], {
    encoding: "utf8",
    maxBuffer: MAX_EXIFTOOL_BUFFER_BYTES,
  });

  const parsed: unknown = JSON.parse(stdout);
  if (!Array.isArray(parsed)) {
    console.error("Expected ExifTool JSON output to be an array");
    process.exit(1);
  }

  const violations: Violation[] = [];

  for (const item of parsed) {
    if (!isRecord(item)) {
      console.error("Expected ExifTool JSON item to be an object");
      process.exit(1);
    }

    const sourceFileValue = item["SourceFile"];
    const fileName =
      typeof sourceFileValue === "string"
        ? path.basename(sourceFileValue)
        : "unknown";

    for (const key of Object.keys(item)) {
      if (key === "SourceFile") {
        continue;
      }

      const colonIndex = key.indexOf(":");
      if (colonIndex === -1) {
        violations.push({
          fileName,
          item: key,
          value: String(item[key]),
          reason: "Key is missing group prefix",
        });
        continue;
      }

      const group = key.slice(0, colonIndex);
      const tag = key.slice(colonIndex + 1);

      // ExifTool:Warning and ExifTool:Error are always treated as errors.
      if (group === "ExifTool" && (tag === "Warning" || tag === "Error")) {
        violations.push({
          fileName,
          item: key,
          value: String(item[key]),
          reason: `ExifTool reported a ${tag.toLowerCase()}`,
        });
        continue;
      }

      // Check whole groups.
      if (ALLOWED.wholeGroups.includes(group)) {
        continue;
      }

      // Check whole group prefixes (e.g. ICC-*).
      if (
        ALLOWED.wholeGroupPrefixes.some((prefix) => group.startsWith(prefix))
      ) {
        continue;
      }

      // Check restricted groups.
      const allowedTags = ALLOWED.restrictedGroups[group];
      if (allowedTags !== undefined) {
        if (allowedTags.includes(tag)) {
          continue;
        }
        violations.push({
          fileName,
          item: key,
          value: String(item[key]),
          reason: `Tag "${tag}" is not allowed in group "${group}"`,
        });
        continue;
      }

      // Disallowed group.
      violations.push({
        fileName,
        item: key,
        value: String(item[key]),
        reason: `Group "${group}" is not in the allowed list`,
      });
    }
  }

  if (violations.length > 0) {
    console.error(`Found ${violations.length} disallowed metadata item(s):`);
    for (const violation of violations) {
      console.error(
        `  ${violation.fileName}: ${violation.item} = ${violation.value} (${violation.reason})`,
      );
    }
    process.exit(1);
  }

  console.log(
    `Checked ${files.length} file(s) with ExifTool: all metadata matches allowed rules.`,
  );
}

main();
