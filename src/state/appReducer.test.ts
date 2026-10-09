import { describe, expect, it } from "vitest";
import type { FileItem } from "../ipc";
import { appReducer, createInitialAppState } from "./appReducer";

const sampleItem: FileItem = {
  id: 1,
  name: "photo.jpg",
  format: "jpeg",
  bytes: 1024,
  kinds: ["location"],
  error: null,
};

describe("appReducer", () => {
  it("initializes state from defaults or settings", () => {
    const defaultState = createInitialAppState("ja");
    expect(defaultState.language.language).toBe("ja");
    expect(defaultState.items).toEqual([]);
    expect(defaultState.outputDir).toBe(null);

    const configuredState = createInitialAppState("ja", {
      language: "en",
      outputDir: { dirLabel: "my_folder" },
    });
    expect(configuredState.language.language).toBe("en");
    expect(configuredState.outputDir).toEqual({ dirLabel: "my_folder" });
  });

  it("handles items actions and drops stale results", () => {
    let state = createInitialAppState();
    state = appReducer(state, { type: "ADD_ITEMS", items: [sampleItem] });
    expect(state.items).toEqual([sampleItem]);

    // Simulate running then finished job
    state = appReducer(state, {
      type: "JOB_STARTED",
      targets: [{ id: sampleItem.id, name: sampleItem.name }],
    });
    state = appReducer(state, {
      type: "JOB_FINISHED",
      finished: { succeeded: 1, failed: 0, unprocessed: 0, cancelled: false },
    });
    expect(state.job.finished).not.toBe(null);

    // Changing items drops stale job result
    state = appReducer(state, { type: "REMOVE_ITEMS", ids: [1] });
    expect(state.items).toEqual([]);
    expect(state.job.finished).toBe(null);
  });

  it("handles outputDir actions and drops stale results", () => {
    let state = createInitialAppState();
    state = appReducer(state, {
      type: "JOB_STARTED",
      targets: [{ id: sampleItem.id, name: sampleItem.name }],
    });
    state = appReducer(state, {
      type: "JOB_FINISHED",
      finished: { succeeded: 1, failed: 0, unprocessed: 0, cancelled: false },
    });
    expect(state.job.finished).not.toBe(null);

    state = appReducer(state, {
      type: "SET_OUTPUT_DIR",
      outputDir: { dirLabel: "new_dir" },
    });
    expect(state.outputDir).toEqual({ dirLabel: "new_dir" });
    expect(state.job.finished).toBe(null);
  });
});
