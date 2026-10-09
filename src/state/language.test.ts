import { describe, expect, it } from "vitest";
import { createInitialLanguageState, languageReducer } from "./language";

describe("language state", () => {
  it("initializes from preference when available", () => {
    const state = createInitialLanguageState("ja-JP", "en");
    expect(state.language).toBe("en");
    expect(state.preference).toBe("en");
  });

  it("detects language when preference is null", () => {
    const state = createInitialLanguageState("ja-JP", null);
    expect(state.language).toBe("ja");
    expect(state.preference).toBe(null);

    const stateEn = createInitialLanguageState("en-US", null);
    expect(stateEn.language).toBe("en");
    expect(stateEn.preference).toBe(null);
  });

  it("updates language and preference on SET_LANGUAGE", () => {
    const initial = createInitialLanguageState("ja-JP", null);
    const updated = languageReducer(initial, {
      type: "SET_LANGUAGE",
      language: "en",
    });
    expect(updated.language).toBe("en");
    expect(updated.preference).toBe("en");

    // Idempotent
    const same = languageReducer(updated, {
      type: "SET_LANGUAGE",
      language: "en",
    });
    expect(same).toBe(updated);
  });
});
