import { describe, expect, it } from "vitest";
import { formatMessage } from "./format";

describe("formatMessage", () => {
  it("replaces existing placeholders with provided values", () => {
    expect(
      formatMessage("{count} 件から情報を消して保存します", { count: 5 }),
    ).toBe("5 件から情報を消して保存します");
    expect(
      formatMessage("Hello {name}, your score is {score}", {
        name: "Alice",
        score: 100,
      }),
    ).toBe("Hello Alice, your score is 100");
  });

  it("leaves unmatched placeholders untouched", () => {
    expect(
      formatMessage("File is too large (limit: {detail}, max: {max})", {
        detail: "256 MB",
      }),
    ).toBe("File is too large (limit: 256 MB, max: {max})");
  });
});
