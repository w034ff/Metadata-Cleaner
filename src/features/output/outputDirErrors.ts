import type { ErrorCode, IpcError } from "../../ipc";

/** The codes of a cleaning job refused because of its output folder (design §6.5, §6.6). */
const OUTPUT_DIR_ERROR_CODES: readonly ErrorCode[] = [
  "SameFolderAsSource",
  "OutputDirMissing",
  "OutputDirNotWritable",
];

/** Whether `error` says the output folder cannot be used for the job. */
export function isOutputDirError(error: IpcError): boolean {
  return OUTPUT_DIR_ERROR_CODES.includes(error.code);
}
