import { useCallback, useEffect, useRef, useState } from "react";
import { DropZone } from "./components";
import { AboutDialog } from "./features/about";
import { ItemDetails } from "./features/details";
import {
  formatSkippedMessage,
  ItemList,
  useItemsDropped,
} from "./features/items";
import { JobFooter, JobSummaryBanner, useJobEvents } from "./features/job";
import { OutputDirError, OutputDirField } from "./features/output";
import { useSettingsAutoSave } from "./features/settings";
import { useBlockBrowserShortcuts } from "./features/shortcuts";
import { getTranslations, isLanguage, SUPPORTED_LANGUAGES } from "./i18n";
import { addFiles, getSettings, type Settings } from "./ipc";
import {
  AppStateProvider,
  createInitialAppState,
  isJobActive,
  useAppDispatch,
  useAppState,
} from "./state";
import "./styles/app.css";

export function AppShell() {
  useBlockBrowserShortcuts();
  useJobEvents();
  useItemsDropped();
  useSettingsAutoSave();

  const { language, items, outputDir, job, lastSkipped } = useAppState();
  const dispatch = useAppDispatch();
  const t = getTranslations(language.language);
  const skippedMessage = formatSkippedMessage(lastSkipped, language.language);

  const [isAboutOpen, setIsAboutOpen] = useState(false);
  const aboutButtonRef = useRef<HTMLButtonElement>(null);
  const wasAboutOpenRef = useRef(false);

  // Give focus back to the button that opened the dialog.
  useEffect(() => {
    if (wasAboutOpenRef.current && !isAboutOpen) {
      aboutButtonRef.current?.focus();
    }
    wasAboutOpenRef.current = isAboutOpen;
  }, [isAboutOpen]);

  const isJobRunning = isJobActive(job);

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

  return (
    <>
      <div className="app-container" inert={isAboutOpen ? true : undefined}>
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
            <select
              className="lang-select"
              aria-label={t.app.languageLabel}
              value={language.language}
              disabled={isJobRunning}
              onChange={(event) => {
                const lang = event.target.value;
                if (isLanguage(lang)) {
                  dispatch({ type: "SET_LANGUAGE", language: lang });
                }
              }}
            >
              {SUPPORTED_LANGUAGES.map((lang) => (
                <option key={lang} value={lang}>
                  {t.app.languages[lang]}
                </option>
              ))}
            </select>
            <button
              ref={aboutButtonRef}
              type="button"
              className="icon-btn"
              aria-label={t.app.aboutButtonAria}
              onClick={() => setIsAboutOpen(true)}
            >
              <svg
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                aria-hidden="true"
              >
                <circle cx="12" cy="12" r="9" />
                <path d="M12 11v5" />
                <path d="M12 8h.01" />
              </svg>
            </button>
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

            <JobSummaryBanner finished={job.finished} />

            <OutputDirError />

            {items.length === 0 ? (
              <>
                <DropZone
                  title={t.dropZone.title}
                  description={t.dropZone.description}
                  addFilesLabel={t.dropZone.addFiles}
                  addFolderLabel={t.dropZone.addFolder}
                  onAddFiles={handleAddFiles}
                  onAddFolder={handleAddFolder}
                />
                {skippedMessage !== null && (
                  <p className="hint drop-zone-skipped">{skippedMessage}</p>
                )}
              </>
            ) : (
              <ItemList />
            )}
          </main>

          <ItemDetails />
        </div>

        <JobFooter />
      </div>

      <AboutDialog isOpen={isAboutOpen} onClose={() => setIsAboutOpen(false)} />
    </>
  );
}

export interface AppProps {
  /** The OS language to assume instead of `navigator.language`. */
  initialNavLang?: string;
  /**
   * Settings to start from instead of asking Rust with `get_settings`;
   * `null` starts from the defaults.
   */
  initialSettings?: Settings | null;
}

/**
 * The app. It shows nothing until the saved settings are read (design §6.7),
 * so the screen does not first appear in the defaults and then change.
 */
export function App({ initialNavLang, initialSettings }: AppProps = {}) {
  const [settings, setSettings] = useState<
    { loaded: false } | { loaded: true; value: Settings | null }
  >(
    initialSettings === undefined
      ? { loaded: false }
      : { loaded: true, value: initialSettings },
  );

  useEffect(() => {
    if (settings.loaded) {
      return;
    }
    let isMounted = true;
    getSettings().then(
      (value) => {
        if (isMounted) {
          setSettings({ loaded: true, value });
        }
      },
      () => {
        if (isMounted) {
          setSettings({ loaded: true, value: null });
        }
      },
    );
    return () => {
      isMounted = false;
    };
  }, [settings.loaded]);

  if (!settings.loaded) {
    return <div className="app-container" />;
  }

  return (
    <AppStateProvider
      initialState={createInitialAppState(initialNavLang, settings.value)}
    >
      <AppShell />
    </AppStateProvider>
  );
}
