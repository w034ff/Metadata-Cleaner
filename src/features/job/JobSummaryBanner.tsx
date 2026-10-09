import type { JobFinishedPayload } from "../../ipc";
import { useAppState } from "../../state";
import "./JobSummaryBanner.css";
import { formatJobSummary, isJobSuccessful } from "./jobSummary";

export interface JobSummaryBannerProps {
  finished: JobFinishedPayload | null;
}

/**
 * Status banner shown when a cleaning job finishes (mockup BResult.dc.html).
 */
export function JobSummaryBanner({ finished }: JobSummaryBannerProps) {
  const { language } = useAppState();

  if (finished === null) {
    return null;
  }

  const successful = isJobSuccessful(finished);
  const text = formatJobSummary(finished, language.language);

  return (
    <div
      role="status"
      className={`job-summary-banner ${
        successful ? "job-summary-banner-success" : "job-summary-banner-warning"
      }`}
    >
      <span
        className={`job-summary-icon ${
          successful ? "job-summary-icon-success" : "job-summary-icon-warning"
        }`}
        aria-hidden="true"
      >
        {successful ? "✓" : "✕"}
      </span>
      <span>{text}</span>
    </div>
  );
}
