import type { Settings } from "../ipc";
import { initialJobState, jobReducer } from "./job";
import { createInitialLanguageState, languageReducer } from "./language";
import type { AppAction, AppState } from "./types";

/**
 * The state the app starts in: the saved `settings` (design §6.7), or the
 * defaults when there are none. The list of items always starts empty.
 */
export function createInitialAppState(
  initialNavLang?: string,
  settings: Settings | null = null,
): AppState {
  if (settings === null) {
    return {
      language: createInitialLanguageState(initialNavLang),
      items: [],
      outputDir: null,
      job: { ...initialJobState },
    };
  }
  return {
    language: createInitialLanguageState(initialNavLang, settings.language),
    items: [],
    outputDir: settings.outputDir,
    job: { ...initialJobState },
  };
}

/**
 * Drops the last cleaning job's result once the list or the output folder
 * changes: the footer and banners describe items and settings that are no
 * longer there (design §10.1). A running job has neither a result nor an error
 * yet, so it is never dropped here.
 */
function dropStaleResult(state: AppState, next: AppState): AppState {
  if (
    next === state ||
    (state.job.finished === null && state.job.error === null)
  ) {
    return next;
  }
  return { ...next, job: initialJobState };
}

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case "SET_LANGUAGE":
      return {
        ...state,
        language: languageReducer(state.language, action),
      };
    case "SET_ITEMS":
      return dropStaleResult(state, { ...state, items: action.items });
    case "ADD_ITEMS":
      return dropStaleResult(state, {
        ...state,
        items: [...state.items, ...action.items],
      });
    case "REMOVE_ITEMS":
      return dropStaleResult(state, {
        ...state,
        items: state.items.filter((item) => !action.ids.includes(item.id)),
      });
    case "CLEAR_ITEMS":
      return dropStaleResult(state, { ...state, items: [] });
    case "SET_OUTPUT_DIR":
      return dropStaleResult(state, { ...state, outputDir: action.outputDir });
    case "JOB_STARTED":
    case "JOB_CANCEL_REQUESTED":
    case "JOB_PROGRESS":
    case "JOB_ITEM":
    case "JOB_FINISHED":
    case "JOB_FAILED":
    case "JOB_NOT_STARTED":
    case "JOB_RESET":
      return {
        ...state,
        job: jobReducer(state.job, action),
      };
  }
}
