import type { Language } from "../ipc";

export type { Language };

export const SUPPORTED_LANGUAGES: readonly Language[] = ["ja", "en"] as const;
