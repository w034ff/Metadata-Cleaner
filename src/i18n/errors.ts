import type { ErrorCode } from "../ipc";
import type { Language } from "./types";

export const ERROR_MESSAGES_JA: Record<ErrorCode, string> = {
  UnsupportedFormat: "対応していない形式です",
  DecodeFailed:
    "画像を読み込めませんでした。ファイルが壊れている可能性があります",
  PdfOpenFailed:
    "PDF を読み込めませんでした。ファイルが壊れている可能性があります",
  PdfEncrypted: "暗号化された PDF は扱えません",
  PdfSigned: "電子署名付きの PDF は扱えません",
  TooLarge: "ファイルが大きすぎます（上限 {detail}）",
  WorkerCrashed:
    "PDF の処理中に問題が起きました。ファイルが壊れている可能性があります",
  WorkerTimeout: "PDF の処理に時間がかかりすぎたため中止しました",
  VerifyFailed: "情報を消しきれなかったため、保存しませんでした",
  SameFolderAsSource:
    "元のファイルと同じフォルダには保存できません。別のフォルダを選んでください",
  OutputDirMissing:
    "保存先のフォルダが見つかりません。フォルダを選び直してください",
  OutputDirNotWritable: "保存先のフォルダに書き込めません",
  ReadFailed: "ファイルの読み込みに失敗しました",
  WriteFailed: "ファイルの書き込みに失敗しました",
  JobRunning: "処理中は操作できません",
  UnknownHandle: "ファイルをもう一度追加してください",
  InvalidParams: "無効な設定です",
};

export const ERROR_MESSAGES_EN: Record<ErrorCode, string> = {
  UnsupportedFormat: "Unsupported file format",
  DecodeFailed: "Couldn't read the image. The file may be damaged.",
  PdfOpenFailed: "Couldn't read the PDF. The file may be damaged.",
  PdfEncrypted: "Encrypted PDFs aren't supported",
  PdfSigned: "Signed PDFs aren't supported",
  TooLarge: "File is too large (limit: {detail})",
  WorkerCrashed:
    "Something went wrong while processing the PDF. The file may be damaged.",
  WorkerTimeout: "Processing the PDF took too long and was stopped",
  VerifyFailed: "Couldn't remove all the information, so the file wasn't saved",
  SameFolderAsSource: "Choose a folder other than the one the files are in",
  OutputDirMissing: "The output folder can't be found. Choose a folder again.",
  OutputDirNotWritable: "Can't write to the output folder",
  ReadFailed: "Failed to read file",
  WriteFailed: "Failed to write file",
  JobRunning: "Not available while processing",
  UnknownHandle: "Please add the file again",
  InvalidParams: "Invalid settings",
};

/**
 * Returns the localized error message for an ErrorCode, replacing `{detail}`
 * with the provided detail string if present (design §6.6).
 */
export function formatErrorMessage(
  code: ErrorCode,
  detail: string | null | undefined,
  lang: Language,
): string {
  const table = lang === "ja" ? ERROR_MESSAGES_JA : ERROR_MESSAGES_EN;
  const template = table[code];
  if (
    detail !== null &&
    detail !== undefined &&
    template.includes("{detail}")
  ) {
    return template.replaceAll("{detail}", detail);
  }
  return template;
}
