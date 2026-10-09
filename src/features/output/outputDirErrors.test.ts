import { describe, expect, it } from "vitest";
import { isOutputDirError } from "./outputDirErrors";

describe("isOutputDirError", () => {
  it("returns true for SameFolderAsSource", () => {
    expect(isOutputDirError({ code: "SameFolderAsSource", detail: null })).toBe(
      true,
    );
  });

  it("returns false for other errors", () => {
    expect(isOutputDirError({ code: "WriteFailed", detail: null })).toBe(false);
    expect(isOutputDirError({ code: "VerifyFailed", detail: null })).toBe(
      false,
    );
    expect(isOutputDirError({ code: "JobRunning", detail: null })).toBe(false);
  });
});
