import { useCallback } from "react";
import { formatMessage, getTranslations } from "../../i18n";
import { addFiles, removeItems } from "../../ipc";
import { isJobActive, useAppDispatch, useAppState } from "../../state";

import { formatSkippedMessage } from "./skipped";

export function ItemListHeader() {
  const { language, items, job, lastSkipped } = useAppState();
  const dispatch = useAppDispatch();
  const t = getTranslations(language.language);

  const isBusy = isJobActive(job);
  const skippedMessage = formatSkippedMessage(lastSkipped, language.language);

  const handleAddFiles = useCallback(async () => {
    try {
      const result = await addFiles("files");
      if (result !== null) {
        dispatch({
          type: "ADD_ITEMS",
          items: result.added,
          skipped: result.skipped,
        });
      }
    } catch {
      // IPC errors handled by backend/dialog
    }
  }, [dispatch]);

  const handleAddFolder = useCallback(async () => {
    try {
      const result = await addFiles("folder");
      if (result !== null) {
        dispatch({
          type: "ADD_ITEMS",
          items: result.added,
          skipped: result.skipped,
        });
      }
    } catch {
      // IPC errors handled by backend/dialog
    }
  }, [dispatch]);

  const handleClearAll = useCallback(async () => {
    const ids = items.map((it) => it.id);
    try {
      await removeItems(ids);
      dispatch({ type: "CLEAR_ITEMS" });
    } catch {
      // Handled silently or by backend
    }
  }, [items, dispatch]);

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        flexWrap: "wrap",
      }}
    >
      <span className="label">
        {formatMessage(t.itemList.count, { count: items.length })}
        {skippedMessage ? ` ${skippedMessage}` : ""}
      </span>
      <div
        style={{
          marginLeft: "auto",
          display: "flex",
          gap: 8,
          flexWrap: "wrap",
        }}
      >
        <button
          type="button"
          className="btn btn-ghost"
          disabled={isBusy}
          onClick={() => void handleAddFiles()}
        >
          {t.dropZone.addFiles}
        </button>
        <button
          type="button"
          className="btn btn-ghost"
          disabled={isBusy}
          onClick={() => void handleAddFolder()}
        >
          {t.dropZone.addFolder}
        </button>
        <button
          type="button"
          className="btn btn-ghost"
          disabled={isBusy || items.length === 0}
          onClick={() => void handleClearAll()}
        >
          {t.itemList.clearAll}
        </button>
      </div>
    </div>
  );
}
