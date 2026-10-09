import { useState } from "react";
import { formatErrorMessage, getTranslations } from "../../i18n";
import {
  normalizeIpcError,
  pickOutputDir,
  type IpcError,
  type OutputDirLabel,
} from "../../ipc";
import { useAppState } from "../../state";
import "./OutputDirField.css";

export interface OutputDirFieldProps {
  /** The folder in use, or `null` if none has been picked. */
  value: OutputDirLabel | null;
  /** Called with the folder the user picked; not called on cancel. */
  onChange: (value: OutputDirLabel) => void;
  disabled?: boolean;
}

/**
 * The "保存先フォルダ" field of the main screen (mockup BList.dc.html).
 * Rust opens the dialog and keeps the path; the screen only ever sees the folder's name (design §7.1).
 */
export function OutputDirField({
  value,
  onChange,
  disabled = false,
}: OutputDirFieldProps) {
  const { language } = useAppState();
  const t = getTranslations(language.language);
  const [error, setError] = useState<IpcError | null>(null);

  async function pick() {
    try {
      const picked = await pickOutputDir();
      setError(null);
      if (picked !== null) {
        onChange(picked);
      }
    } catch (failure: unknown) {
      setError(normalizeIpcError(failure));
    }
  }

  return (
    <div className="output-dir-field">
      <span className="label">{t.outputDir.title}</span>
      <span
        className={value === null ? "hint" : "mono output-dir-field-name"}
        data-testid="output-dir-name"
      >
        {value === null ? t.outputDir.notChosen : value.dirLabel}
      </span>
      <button
        type="button"
        className="btn btn-ghost output-dir-field-button"
        disabled={disabled}
        onClick={() => void pick()}
      >
        {t.outputDir.choose}
      </button>
      {error !== null && (
        <span className="error-text" role="alert">
          {formatErrorMessage(error.code, error.detail, language.language)}
        </span>
      )}
    </div>
  );
}
