import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { AboutInfo } from "./generated/AboutInfo";
import type { AddResult } from "./generated/AddResult";
import type { AddSource } from "./generated/AddSource";
import type { Details } from "./generated/Details";
import type { ErrorCode } from "./generated/ErrorCode";
import type { IpcError } from "./generated/IpcError";
import type { ItemsDropped } from "./generated/ItemsDropped";
import type { JobFinishedPayload } from "./generated/JobFinishedPayload";
import type { JobItemPayload } from "./generated/JobItemPayload";
import type { JobProgressPayload } from "./generated/JobProgressPayload";
import type { OutputDirLabel } from "./generated/OutputDirLabel";
import type { Settings } from "./generated/Settings";
import type { SettingsInput } from "./generated/SettingsInput";

export type { AboutInfo } from "./generated/AboutInfo";
export type { AddResult } from "./generated/AddResult";
export type { AddSource } from "./generated/AddSource";
export type { DetailEntry } from "./generated/DetailEntry";
export type { DetailGroup } from "./generated/DetailGroup";
export type { DetailValue } from "./generated/DetailValue";
export type { Details } from "./generated/Details";
export type { ErrorCode } from "./generated/ErrorCode";
export type { Field } from "./generated/Field";
export type { FileItem } from "./generated/FileItem";
export type { Format } from "./generated/Format";
export type { IpcError } from "./generated/IpcError";
export type { ItemsDropped } from "./generated/ItemsDropped";
export type { JobFinishedPayload } from "./generated/JobFinishedPayload";
export type { JobItemPayload } from "./generated/JobItemPayload";
export type { JobItemStatus } from "./generated/JobItemStatus";
export type { JobProgressPayload } from "./generated/JobProgressPayload";
export type { KeptInfo } from "./generated/KeptInfo";
export type { Language } from "./generated/Language";
export type { MetadataKind } from "./generated/MetadataKind";
export type { OutputDirLabel } from "./generated/OutputDirLabel";
export type { ResolutionUnit } from "./generated/ResolutionUnit";
export type { Settings } from "./generated/Settings";
export type { SettingsInput } from "./generated/SettingsInput";
export type { Skipped } from "./generated/Skipped";
export type { UnlistenFn };

export { FIELDS, METADATA_KINDS } from "./kinds";

/** Name of the event Rust emits after a drop (design §7.2). */
export const ITEMS_DROPPED_EVENT = "items-dropped";

/** Name of the event Rust emits during cleaning progress (design §7.2). */
export const JOB_PROGRESS_EVENT = "job-progress";

/** Name of the event Rust emits when an item completes (design §7.2). */
export const JOB_ITEM_EVENT = "job-item";

/** Name of the event Rust emits when a job finishes (design §7.2). */
export const JOB_FINISHED_EVENT = "job-finished";

const ERROR_CODES = {
  UnsupportedFormat: true,
  DecodeFailed: true,
  PdfOpenFailed: true,
  PdfEncrypted: true,
  PdfSigned: true,
  TooLarge: true,
  WorkerCrashed: true,
  WorkerTimeout: true,
  VerifyFailed: true,
  SameFolderAsSource: true,
  OutputDirMissing: true,
  OutputDirNotWritable: true,
  ReadFailed: true,
  WriteFailed: true,
  JobRunning: true,
  UnknownHandle: true,
  InvalidParams: true,
} satisfies Record<ErrorCode, true>;

/** Whether a value is one of the error codes of design §6.6. */
export function isErrorCode(value: unknown): value is ErrorCode {
  return (
    typeof value === "string" &&
    Object.prototype.hasOwnProperty.call(ERROR_CODES, value)
  );
}

function hasProperty<K extends string>(
  value: object,
  key: K,
): value is Record<K, unknown> {
  return key in value;
}

/** Whether a value has the shape `{ code, detail }` of design §6.6. */
export function isIpcError(value: unknown): value is IpcError {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  if (!hasProperty(value, "code") || !hasProperty(value, "detail")) {
    return false;
  }
  const code = value.code;
  const detail = value.detail;
  return isErrorCode(code) && (typeof detail === "string" || detail === null);
}

/**
 * Turns anything an IPC call rejected with into `{ code, detail }`. Tauri
 * rejects with a plain string when it cannot read the arguments, which design
 * §6.6 reports as `InvalidParams`.
 */
export function normalizeIpcError(error: unknown): IpcError {
  if (isIpcError(error)) {
    return error;
  }
  let detail: string;
  if (typeof error === "string") {
    try {
      const parsed: unknown = JSON.parse(error);
      if (isIpcError(parsed)) {
        return parsed;
      }
    } catch {
      // Ignore JSON parse error, treat as raw string detail.
    }
    detail = error;
  } else if (error instanceof Error) {
    detail = error.message;
  } else if (typeof error === "object" && error !== null) {
    try {
      detail = JSON.stringify(error);
    } catch {
      detail = String(error);
    }
  } else {
    detail = String(error);
  }
  return { code: "InvalidParams", detail };
}

async function invokeWrapped<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (error: unknown) {
    throw normalizeIpcError(error);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function isOutputDirLabel(
  value: unknown,
): value is OutputDirLabel | null {
  return (
    value === null || (isRecord(value) && typeof value.dirLabel === "string")
  );
}

/** Whether a value has the shape of the `get_settings` answer (design §6.7). */
export function isSettings(value: unknown): value is Settings {
  if (!isRecord(value)) {
    return false;
  }
  const { language, outputDir } = value;
  const isLangValid =
    language === "ja" || language === "en" || language === null;
  return isLangValid && isOutputDirLabel(outputDir);
}

/** Whether a value has the shape of the `get_about` answer (design §7.1). */
export function isAboutInfo(value: unknown): value is AboutInfo {
  return isRecord(value) && typeof value.version === "string";
}

/**
 * Returns the app version (design §7.1).
 */
export async function getAbout(): Promise<AboutInfo> {
  const value = await invokeWrapped<unknown>("get_about");
  if (!isAboutInfo(value)) {
    throw normalizeIpcError("get_about returned an unexpected value");
  }
  return value;
}

/**
 * Fetches the settings (design §6.7). Output folder comes as a name, never as a path.
 */
export async function getSettings(): Promise<Settings> {
  const value = await invokeWrapped<unknown>("get_settings");
  if (!isSettings(value)) {
    throw normalizeIpcError("get_settings returned an unexpected value");
  }
  return value;
}

/**
 * Saves the language setting (design §6.7).
 */
export async function saveSettings(input: SettingsInput): Promise<void> {
  return invokeWrapped<void>("save_settings", { input });
}

/**
 * Asks for a folder and makes it the output folder (design §6.7, §7.1).
 * Resolves to `null` if the dialog was cancelled.
 */
export async function pickOutputDir(): Promise<OutputDirLabel | null> {
  return invokeWrapped<OutputDirLabel | null>("pick_output_dir");
}

/**
 * Asks for files or a folder and adds them to the list (design §7.1).
 * Resolves to `null` if the dialog was cancelled.
 */
export async function addFiles(source: AddSource): Promise<AddResult | null> {
  return invokeWrapped<AddResult | null>("add_files", { source });
}

/**
 * Removes items from the list (design §7.1). An ID not in the table is ignored.
 */
export async function removeItems(ids: number[]): Promise<void> {
  return invokeWrapped<void>("remove_items", { ids });
}

/**
 * Gets the metadata details for an item (design §6.2, §7.1).
 */
export async function getDetails(id: number): Promise<Details> {
  return invokeWrapped<Details>("get_details", { id });
}

/**
 * Starts batch cleaning of metadata in the background (design §6.3, §7.1).
 */
export async function startClean(ids: number[]): Promise<void> {
  return invokeWrapped<void>("start_clean", { ids });
}

/**
 * Cancels the currently executing cleaning job (design §6.5, §7.1).
 */
export async function cancelJob(): Promise<void> {
  return invokeWrapped<void>("cancel_job");
}

/**
 * Subscribes to what a drop added to the list (design §7.2).
 */
export async function onItemsDropped(
  handler: (payload: ItemsDropped) => void,
): Promise<UnlistenFn> {
  return listen<ItemsDropped>(ITEMS_DROPPED_EVENT, (event) => {
    handler(event.payload);
  });
}

/**
 * Subscribes to job progress updates (design §7.2).
 */
export async function onJobProgress(
  handler: (payload: JobProgressPayload) => void,
): Promise<UnlistenFn> {
  return listen<JobProgressPayload>(JOB_PROGRESS_EVENT, (event) => {
    handler(event.payload);
  });
}

/**
 * Subscribes to individual item completion events (design §7.2).
 */
export async function onJobItem(
  handler: (payload: JobItemPayload) => void,
): Promise<UnlistenFn> {
  return listen<JobItemPayload>(JOB_ITEM_EVENT, (event) => {
    handler(event.payload);
  });
}

/**
 * Subscribes to job finished events (design §7.2).
 */
export async function onJobFinished(
  handler: (payload: JobFinishedPayload) => void,
): Promise<UnlistenFn> {
  return listen<JobFinishedPayload>(JOB_FINISHED_EVENT, (event) => {
    handler(event.payload);
  });
}
