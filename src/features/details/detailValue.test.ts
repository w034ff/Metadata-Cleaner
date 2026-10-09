import { describe, expect, it } from "vitest";
import { en, ja } from "../../i18n";
import { formatDetailValue } from "./detailValue";

describe("formatDetailValue", () => {
  it("formats text values as is", () => {
    expect(
      formatDetailValue({ type: "text", value: "Example Cam X1" }, ja),
    ).toBe("Example Cam X1");
    expect(
      formatDetailValue({ type: "text", value: "2026/01/02 03:04" }, en),
    ).toBe("2026/01/02 03:04");
  });

  it("formats bytes values with localized message", () => {
    expect(formatDetailValue({ type: "bytes", value: 4096 }, ja)).toBe(
      "4096 バイト",
    );
    expect(formatDetailValue({ type: "bytes", value: 4096 }, en)).toBe(
      "4096 bytes",
    );
  });

  it("formats count values with localized message", () => {
    expect(formatDetailValue({ type: "count", value: 2 }, ja)).toBe(
      "2 回分の以前の内容が残っています",
    );
    expect(formatDetailValue({ type: "count", value: 2 }, en)).toBe(
      "2 earlier version(s) remaining",
    );
  });
});
