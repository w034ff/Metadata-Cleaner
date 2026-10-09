import { useCallback } from "react";
import { DropZone, SegmentedControl } from "./components";
import { useItemsDropped } from "./features/items";
import { JobFooter, JobSummaryBanner, useJobEvents } from "./features/job";
import { OutputDirError, OutputDirField } from "./features/output";
import { useBlockBrowserShortcuts } from "./features/shortcuts";
import { getTranslations } from "./i18n";
import { addFiles } from "./ipc";
import {
  AppStateProvider,
  isJobActive,
  useAppDispatch,
  useAppState,
} from "./state";
import "./styles/app.css";

export function AppShell() {
  useBlockBrowserShortcuts();
  useJobEvents();
  useItemsDropped();

  const { language, items, outputDir, job } = useAppState();
  const dispatch = useAppDispatch();
  const t = getTranslations(language.language);

  const isJobRunning = isJobActive(job);

  const handleAddFiles = useCallback(async () => {
    try {
      const result = await addFiles("files");
      if (result !== null && result.added.length > 0) {
        dispatch({ type: "ADD_ITEMS", items: result.added });
      }
    } catch {
      // IPC errors handled by backend/dialog
    }
  }, [dispatch]);

  const handleAddFolder = useCallback(async () => {
    try {
      const result = await addFiles("folder");
      if (result !== null && result.added.length > 0) {
        dispatch({ type: "ADD_ITEMS", items: result.added });
      }
    } catch {
      // IPC errors handled by backend/dialog
    }
  }, [dispatch]);

  return (
    <div className="app-layout">
      <header className="app-header">
        <h1 className="app-title">
          <svg
            className="app-title-icon"
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M20 20H9l-5-5a2 2 0 0 1 0-2.8l8.2-8.2a2 2 0 0 1 2.8 0l5 5a2 2 0 0 1 0 2.8L12 20" />
            <path d="M8.5 8.5l7 7" />
          </svg>
          {t.app.title}
        </h1>
        <div className="app-header-actions">
          <SegmentedControl
            label={t.app.languageLabel}
            value={language.language}
            disabled={isJobRunning}
            options={[
              { value: "ja", label: t.app.languages.ja },
              { value: "en", label: t.app.languages.en },
            ]}
            onChange={(lang) =>
              dispatch({ type: "SET_LANGUAGE", language: lang })
            }
          />
        </div>
      </header>

      <div className="app-body">
        <main className="app-main">
          <OutputDirField
            value={outputDir}
            disabled={isJobRunning}
            onChange={(dir) =>
              dispatch({ type: "SET_OUTPUT_DIR", outputDir: dir })
            }
          />

          <OutputDirError />

          <JobSummaryBanner finished={job.finished} />

          {items.length === 0 && (
            <DropZone
              title={t.dropZone.title}
              description={t.dropZone.description}
              addFilesLabel={t.dropZone.addFiles}
              addFolderLabel={t.dropZone.addFolder}
              onAddFiles={handleAddFiles}
              onAddFolder={handleAddFolder}
            />
          )}
        </main>

        <aside className="app-aside" aria-label={t.details.ariaLabel}>
          <span className="hint" style={{ lineHeight: 1.7 }}>
            {t.details.hintEmpty}
          </span>
        </aside>
      </div>

      <JobFooter />
    </div>
  );
}

export function App() {
  return (
    <AppStateProvider>
      <AppShell />
    </AppStateProvider>
  );
}
