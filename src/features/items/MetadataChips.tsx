import { getTranslations, type Language } from "../../i18n";
import type { MetadataKind } from "../../ipc";

export interface MetadataChipsProps {
  kinds: readonly MetadataKind[];
  hasError: boolean;
  language: Language;
}

/**
 * Renders the chips for discovered metadata categories in a table cell (mockup BList.dc.html).
 */
export function MetadataChips({
  kinds,
  hasError,
  language,
}: MetadataChipsProps) {
  if (hasError) {
    return null;
  }

  const t = getTranslations(language);

  if (kinds.length === 0) {
    return <span className="hint">{t.itemList.noMetadata}</span>;
  }

  return (
    <div style={{ display: "flex", flexWrap: "wrap", gap: 4 }}>
      {kinds.map((kind) => {
        const isLoc = kind === "location";
        const label = t.metadataKinds[kind];
        if (isLoc) {
          return (
            <span key={kind} className="chip chip-loc">
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2.2"
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true"
              >
                <path d="M12 21s-7-6.2-7-12a7 7 0 0 1 14 0c0 5.8-7 12-7 12z" />
                <circle cx="12" cy="9" r="2.5" />
              </svg>
              {label}
            </span>
          );
        }
        return (
          <span key={kind} className="chip">
            {label}
          </span>
        );
      })}
    </div>
  );
}
