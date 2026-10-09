import type { ReactNode } from "react";
import "./DropZone.css";

export interface DropZoneProps {
  title: string;
  description: string;
  addFilesLabel?: string;
  addFolderLabel?: string;
  onAddFiles?: () => void;
  onAddFolder?: () => void;
  isDragOver?: boolean;
  icon?: ReactNode;
}

/**
 * Drop zone area displayed when queue is empty (design §10.1, mockup BList.dc.html).
 * Pure presentation component that does not invoke IPC directly.
 */
export function DropZone({
  title,
  description,
  addFilesLabel = "ファイルを追加",
  addFolderLabel = "フォルダを追加",
  onAddFiles,
  onAddFolder,
  isDragOver = false,
  icon,
}: DropZoneProps) {
  return (
    <div
      role="region"
      aria-label={title}
      className={`drop-zone ${isDragOver ? "is-drag-over" : ""}`}
      data-testid="drop-zone"
    >
      {icon ? (
        icon
      ) : (
        <svg
          className="drop-zone-icon"
          width="48"
          height="48"
          viewBox="0 0 24 24"
          fill="none"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
          <polyline points="14 2 14 8 20 8" />
          <line x1="12" y1="18" x2="12" y2="12" />
          <line x1="9" y1="15" x2="15" y2="15" />
        </svg>
      )}

      <p className="drop-zone-title">{title}</p>
      <p className="drop-zone-description">{description}</p>

      {(onAddFiles || onAddFolder) && (
        <div className="drop-zone-actions">
          {onAddFiles && (
            <button
              type="button"
              className="btn btn-primary"
              onClick={onAddFiles}
            >
              {addFilesLabel}
            </button>
          )}
          {onAddFolder && (
            <button
              type="button"
              className="btn btn-ghost"
              onClick={onAddFolder}
            >
              {addFolderLabel}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
