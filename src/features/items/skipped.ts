import { formatMessage, getTranslations, type Language } from "../../i18n";
import type { Skipped } from "../../ipc";

/**
 * Formats a message describing items skipped during addition (design §10.1, requirements FR-01).
 * E.g. "（対象外 3 件：サブフォルダ、非対応の形式）"
 * Returns `null` if total skipped count is 0.
 * Reasons appear in order: subfolders, unsupported formats, duplicates.
 */
export function formatSkippedMessage(
  skipped: Skipped | null | undefined,
  language: Language,
): string | null {
  if (!skipped) {
    return null;
  }
  const total = skipped.folders + skipped.unsupported + skipped.duplicates;
  if (total === 0) {
    return null;
  }

  const t = getTranslations(language);
  const reasons: string[] = [];
  if (skipped.folders > 0) {
    reasons.push(t.skipped.folders);
  }
  if (skipped.unsupported > 0) {
    reasons.push(t.skipped.unsupported);
  }
  if (skipped.duplicates > 0) {
    reasons.push(t.skipped.duplicates);
  }

  const reasonsText = reasons.join(t.skipped.reasonSeparator);
  return formatMessage(t.skipped.message, {
    count: total,
    reasons: reasonsText,
  });
}
