import { ErrorDisplay } from "../../components";
import { formatErrorMessage } from "../../i18n";
import { useAppState } from "../../state";
import { isOutputDirError } from "./outputDirErrors";

/**
 * The band above a list that says cleaning did not start because its
 * output folder is the same folder as a source file (design §6.6).
 */
export function OutputDirError() {
  const { language, job } = useAppState();
  if (job.error === null || !isOutputDirError(job.error)) {
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
