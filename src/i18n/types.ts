import type { Language } from "../ipc";

export type { Language };

export const SUPPORTED_LANGUAGES: readonly Language[] = ["ja", "en"] as const;

/** Whether `value` is one of {@link SUPPORTED_LANGUAGES}. */
export function isLanguage(value: string): value is Language {
  return SUPPORTED_LANGUAGES.some((lang) => lang === value);
}
