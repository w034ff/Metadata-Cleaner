import { ProgressBar } from "../../components";
import { formatMessage, getTranslations } from "../../i18n";
import { useAppState } from "../../state";
import "./JobFooter.css";
import { useJobRunner } from "./useJobRunner";

/**
 * Bottom action and progress bar of the application (mockup BList.dc.html, BResult.dc.html).
 */
export function JobFooter() {
  const { language, items, job } = useAppState();
  const { startJob, cancelJob, isRunning, isCancelling, isPickingOutputDir } =
    useJobRunner();

  const t = getTranslations(language.language);
  const isActive = isRunning || isCancelling;

  const doneCount = job.progress?.done ?? 0;
  const totalCount = job.progress?.total ?? job.targets.length;

  return (
    <footer className="job-footer">
      {isActive ? (
        <>
          <div className="job-footer-progress">
            <ProgressBar
              value={doneCount}
              max={totalCount}
              label={t.footer.progressLabel}
            />
          </div>
          <button
            type="button"
            className="btn btn-ghost job-footer-action"
            disabled={isCancelling}
            onClick={() => void cancelJob()}
          >
            {isCancelling ? t.footer.cancelling : t.footer.cancel}
          </button>
        </>
      ) : (
        <>
          {job.finished !== null ? (
            job.finished.failed > 0 || job.finished.unprocessed > 0 ? (
              <span className="job-footer-status-error">
                ✕ {t.job.doneWithFailures}
              </span>
            ) : (
              <span className="hint">{t.job.doneAllSucceeded}</span>
            )
          ) : items.length > 0 ? (
            <span className="hint">
              {formatMessage(t.footer.cleanCount, { count: items.length })}
            </span>
          ) : (
            <span className="hint">{t.footer.noFiles}</span>
          )}

          <button
            type="button"
            className="btn btn-primary job-footer-action"
            disabled={items.length === 0 || isPickingOutputDir}
            onClick={() => void startJob()}
          >
            {t.footer.cleanAndSave}
          </button>
        </>
      )}
    </footer>
  );
}
