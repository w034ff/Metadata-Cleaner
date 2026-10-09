// Verifies that cleaned fixtures contain only allowed metadata according to
// design §4.2–§4.6 and §11.2, using ExifTool to detect any stripped metadata.

import { execFileSync } from "node:child_process";
import { readdirSync } from "node:fs";
import path from "node:path";

const MAX_EXIFTOOL_BUFFER_BYTES = 64 * 1024 * 1024;

// Allowed metadata groups and tags according to design §4.2–§4.6:
//
// 1. Whole groups allowed:
//    - ExifTool: ExifTool version and tool metadata (ExifTool:Warning and
//      ExifTool:Error are rejected separately per T15 specification §2).
//    - System: Filesystem attributes (file name, size, modification date).
//    - Composite: Derived values computed by ExifTool (aspect ratio, megapixels).
//    - ICC_Profile, ICC-header, ICC-view, ICC-meas (ICC-*): Color profiles
//      preserved per design §4.2 (JPEG), §4.3 (PNG), §4.4 (WebP).
//
// 2. Groups with restricted tags:
//    - File: FileType, FileTypeExtension, MIMEType, ExifByteOrder, ImageWidth,
//      ImageHeight, EncodingProcess, BitsPerSample, ColorComponents,
//      YCbCrSubSampling (basic file info; JPEG COM appears as File:Comment so
//      the whole group cannot be allowed).
//    - IFD0: Orientation, XResolution, YResolution, ResolutionUnit (design §4.5:
//      EXIF with only kept information; no GPS, dates, devices, or authors).
//    - JFIF: JFIFVersion, ResolutionUnit, XResolution, YResolution (design §4.2:
//      JFIF density preserved; thumbnails are stripped).
//    - Adobe: DCTEncodeVersion, APP14Flags0, APP14Flags1, ColorTransform (design §4.2:
//      APP14 color transform preserved for CMYK).
//    - PNG: ImageWidth, ImageHeight, BitDepth, ColorType, Compression, Filter,
//      Interlace (IHDR), AnimationFrames, AnimationPlays (acTL), Palette (PLTE),
//      BackgroundColor (bKGD), Gamma (gAMA), ProfileName (iCCP), SignificantBits (sBIT),
//      SRGBRendering (sRGB), Transparency (tRNS), WhitePointX, WhitePointY, RedX,
//      RedY, GreenX, GreenY, BlueX, BlueY (cHRM) (design §4.3: basic image, animation,
//      and color chunks; text chunks, tIME, and eXIf are stripped).
//    - PNG-pHYs: PixelsPerUnitX, PixelsPerUnitY, PixelUnits (design §4.3: pHYs pixel density).
//    - RIFF: ImageWidth, ImageHeight (VP8, VP8L, VP8X), VP8Version, HorizontalScale,
//      VerticalScale (VP8), WebP_Flags (VP8X), AlphaPreprocessing, AlphaFiltering,
//      AlphaCompression (ALPH), BackgroundColor, AnimationLoopCount (ANIM), Duration (ANMF)
//      (design §4.4: basic WebP, alpha, and animation chunks; EXIF and XMP are stripped).
//    - PDF: PDFVersion, Linearized, PageCount (design §4.6: basic PDF structure;
//      /Info and /Metadata are stripped).
const ALLOWED: {
  readonly wholeGroups: readonly string[];
  readonly wholeGroupPrefixes: readonly string[];
  readonly restrictedGroups: Readonly<Record<string, readonly string[]>>;
} = {
  wholeGroups: ["ExifTool", "System", "Composite", "ICC_Profile"],
  wholeGroupPrefixes: ["ICC-"],
  restrictedGroups: {
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
    IFD0: ["Orientation", "XResolution", "YResolution", "ResolutionUnit"],
    JFIF: ["JFIFVersion", "ResolutionUnit", "XResolution", "YResolution"],
    Adobe: ["DCTEncodeVersion", "APP14Flags0", "APP14Flags1", "ColorTransform"],
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
    ],
    "PNG-pHYs": ["PixelsPerUnitX", "PixelsPerUnitY", "PixelUnits"],
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
