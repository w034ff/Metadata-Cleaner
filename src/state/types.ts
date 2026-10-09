import type { Language } from "../i18n";
import type { FileItem, OutputDirLabel } from "../ipc";
import type { JobAction, JobState } from "./job";

export interface LanguageState {
  /** The language the screen is shown in. */
  language: Language;
  /**
   * The language the user picked, saved as is: `null` until they pick one,
   * so the screen keeps following the OS language (design §6.7, FR-07).
   */
  preference: Language | null;
}

export type LanguageAction = { type: "SET_LANGUAGE"; language: Language };

export interface AppState {
  language: LanguageState;
  items: FileItem[];
  outputDir: OutputDirLabel | null;
  job: JobState;
}

export type AppAction =
  | LanguageAction
  | { type: "SET_ITEMS"; items: FileItem[] }
  | { type: "ADD_ITEMS"; items: FileItem[] }
  | { type: "REMOVE_ITEMS"; ids: number[] }
  | { type: "CLEAR_ITEMS" }
  | { type: "SET_OUTPUT_DIR"; outputDir: OutputDirLabel | null }
  | JobAction;
