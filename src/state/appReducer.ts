import type { Settings } from "../ipc";
import { initialJobState, jobReducer } from "./job";
import { createInitialLanguageState, languageReducer } from "./language";
import type { AppAction, AppState } from "./types";

/**
 * The state the app starts in: the saved `settings` (design §6.7), or the
 * defaults when there are none. The list of items always starts empty.
 */
/** Request number that no details answer carries: request numbers start at 1. */
const NO_DETAILS_REQUEST = 0;

export function createInitialAppState(
  initialNavLang?: string,
  settings: Settings | null = null,
): AppState {
  if (settings === null) {
    return {
      language: createInitialLanguageState(initialNavLang),
      items: [],
      lastSkipped: null,
      outputDir: null,
      job: { ...initialJobState },
      selectedId: null,
      detailsRequestId: NO_DETAILS_REQUEST,
      details: null,
      detailsError: null,
      isLoadingDetails: false,
    };
  }
  return {
    language: createInitialLanguageState(initialNavLang, settings.language),
    items: [],
    lastSkipped: null,
    outputDir: settings.outputDir,
    job: { ...initialJobState },
    selectedId: null,
    detailsRequestId: NO_DETAILS_REQUEST,
    details: null,
    detailsError: null,
    isLoadingDetails: false,
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
    case "SET_ITEMS": {
      const stillSelected =
        state.selectedId !== null &&
        action.items.some((item) => item.id === state.selectedId);
      const nextState: AppState = {
        ...state,
        items: action.items,
        selectedId: stillSelected ? state.selectedId : null,
        details: stillSelected ? state.details : null,
        detailsError: stillSelected ? state.detailsError : null,
        isLoadingDetails: stillSelected ? state.isLoadingDetails : false,
      };
      return dropStaleResult(state, nextState);
    }
    case "ADD_ITEMS":
      if (action.items.length === 0) {
        return {
          ...state,
          lastSkipped: action.skipped ?? null,
        };
      }
      return dropStaleResult(state, {
        ...state,
        items: [...state.items, ...action.items],
        lastSkipped: action.skipped ?? null,
      });
    case "REMOVE_ITEMS": {
      const wasSelected =
        state.selectedId !== null && action.ids.includes(state.selectedId);
      const nextState: AppState = {
        ...state,
        items: state.items.filter((item) => !action.ids.includes(item.id)),
        selectedId: wasSelected ? null : state.selectedId,
        details: wasSelected ? null : state.details,
        detailsError: wasSelected ? null : state.detailsError,
        isLoadingDetails: wasSelected ? false : state.isLoadingDetails,
      };
      return dropStaleResult(state, nextState);
    }
    case "CLEAR_ITEMS": {
      const nextState: AppState = {
        ...state,
        items: [],
        lastSkipped: null,
        selectedId: null,
        details: null,
        detailsError: null,
        isLoadingDetails: false,
      };
      return dropStaleResult(state, nextState);
    }
    case "SET_OUTPUT_DIR":
      return dropStaleResult(state, { ...state, outputDir: action.outputDir });
    case "SELECT_ITEM": {
      if (action.id === null || action.id === state.selectedId) {
        return {
          ...state,
          selectedId: null,
          detailsRequestId: NO_DETAILS_REQUEST,
          details: null,
          detailsError: null,
          isLoadingDetails: false,
        };
      }
      // A new selection invalidates any answer still on its way; only the
      // request started for this row (FETCH_DETAILS_START) is accepted.
      return {
        ...state,
        selectedId: action.id,
        detailsRequestId: NO_DETAILS_REQUEST,
        details: null,
        detailsError: null,
        isLoadingDetails: false,
      };
    }
    case "FETCH_DETAILS_START":
      return {
        ...state,
        detailsRequestId: action.requestId,
        details: null,
        detailsError: null,
        isLoadingDetails: true,
      };
    case "FETCH_DETAILS_SUCCESS":
      if (action.requestId !== state.detailsRequestId) {
        return state;
      }
      return {
        ...state,
        details: action.details,
        detailsError: null,
        isLoadingDetails: false,
      };
    case "FETCH_DETAILS_FAILURE":
      if (action.requestId !== state.detailsRequestId) {
        return state;
      }
      return {
        ...state,
        details: null,
        detailsError: action.error,
        isLoadingDetails: false,
      };
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
