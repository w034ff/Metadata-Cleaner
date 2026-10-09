import { describe, expect, it } from "vitest";
import { en, ja } from "../../i18n";
import type { KeptInfo } from "../../ipc";
import {
  convertToDpi,
  formatColorProfile,
  formatKeptInfo,
  formatOrientation,
  formatResolution,
} from "./formatKeptInfo";

describe("formatKeptInfo", () => {
  describe("convertToDpi", () => {
    it("preserves inch unit values", () => {
      expect(convertToDpi(300, "inch")).toBe(300);
      expect(convertToDpi(72, "inch")).toBe(72);
    });

    it("converts centimeter to dpi (x 2.54, rounded)", () => {
      expect(convertToDpi(118.11, "centimeter")).toBe(300);
      expect(convertToDpi(28.35, "centimeter")).toBe(72);
    });

    it("converts meter to dpi (x 0.0254, rounded)", () => {
      expect(convertToDpi(11811, "meter")).toBe(300);
      expect(convertToDpi(2835, "meter")).toBe(72);
    });
  });

  describe("formatOrientation", () => {
    it("formats orientations 2 to 8 in Japanese", () => {
      expect(formatOrientation(2, ja)).toBe("向き（左右反転）");
      expect(formatOrientation(3, ja)).toBe("向き（180 度回転）");
      expect(formatOrientation(4, ja)).toBe("向き（上下反転）");
      expect(formatOrientation(5, ja)).toBe(
        "向き（左右反転して左に 90 度回転）",
      );
      expect(formatOrientation(6, ja)).toBe("向き（右に 90 度回転）");
      expect(formatOrientation(7, ja)).toBe(
        "向き（左右反転して右に 90 度回転）",
      );
      expect(formatOrientation(8, ja)).toBe("向き（左に 90 度回転）");
    });

    it("formats orientations 2 to 8 in English", () => {
      expect(formatOrientation(2, en)).toBe(
        "Orientation (flipped horizontally)",
      );
      expect(formatOrientation(3, en)).toBe("Orientation (rotated 180°)");
      expect(formatOrientation(4, en)).toBe("Orientation (flipped vertically)");
      expect(formatOrientation(5, en)).toBe(
        "Orientation (flipped and rotated 90° left)",
      );
      expect(formatOrientation(6, en)).toBe("Orientation (rotated 90° right)");
      expect(formatOrientation(7, en)).toBe(
        "Orientation (flipped and rotated 90° right)",
      );
      expect(formatOrientation(8, en)).toBe("Orientation (rotated 90° left)");
    });

    it("returns null for orientation 1 (not kept)", () => {
      expect(formatOrientation(1, ja)).toBeNull();
      expect(formatOrientation(1, en)).toBeNull();
    });
  });

  describe("formatColorProfile", () => {
    it("formats with description when present", () => {
      expect(formatColorProfile("sRGB", ja)).toBe("色のプロファイル（sRGB）");
      expect(formatColorProfile("Display P3", en)).toBe(
        "Color profile (Display P3)",
      );
    });

    it("formats without description when null or empty", () => {
      expect(formatColorProfile(null, ja)).toBe("色のプロファイル");
      expect(formatColorProfile("", ja)).toBe("色のプロファイル");
      expect(formatColorProfile(null, en)).toBe("Color profile");
      expect(formatColorProfile("   ", en)).toBe("Color profile");
    });
  });

  describe("formatResolution", () => {
    it("formats single dpi when x equals y", () => {
      expect(formatResolution(300, 300, "inch", ja)).toBe("解像度（300 dpi）");
      expect(formatResolution(300, 300, "inch", en)).toBe(
        "Resolution (300 dpi)",
      );
    });

    it("formats x * y dpi when x does not equal y", () => {
      expect(formatResolution(300, 600, "inch", ja)).toBe(
        "解像度（300 × 600 dpi）",
      );
      expect(formatResolution(300, 600, "inch", en)).toBe(
        "Resolution (300 × 600 dpi)",
      );
    });

    it("converts centimeter and meter units", () => {
      expect(formatResolution(118.11, 118.11, "centimeter", ja)).toBe(
        "解像度（300 dpi）",
      );
      expect(formatResolution(11811, 23622, "meter", en)).toBe(
        "Resolution (300 × 600 dpi)",
      );
    });
  });

  describe("formatKeptInfo (list)", () => {
    it("formats multiple items joined with localized separator", () => {
      const items: KeptInfo[] = [
        { type: "orientation", value: 6 },
        { type: "colorProfile", description: "sRGB" },
      ];
      expect(formatKeptInfo(items, ja)).toBe(
        "向き（右に 90 度回転）、色のプロファイル（sRGB）",
      );
      expect(formatKeptInfo(items, en)).toBe(
        "Orientation (rotated 90° right), Color profile (sRGB)",
      );
    });

    it("returns empty string when kept is empty", () => {
      expect(formatKeptInfo([], ja)).toBe("");
      expect(formatKeptInfo([], en)).toBe("");
    });
  });
});
