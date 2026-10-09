import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  addFiles,
  cancelJob,
  FIELDS,
  getAbout,
  getDetails,
  getSettings,
  isAboutInfo,
  isErrorCode,
  isIpcError,
  isOutputDirLabel,
  isSettings,
  METADATA_KINDS,
  normalizeIpcError,
  onItemsDropped,
  onJobFinished,
  onJobItem,
  onJobProgress,
  pickOutputDir,
  removeItems,
  saveSettings,
  startClean,
} from "./index";

afterEach(() => {
  clearMocks();
});

describe("ipc error helpers", () => {
  it("identifies valid error codes", () => {
    expect(isErrorCode("UnsupportedFormat")).toBe(true);
    expect(isErrorCode("SameFolderAsSource")).toBe(true);
    expect(isErrorCode("VerifyFailed")).toBe(true);
    expect(isErrorCode("NotAnErrorCode")).toBe(false);
    expect(isErrorCode(123)).toBe(false);
    expect(isErrorCode(null)).toBe(false);
  });

  it("identifies valid IpcError objects", () => {
    expect(isIpcError({ code: "UnsupportedFormat", detail: null })).toBe(true);
    expect(isIpcError({ code: "TooLarge", detail: "256 MB" })).toBe(true);
    expect(isIpcError({ code: "InvalidCode", detail: null })).toBe(false);
    expect(isIpcError({ code: "TooLarge" })).toBe(false);
    expect(isIpcError(null)).toBe(false);
    expect(isIpcError("string")).toBe(false);
  });

  it("normalizes errors to IpcError", () => {
    const original = { code: "DecodeFailed" as const, detail: null };
    expect(normalizeIpcError(original)).toBe(original);

    expect(normalizeIpcError("something broke")).toEqual({
      code: "InvalidParams",
      detail: "something broke",
    });

    expect(normalizeIpcError(new Error("err msg"))).toEqual({
      code: "InvalidParams",
      detail: "err msg",
    });

    expect(normalizeIpcError({ foo: "bar" })).toEqual({
      code: "InvalidParams",
      detail: JSON.stringify({ foo: "bar" }),
    });
  });
});

describe("ipc shape validators", () => {
  it("validates OutputDirLabel", () => {
    expect(isOutputDirLabel(null)).toBe(true);
    expect(isOutputDirLabel({ dirLabel: "cleaned" })).toBe(true);
    expect(isOutputDirLabel({ dirLabel: 123 })).toBe(false);
    expect(isOutputDirLabel({})).toBe(false);
    expect(isOutputDirLabel("cleaned")).toBe(false);
  });

  it("validates Settings", () => {
    expect(isSettings({ language: "ja", outputDir: null })).toBe(true);
    expect(isSettings({ language: "en", outputDir: { dirLabel: "out" } })).toBe(
      true,
    );
    expect(isSettings({ language: null, outputDir: null })).toBe(true);
    expect(isSettings({ language: "fr", outputDir: null })).toBe(false);
    expect(isSettings(null)).toBe(false);
  });

  it("validates AboutInfo", () => {
    expect(isAboutInfo({ version: "0.1.0" })).toBe(true);
    expect(isAboutInfo({ version: 1 })).toBe(false);
    expect(isAboutInfo({})).toBe(false);
  });
});

describe("kinds and fields runtime arrays", () => {
  it("has non-empty lists for METADATA_KINDS and FIELDS", () => {
    expect(METADATA_KINDS.length).toBe(9);
    expect(FIELDS.length).toBe(25);
    expect(METADATA_KINDS).toContain("location");
    expect(METADATA_KINDS).toContain("history");
    expect(FIELDS).toContain("latitude");
    expect(FIELDS).toContain("earlierVersions");
  });
});

describe("ipc command wrappers", () => {
  it("getAbout returns about info", async () => {
    mockIPC((cmd) => (cmd === "get_about" ? { version: "1.2.3" } : null));
    const result = await getAbout();
    expect(result).toEqual({ version: "1.2.3" });
  });

  it("getSettings returns settings", async () => {
    mockIPC((cmd) =>
      cmd === "get_settings"
        ? { language: "ja", outputDir: { dirLabel: "test" } }
        : null,
    );
    const result = await getSettings();
    expect(result).toEqual({
      language: "ja",
      outputDir: { dirLabel: "test" },
    });
  });

  it("saveSettings invokes save_settings with input", async () => {
    let savedInput: unknown = null;
    mockIPC((cmd, args) => {
      if (
        cmd === "save_settings" &&
        typeof args === "object" &&
        args !== null
      ) {
        savedInput = Reflect.get(args, "input");
        return null;
      }
      return null;
    });
    await saveSettings({ language: "en" });
    expect(savedInput).toEqual({ language: "en" });
  });

  it("pickOutputDir returns label or null", async () => {
    mockIPC((cmd) =>
      cmd === "pick_output_dir" ? { dirLabel: "picked_dir" } : null,
    );
    const result = await pickOutputDir();
    expect(result).toEqual({ dirLabel: "picked_dir" });
  });

  it("addFiles invokes add_files with source", async () => {
    let receivedSource: unknown = null;
    mockIPC((cmd, args) => {
      if (cmd === "add_files" && typeof args === "object" && args !== null) {
        receivedSource = Reflect.get(args, "source");
        return {
          added: [],
          skipped: { unsupported: 0, folders: 0, duplicates: 0 },
        };
      }
      return null;
    });
    const result = await addFiles("files");
    expect(receivedSource).toBe("files");
    expect(result?.added).toEqual([]);
  });

  it("removeItems invokes remove_items with ids", async () => {
    let removedIds: unknown = null;
    mockIPC((cmd, args) => {
      if (cmd === "remove_items" && typeof args === "object" && args !== null) {
        removedIds = Reflect.get(args, "ids");
        return null;
      }
      return null;
    });
    await removeItems([1, 2, 3]);
    expect(removedIds).toEqual([1, 2, 3]);
  });

  it("getDetails invokes get_details with id", async () => {
    mockIPC((cmd) =>
      cmd === "get_details" ? { groups: [], kept: [], truncated: false } : null,
    );
    const details = await getDetails(42);
    expect(details).toEqual({ groups: [], kept: [], truncated: false });
  });

  it("startClean invokes start_clean with ids", async () => {
    let cleanIds: unknown = null;
    mockIPC((cmd, args) => {
      if (cmd === "start_clean" && typeof args === "object" && args !== null) {
        cleanIds = Reflect.get(args, "ids");
        return null;
      }
      return null;
    });
    await startClean([10, 20]);
    expect(cleanIds).toEqual([10, 20]);
  });

  it("cancelJob invokes cancel_job", async () => {
    let called = false;
    mockIPC((cmd) => {
      if (cmd === "cancel_job") {
        called = true;
        return null;
      }
      return null;
    });
    await cancelJob();
    expect(called).toBe(true);
  });

  it("event subscription helpers return unlisten functions and handle events", async () => {
    mockIPC(() => undefined, { shouldMockEvents: true });

    const receivedProgress: unknown[] = [];
    const unlisten1 = await onItemsDropped(vi.fn());
    const unlisten2 = await onJobProgress((p) => receivedProgress.push(p));
    const unlisten3 = await onJobItem(vi.fn());
    const unlisten4 = await onJobFinished(vi.fn());

    expect(typeof unlisten1).toBe("function");
    expect(typeof unlisten2).toBe("function");
    expect(typeof unlisten3).toBe("function");
    expect(typeof unlisten4).toBe("function");

    unlisten1();
    unlisten2();
    unlisten3();
    unlisten4();
  });
});
