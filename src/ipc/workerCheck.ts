import { invoke } from "@tauri-apps/api/core";

/** Answer of the `check_worker` command (src-tauri `commands::WorkerCheck`). */
export type WorkerCheck = {
  workerVersion: string | null;
  error: string | null;
};

function isStringOrNull(value: unknown): value is string | null {
  return typeof value === "string" || value === null;
}

function isWorkerCheck(value: unknown): value is WorkerCheck {
  if (typeof value !== "object" || value === null) return false;
  const v: Record<string, unknown> = { ...value };
  return isStringOrNull(v.workerVersion) && isStringOrNull(v.error);
}

/** Starts a PDF worker in Rust and reports whether it answered. */
export async function checkWorker(): Promise<WorkerCheck> {
  const value: unknown = await invoke("check_worker");
  if (!isWorkerCheck(value)) {
    throw new Error("check_worker returned an unexpected value");
  }
  return value;
}
