import { describe, expect, it } from "vitest";
import { formatFileFormat } from "./fileFormat";

describe("formatFileFormat", () => {
  it("formats known formats to uppercase", () => {
    expect(formatFileFormat("test.jpg", "jpeg")).toBe("JPEG");
    expect(formatFileFormat("photo.png", "png")).toBe("PNG");
    expect(formatFileFormat("image.webp", "webp")).toBe("WebP");
    expect(formatFileFormat("doc.pdf", "pdf")).toBe("PDF");
  });

  it("extracts and uppercases extension when format is null", () => {
    expect(formatFileFormat("契約書-署名済み.pdf", null)).toBe("PDF");
    expect(formatFileFormat("corrupt.jpg", null)).toBe("JPG");
    expect(formatFileFormat("archive.tar.gz", null)).toBe("GZ");
  });

  it("returns dash when format is null and filename has no extension", () => {
    expect(formatFileFormat("noextension", null)).toBe("—");
    expect(formatFileFormat(".hidden", null)).toBe("—");
    expect(formatFileFormat("", null)).toBe("—");
  });
});
