import type { Format } from "../ipc";

const NO_EXTENSION = "—";

/**
 * Formats the format string in uppercase (JPEG, PNG, WebP, PDF),
 * falling back to the filename extension in uppercase if format is null,
 * or "—" if there is no extension (design §10.1, mockup BList.dc.html).
 */
export function formatFileFormat(name: string, format: Format | null): string {
  if (format !== null) {
    switch (format) {
      case "jpeg":
        return "JPEG";
      case "png":
        return "PNG";
      case "webp":
        return "WebP";
      case "pdf":
        return "PDF";
    }
  }
  const lastDot = name.lastIndexOf(".");
  if (lastDot > 0 && lastDot < name.length - 1) {
    return name.slice(lastDot + 1).toUpperCase();
  }
  return NO_EXTENSION;
}
