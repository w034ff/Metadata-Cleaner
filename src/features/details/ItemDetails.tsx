import { formatFileFormat } from "../../format/fileFormat";
import { formatFileSize } from "../../format/fileSize";
import { formatErrorMessage, getTranslations } from "../../i18n";
import { useAppState } from "../../state";
import { formatDetailValue } from "./detailValue";
import { formatKeptInfo } from "./formatKeptInfo";
import "./ItemDetails.css";

/**
 * Details sidebar showing metadata of the selected file (design §6.2, mockup BList.dc.html, BResult.dc.html).
 */
export function ItemDetails() {
  const { language, items, job, selectedId, details, detailsError } =
    useAppState();
  const t = getTranslations(language.language);

  if (selectedId === null) {
    return (
      <aside className="details-aside" aria-label={t.details.ariaLabel}>
        <span className="hint" style={{ lineHeight: 1.7 }}>
          {t.details.hintEmpty}
        </span>
      </aside>
    );
  }

  const item = items.find((it) => it.id === selectedId);
  if (!item) {
    return (
      <aside className="details-aside" aria-label={t.details.ariaLabel}>
        <span className="hint" style={{ lineHeight: 1.7 }}>
          {t.details.hintEmpty}
        </span>
      </aside>
    );
  }

  const jobResult = job.results[item.id];
  const isPostProcessing = jobResult !== undefined || job.finished !== null;

  return (
    <aside className="details-aside" aria-label={t.details.ariaLabel}>
      <div className="details-title-group">
        <span className="details-file-name">{item.name}</span>
        <span className="mono">
          {formatFileFormat(item.name, item.format)} ·{" "}
          {formatFileSize(item.bytes)}
        </span>
      </div>

      {isPostProcessing && jobResult !== undefined ? (
        // Post-processing details (mockup BResult.dc.html)
        <>
          {jobResult.status === "ok" && (
            <div className="details-status-ok">{t.job.rowStatus.ok}</div>
          )}
          {jobResult.status === "failed" && (
            <>
              <div className="details-status-failed">
                {t.job.rowStatus.failed}
              </div>
              {jobResult.error && (
                <div className="details-error-text" style={{ fontWeight: 400 }}>
                  {formatErrorMessage(
                    jobResult.error.code,
                    jobResult.error.detail,
                    language.language,
                  )}
                </div>
              )}
            </>
          )}
          {jobResult.status === "cancelled" && (
            <div className="details-status-cancelled">
              {t.job.rowStatus.cancelled}
            </div>
          )}

          {/* Saved file name (only if ok and savedName is present) */}
          {jobResult.status === "ok" && jobResult.savedName && (
            <section className="details-section">
              <span className="label">{t.details.savedNameTitle}</span>
              <span
                className="mono"
                style={{
                  color: "var(--color-text-primary)",
                  wordBreak: "break-all",
                }}
              >
                {jobResult.savedName}
              </span>
            </section>
          )}

          {/* Removed information (only for ok items, not failed ones) */}
          {jobResult.status === "ok" && jobResult.removed.length > 0 && (
            <section className="details-section details-section-divider">
              <span className="label">{t.details.removedTitle}</span>
              <ul className="details-list">
                {jobResult.removed.map((kind) => {
                  const isLoc = kind === "location";
                  const kindLabel = t.metadataKinds[kind];
                  const matchedGroup = details?.groups.find(
                    (g) => g.kind === kind,
                  );
                  const fieldNames = matchedGroup
                    ? Array.from(
                        new Set(
                          matchedGroup.entries.map((e) =>
                            e.field === "other" && e.name
                              ? e.name
                              : t.fields[e.field],
                          ),
                        ),
                      )
                    : [];
                  const suffix =
                    fieldNames.length > 0
                      ? `（${fieldNames.join(t.details.fieldSeparator)}）`
                      : "";

                  return (
                    <li key={kind}>
                      {isLoc ? (
                        <span
                          style={{
                            color: "var(--color-location-text)",
                            fontWeight: 500,
                          }}
                        >
                          {kindLabel}
                        </span>
                      ) : (
                        <span>{kindLabel}</span>
                      )}
                      {suffix}
                    </li>
                  );
                })}
              </ul>
            </section>
          )}

          {/* Kept information (for successful items) */}
          {jobResult.status === "ok" &&
            details?.kept &&
            details.kept.length > 0 && (
              <section className="details-section details-section-divider">
                <span className="label">{t.details.keptResultTitle}</span>
                <span className="hint" style={{ lineHeight: 1.7 }}>
                  {formatKeptInfo(details.kept, t)}
                </span>
              </section>
            )}
        </>
      ) : (
        // Pre-processing details (mockup BList.dc.html)
        <>
          {item.error !== null ? (
            // Inspection error (get_details was not called)
            <div className="details-error-text">
              ✕{" "}
              {item.error.code === "PdfSigned"
                ? t.details.pdfSignedReason
                : formatErrorMessage(
                    item.error.code,
                    item.error.detail,
                    language.language,
                  )}
            </div>
          ) : detailsError !== null ? (
            // get_details call error
            <div className="details-error-text">
              ✕{" "}
              {formatErrorMessage(
                detailsError.code,
                detailsError.detail,
                language.language,
              )}
            </div>
          ) : details !== null ? (
            <>
              {details.groups.length === 0 && (
                <span className="hint">{t.details.noneFound}</span>
              )}

              {details.groups.map((group) => {
                const isLoc = group.kind === "location";
                return (
                  <section key={group.kind} className="details-section">
                    <span
                      className="label"
                      style={{
                        color: isLoc ? "var(--color-location-text)" : undefined,
                      }}
                    >
                      {t.metadataKinds[group.kind]}
                    </span>
                    <dl className="details-dl">
                      {group.entries.map((entry, idx) => {
                        const fieldName =
                          entry.field === "other" && entry.name
                            ? entry.name
                            : t.fields[entry.field];
                        const isMono =
                          entry.field === "latitude" ||
                          entry.field === "longitude" ||
                          entry.field === "serialNumber";

                        return (
                          <div key={`${entry.field}-${idx}`}>
                            <dt>{fieldName}</dt>
                            <dd className={isMono ? "mono" : undefined}>
                              {formatDetailValue(entry.value, t)}
                            </dd>
                          </div>
                        );
                      })}
                    </dl>
                  </section>
                );
              })}

              {details.truncated && (
                <span className="hint">{t.details.truncated}</span>
              )}

              {details.kept && details.kept.length > 0 && (
                <section className="details-section details-section-divider">
                  <span className="label">{t.details.keptTitle}</span>
                  <span className="hint" style={{ lineHeight: 1.7 }}>
                    {formatKeptInfo(details.kept, t)}
                  </span>
                </section>
              )}
            </>
          ) : null}
        </>
      )}
    </aside>
  );
}
