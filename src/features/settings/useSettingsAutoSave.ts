import { useEffect, useRef } from "react";
import { saveSettings, type SettingsInput } from "../../ipc";
import { useAppState } from "../../state";

/**
 * How long the settings wait after a change before they are saved, so a run
 * of changes is written once.
 */
export const SETTINGS_SAVE_DEBOUNCE_MS = 1000;

function sameSettings(a: SettingsInput, b: SettingsInput): boolean {
  return a.language === b.language;
}

/**
 * Saves the settings with `save_settings` once they have stayed unchanged
 * for {@link SETTINGS_SAVE_DEBOUNCE_MS} (design §6.7). What the app started
 * with is not saved again. Output folders are not part of it: `pick_output_dir`
 * saves those itself.
 *
 * A failed save is dropped: the settings still apply to this session, and
 * keeping them for the next one is a Should requirement (FR-10) that must not
 * get in the way of cleaning.
 */
export function useSettingsAutoSave(): void {
  const { language } = useAppState();
  const preference = language.preference;

  const savedRef = useRef<SettingsInput>({
    language: preference,
  });

  useEffect(() => {
    const next: SettingsInput = {
      language: preference,
    };
    if (sameSettings(next, savedRef.current)) {
      return;
    }
    const timer = setTimeout(() => {
      savedRef.current = next;
      saveSettings(next).catch(() => {
        // Dropped on purpose; see the doc comment.
      });
    }, SETTINGS_SAVE_DEBOUNCE_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [preference]);
}
