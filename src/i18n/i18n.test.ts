import { describe, expect, it } from "vitest";
import type { ErrorCode } from "../ipc";
import { FIELDS, METADATA_KINDS } from "../ipc";
import {
  detectInitialLanguage,
  en,
  formatErrorMessage,
  ja,
  type Translations,
} from "./index";

const ALL_ERROR_CODES: ErrorCode[] = [
  "UnsupportedFormat",
  "DecodeFailed",
  "PdfOpenFailed",
  "PdfEncrypted",
  "PdfSigned",
  "TooLarge",
  "WorkerCrashed",
  "WorkerTimeout",
  "VerifyFailed",
  "SameFolderAsSource",
  "ReadFailed",
  "WriteFailed",
  "JobRunning",
  "UnknownHandle",
  "InvalidParams",
];

describe("i18n", () => {
  describe("detectInitialLanguage", () => {
    it("returns ja when language starts with ja", () => {
      expect(detectInitialLanguage("ja-JP")).toBe("ja");
      expect(detectInitialLanguage("ja")).toBe("ja");
      expect(detectInitialLanguage("JA-jp")).toBe("ja");
    });

    it("returns en when language starts with en or other", () => {
      expect(detectInitialLanguage("en-US")).toBe("en");
      expect(detectInitialLanguage("en-GB")).toBe("en");
      expect(detectInitialLanguage("fr-FR")).toBe("en");
    });

    it("returns en when language is empty or undefined", () => {
      expect(detectInitialLanguage("")).toBe("en");
      expect(detectInitialLanguage(undefined)).toBe("en");
    });
  });

  describe("formatErrorMessage", () => {
    it("formats all 15 error codes in Japanese", () => {
      expect(formatErrorMessage("UnsupportedFormat", null, "ja")).toBe(
        "対応していない形式です",
      );
      expect(formatErrorMessage("DecodeFailed", null, "ja")).toBe(
        "画像を読み込めませんでした。ファイルが壊れている可能性があります",
      );
      expect(formatErrorMessage("PdfOpenFailed", null, "ja")).toBe(
        "PDF を読み込めませんでした。ファイルが壊れている可能性があります",
      );
      expect(formatErrorMessage("PdfEncrypted", null, "ja")).toBe(
        "暗号化された PDF は扱えません",
      );
      expect(formatErrorMessage("PdfSigned", null, "ja")).toBe(
        "電子署名付きの PDF は扱えません",
      );
      expect(formatErrorMessage("TooLarge", "256 MB", "ja")).toBe(
        "ファイルが大きすぎます（上限 256 MB）",
      );
      expect(formatErrorMessage("WorkerCrashed", null, "ja")).toBe(
        "PDF の処理中に問題が起きました。ファイルが壊れている可能性があります",
      );
      expect(formatErrorMessage("WorkerTimeout", null, "ja")).toBe(
        "PDF の処理に時間がかかりすぎたため中止しました",
      );
      expect(formatErrorMessage("VerifyFailed", null, "ja")).toBe(
        "情報を消しきれなかったため、保存しませんでした",
      );
      expect(formatErrorMessage("SameFolderAsSource", null, "ja")).toBe(
        "元のファイルと同じフォルダには保存できません。別のフォルダを選んでください",
      );
      expect(formatErrorMessage("ReadFailed", null, "ja")).toBe(
        "ファイルの読み込みに失敗しました",
      );
      expect(formatErrorMessage("WriteFailed", null, "ja")).toBe(
        "ファイルの書き込みに失敗しました",
      );
      expect(formatErrorMessage("JobRunning", null, "ja")).toBe(
        "処理中は操作できません",
      );
      expect(formatErrorMessage("UnknownHandle", null, "ja")).toBe(
        "ファイルをもう一度追加してください",
      );
      expect(formatErrorMessage("InvalidParams", null, "ja")).toBe(
        "無効な設定です",
      );
    });

    it("formats all 15 error codes in English", () => {
      expect(formatErrorMessage("UnsupportedFormat", null, "en")).toBe(
        "Unsupported file format",
      );
      expect(formatErrorMessage("DecodeFailed", null, "en")).toBe(
        "Couldn't read the image. The file may be damaged.",
      );
      expect(formatErrorMessage("PdfOpenFailed", null, "en")).toBe(
        "Couldn't read the PDF. The file may be damaged.",
      );
      expect(formatErrorMessage("PdfEncrypted", null, "en")).toBe(
        "Encrypted PDFs aren't supported",
      );
      expect(formatErrorMessage("PdfSigned", null, "en")).toBe(
        "Signed PDFs aren't supported",
      );
      expect(formatErrorMessage("TooLarge", "256 MB", "en")).toBe(
        "File is too large (limit: 256 MB)",
      );
      expect(formatErrorMessage("WorkerCrashed", null, "en")).toBe(
        "Something went wrong while processing the PDF. The file may be damaged.",
      );
      expect(formatErrorMessage("WorkerTimeout", null, "en")).toBe(
        "Processing the PDF took too long and was stopped",
      );
      expect(formatErrorMessage("VerifyFailed", null, "en")).toBe(
        "Couldn't remove all the information, so the file wasn't saved",
      );
      expect(formatErrorMessage("SameFolderAsSource", null, "en")).toBe(
        "Choose a folder other than the one the files are in",
      );
      expect(formatErrorMessage("ReadFailed", null, "en")).toBe(
        "Failed to read file",
      );
      expect(formatErrorMessage("WriteFailed", null, "en")).toBe(
        "Failed to write file",
      );
      expect(formatErrorMessage("JobRunning", null, "en")).toBe(
        "Not available while processing",
      );
      expect(formatErrorMessage("UnknownHandle", null, "en")).toBe(
        "Please add the file again",
      );
      expect(formatErrorMessage("InvalidParams", null, "en")).toBe(
        "Invalid settings",
      );
    });

    it("covers all error codes in design §6.6 with non-empty strings", () => {
      for (const code of ALL_ERROR_CODES) {
        const jaMsg = formatErrorMessage(code, "detail", "ja");
        const enMsg = formatErrorMessage(code, "detail", "en");
        expect(jaMsg).not.toBe("");
        expect(enMsg).not.toBe("");
        expect(jaMsg.length).toBeGreaterThan(0);
        expect(enMsg.length).toBeGreaterThan(0);
      }
    });
  });

  describe("dictionary keys parity and completeness", () => {
    it("has identical keys structure between ja and en", () => {
      function isRecord(value: unknown): value is Record<string, unknown> {
        return typeof value === "object" && value !== null;
      }

      function getKeys(obj: Record<string, unknown>, prefix = ""): string[] {
        const keys: string[] = [];
        for (const k of Object.keys(obj)) {
          const val = obj[k];
          const fullKey = prefix ? `${prefix}.${k}` : k;
          if (isRecord(val)) {
            keys.push(...getKeys(val, fullKey));
          } else {
            keys.push(fullKey);
          }
        }
        return keys.sort();
      }

      const jaKeys = getKeys(ja);
      const enKeys = getKeys(en);
      expect(enKeys).toEqual(jaKeys);
    });

    it("satisfies Translations type for en dictionary", () => {
      const typedEn: Translations = en;
      expect(typedEn).toBeDefined();
    });

    it("provides non-empty translations for all MetadataKind values in both ja and en", () => {
      for (const kind of METADATA_KINDS) {
        expect(ja.metadataKinds[kind]).toBeDefined();
        expect(ja.metadataKinds[kind].trim()).not.toBe("");
        expect(en.metadataKinds[kind]).toBeDefined();
        expect(en.metadataKinds[kind].trim()).not.toBe("");
      }
    });

    it("provides non-empty translations for all Field values in both ja and en", () => {
      for (const field of FIELDS) {
        expect(ja.fields[field]).toBeDefined();
        expect(ja.fields[field].trim()).not.toBe("");
        expect(en.fields[field]).toBeDefined();
        expect(en.fields[field].trim()).not.toBe("");
      }
    });

    it("provides detailValue template messages in both ja and en", () => {
      expect(ja.detailValue.bytes).toContain("{n}");
      expect(ja.detailValue.count).toContain("{n}");
      expect(en.detailValue.bytes).toContain("{n}");
      expect(en.detailValue.count).toContain("{n}");
    });
  });
});
