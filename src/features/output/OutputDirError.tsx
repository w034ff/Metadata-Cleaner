import { ErrorDisplay } from "../../components";
import { formatErrorMessage } from "../../i18n";
import { useAppState } from "../../state";

/**
 * The band above a list that displays errors preventing a job from starting
 * (e.g. output folder errors) as well as any other job errors (design §6.5, §6.6).
 */
export function OutputDirError() {
  const { language, job } = useAppState();
  if (job.error === null) {
    return null;
  }
  return (
    <ErrorDisplay
      message={formatErrorMessage(
        job.error.code,
        job.error.detail,
        language.language,
      )}
    />
  );
}
