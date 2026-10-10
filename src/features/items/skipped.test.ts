import { describe, expect, it } from "vitest";
import { formatSkippedMessage } from "./skipped";

describe("formatSkippedMessage", () => {
  it("returns null when skipped is null or undefined", () => {
    expect(formatSkippedMessage(null, "ja")).toBeNull();
    expect(formatSkippedMessage(undefined, "ja")).toBeNull();
  });

  it("returns null when total skipped count is 0", () => {
    expect(
      formatSkippedMessage({ folders: 0, unsupported: 0, duplicates: 0 }, "ja"),
    ).toBeNull();
    expect(
      formatSkippedMessage({ folders: 0, unsupported: 0, duplicates: 0 }, "en"),
    ).toBeNull();
  });

  describe("Japanese formatting", () => {
    it("formats single reason: folders only", () => {
      expect(
        formatSkippedMessage(
          { folders: 2, unsupported: 0, duplicates: 0 },
          "ja",
        ),
      ).toBe("（対象外 2 件：サブフォルダ）");
    });

    it("formats single reason: unsupported only", () => {
      expect(
        formatSkippedMessage(
          { folders: 0, unsupported: 3, duplicates: 0 },
          "ja",
        ),
      ).toBe("（対象外 3 件：非対応の形式）");
    });

    it("formats single reason: duplicates only", () => {
      expect(
        formatSkippedMessage(
          { folders: 0, unsupported: 0, duplicates: 1 },
          "ja",
        ),
      ).toBe("（対象外 1 件：重複）");
    });

    it("formats combination of two reasons: folders and unsupported", () => {
      expect(
        formatSkippedMessage(
          { folders: 1, unsupported: 2, duplicates: 0 },
          "ja",
        ),
      ).toBe("（対象外 3 件：サブフォルダ、非対応の形式）");
    });

    it("formats combination of two reasons: folders and duplicates", () => {
      expect(
        formatSkippedMessage(
          { folders: 2, unsupported: 0, duplicates: 3 },
          "ja",
        ),
      ).toBe("（対象外 5 件：サブフォルダ、重複）");
    });

    it("formats combination of two reasons: unsupported and duplicates", () => {
      expect(
        formatSkippedMessage(
          { folders: 0, unsupported: 1, duplicates: 1 },
          "ja",
        ),
      ).toBe("（対象外 2 件：非対応の形式、重複）");
    });

    it("formats combination of all three reasons in correct order", () => {
      expect(
        formatSkippedMessage(
          { folders: 1, unsupported: 2, duplicates: 3 },
          "ja",
        ),
      ).toBe("（対象外 6 件：サブフォルダ、非対応の形式、重複）");
    });
  });

  describe("English formatting", () => {
    it("formats combination of all three reasons", () => {
      expect(
        formatSkippedMessage(
          { folders: 1, unsupported: 2, duplicates: 3 },
          "en",
        ),
      ).toBe(
        "({count} excluded: subfolders, unsupported format, duplicates)".replace(
          "{count}",
          "6",
        ),
      );
    });

    it("formats single reason in English", () => {
      expect(
        formatSkippedMessage(
          { folders: 1, unsupported: 0, duplicates: 0 },
          "en",
        ),
      ).toBe("(1 excluded: subfolders)");
    });
  });
});
