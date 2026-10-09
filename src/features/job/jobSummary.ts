import { formatMessage, getTranslations, type Language } from "../../i18n";
import type { JobFinishedPayload } from "../../ipc";

/**
 * Returns whether all items were successfully cleaned without any failures, unprocessed items, or unreadable items.
 */
export function isJobSuccessful(
  finished: JobFinishedPayload,
  unreadableCount = 0,
): boolean {
  return (
    !finished.cancelled &&
    finished.failed === 0 &&
    finished.unprocessed === 0 &&
    unreadableCount === 0
  );
}

/**
 * Formats the summary string of a finished job (design §7.2, mockup BResult.dc.html).
 * E.g. "保存 4 件 · 失敗 1 件 · 対象外 1 件"
 */
export function formatJobSummary(
  finished: JobFinishedPayload,
  language: Language,
  unreadableCount = 0,
): string {
  const t = getTranslations(language);
  const parts: string[] = [];

  if (
    finished.succeeded > 0 ||
    (finished.failed === 0 &&
      finished.unprocessed === 0 &&
      unreadableCount === 0)
  ) {
    parts.push(
      formatMessage(t.job.counts.succeeded, { count: finished.succeeded }),
    );
  }
  if (finished.failed > 0) {
    parts.push(formatMessage(t.job.counts.failed, { count: finished.failed }));
  }
  if (unreadableCount > 0) {
    parts.push(
      formatMessage(t.job.counts.unreadable, { count: unreadableCount }),
    );
  }
  if (finished.unprocessed > 0) {
    parts.push(
      formatMessage(t.job.counts.unprocessed, { count: finished.unprocessed }),
    );
  }

  const countsText = parts.join(t.job.summarySeparator);
  const template = finished.cancelled
    ? t.job.summaryCancelled
    : t.job.summaryFinished;

  return formatMessage(template, { counts: countsText });
}
