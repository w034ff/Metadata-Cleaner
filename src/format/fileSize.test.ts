import { describe, expect, it } from "vitest";
import { formatFileSize } from "./fileSize";

describe("formatFileSize", () => {
  it("formats bytes under 1 KB as bytes", () => {
    expect(formatFileSize(0)).toBe("0 B");
    expect(formatFileSize(512)).toBe("512 B");
    expect(formatFileSize(1023)).toBe("1023 B");
  });

  it("formats kilobytes with one decimal digit", () => {
    expect(formatFileSize(1024)).toBe("1.0 KB");
    expect(formatFileSize(88 * 1024)).toBe("88.0 KB");
    expect(formatFileSize(412 * 1024)).toBe("412.0 KB");
  });

  it("formats megabytes with one decimal digit", () => {
    expect(formatFileSize(1024 * 1024)).toBe("1.0 MB");
    expect(formatFileSize(2.9 * 1024 * 1024)).toBe("2.9 MB");
    expect(formatFileSize(3.2 * 1024 * 1024)).toBe("3.2 MB");
  });
});
