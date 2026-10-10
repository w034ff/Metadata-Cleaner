import type { JobFinishedPayload } from "../../ipc";
import { useAppState } from "../../state";
import "./JobSummaryBanner.css";
import { formatJobSummary, jobOutcome } from "./jobSummary";

export interface JobSummaryBannerProps {
  finished: JobFinishedPayload | null;
}

/**
 * Status banner shown when a cleaning job finishes (mockup BResult.dc.html).
 */
export function JobSummaryBanner({ finished }: JobSummaryBannerProps) {
  const { language, items } = useAppState();

  if (finished === null) {
    return null;
  }

  const unreadableCount = items.filter((it) => it.error !== null).length;
  const outcome = jobOutcome(finished);
  const text = formatJobSummary(finished, language.language, unreadableCount);

  return (
    <div
      role="status"
      className={`job-summary-banner ${
        outcome === "success"
          ? "job-summary-banner-success"
          : outcome === "failure"
            ? "job-summary-banner-warning"
            : "job-summary-banner-neutral"
      }`}
    >
      {outcome !== "cancelled" && (
        <span
          className={`job-summary-icon ${
            outcome === "success"
              ? "job-summary-icon-success"
              : "job-summary-icon-warning"
          }`}
          aria-hidden="true"
        >
          {outcome === "success" ? "✓" : "✕"}
        </span>
      )}
      <span>{text}</span>
    </div>
  );
}
