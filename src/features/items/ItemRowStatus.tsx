import { formatErrorMessage, getTranslations, type Language } from "../../i18n";
import type { FileItem, JobItemPayload } from "../../ipc";
import { isJobActive, runningIds, type JobState } from "../../state";

export interface ItemRowStatusProps {
  item: FileItem;
  job: JobState;
  language: Language;
}

/**
 * Renders the status cell of a row in the files table (design §10.1, mockup BList.dc.html, BResult.dc.html).
 */
export function ItemRowStatus({ item, job, language }: ItemRowStatusProps) {
  const t = getTranslations(language);

  // 1. If inspection failed when item was added
  if (item.error !== null) {
    const reason = formatErrorMessage(
      item.error.code,
      item.error.detail,
      language,
    );
    return (
      <td
        className="item-status-cell item-status-error"
        style={{ color: "var(--color-error)", fontWeight: 500 }}
      >
        <span>{t.job.rowStatus.loadFailed}</span>
        {reason && (
          <div
            title={reason}
            style={{
              fontWeight: 400,
              fontSize: 12,
              color: "var(--color-text-secondary)",
              marginTop: 2,
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
            }}
          >
            {reason}
          </div>
        )}
      </td>
    );
  }

  // Check job item results
  const jobResult: JobItemPayload | undefined = job.results[item.id];

  if (jobResult !== undefined) {
    if (jobResult.status === "ok") {
      return (
        <td
          className="item-status-cell item-status-ok"
          style={{ color: "var(--color-success)", fontWeight: 500 }}
        >
          <span>{t.job.rowStatus.ok}</span>
        </td>
      );
    }
    if (jobResult.status === "failed") {
      const reason = jobResult.error
        ? formatErrorMessage(
            jobResult.error.code,
            jobResult.error.detail,
            language,
          )
        : null;
      return (
        <td
          className="item-status-cell item-status-failed"
          style={{ color: "var(--color-error)", fontWeight: 500 }}
        >
          <span>{t.job.rowStatus.failed}</span>
          {reason && (
            <div
              title={reason}
              style={{
                fontWeight: 400,
                fontSize: 12,
                color: "var(--color-text-secondary)",
                marginTop: 2,
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
              }}
            >
              {reason}
            </div>
          )}
        </td>
      );
    }
    if (jobResult.status === "cancelled") {
      return (
        <td
          className="item-status-cell item-status-cancelled"
          style={{ color: "var(--color-text-secondary)", fontWeight: 400 }}
        >
          <span>{t.job.rowStatus.cancelled}</span>
        </td>
      );
    }
  }

  // 2. While job is active
  if (isJobActive(job)) {
    if (runningIds(job).includes(item.id)) {
      return (
        <td
          className="item-status-cell item-status-running"
          style={{ color: "var(--color-text-primary)", fontWeight: 400 }}
        >
          <span>{t.job.rowStatus.running}</span>
        </td>
      );
    }
    return (
      <td
        className="item-status-cell item-status-waiting"
        style={{ color: "var(--color-text-secondary)", fontWeight: 400 }}
      >
        <span>{t.job.rowStatus.waiting}</span>
      </td>
    );
  }

  // 3. After job finished but item had no result
  if (job.finished !== null) {
    const isTarget = job.targets.some((tg) => tg.id === item.id);
    if (isTarget) {
      return (
        <td
          className="item-status-cell item-status-unprocessed"
          style={{ color: "var(--color-text-secondary)", fontWeight: 400 }}
        >
          <span>{t.job.rowStatus.unprocessed}</span>
        </td>
      );
    }
  }

  // 4. Default before processing: waiting
  return (
    <td
      className="item-status-cell item-status-waiting"
      style={{ color: "var(--color-text-secondary)", fontWeight: 400 }}
    >
      <span>{t.job.rowStatus.waiting}</span>
    </td>
  );
}
