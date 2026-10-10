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

  it("handles item selection and toggling off", () => {
    let state = createInitialAppState();
    state = appReducer(state, { type: "ADD_ITEMS", items: [sampleItem] });
    expect(state.selectedId).toBe(null);

    // Select item 1
    state = appReducer(state, { type: "SELECT_ITEM", id: 1 });
    expect(state.selectedId).toBe(1);

    // Select item 1 again -> toggles off
    state = appReducer(state, { type: "SELECT_ITEM", id: 1 });
    expect(state.selectedId).toBe(null);

    // Select item 1 again, then pass null -> deselects
    state = appReducer(state, { type: "SELECT_ITEM", id: 1 });
    expect(state.selectedId).toBe(1);
    state = appReducer(state, { type: "SELECT_ITEM", id: null });
    expect(state.selectedId).toBe(null);
  });

  it("handles details loading and drops stale responses", () => {
    let state = createInitialAppState();
    state = appReducer(state, { type: "ADD_ITEMS", items: [sampleItem] });

    // Start request 1
    state = appReducer(state, {
      type: "FETCH_DETAILS_START",
      id: 1,
      requestId: 1,
    });
    expect(state.detailsRequestId).toBe(1);
    expect(state.isLoadingDetails).toBe(true);

    // Switch to request 2
    state = appReducer(state, {
      type: "FETCH_DETAILS_START",
      id: 2,
      requestId: 2,
    });
    expect(state.detailsRequestId).toBe(2);

    // Old response for request 1 arrives -> ignored
    state = appReducer(state, {
      type: "FETCH_DETAILS_SUCCESS",
      requestId: 1,
      details: { groups: [], kept: [], truncated: false },
    });
    expect(state.details).toBe(null);

    // Response for request 2 arrives -> accepted
    const details = { groups: [], kept: [], truncated: false };
    state = appReducer(state, {
      type: "FETCH_DETAILS_SUCCESS",
      requestId: 2,
      details,
    });
    expect(state.details).toEqual(details);
    expect(state.isLoadingDetails).toBe(false);
  });

  it("drops an answer that arrives after the selection changed", () => {
    const details = { groups: [], kept: [], truncated: false };
    let state = createInitialAppState();
    state = appReducer(state, { type: "ADD_ITEMS", items: [sampleItem] });
    state = appReducer(state, { type: "SELECT_ITEM", id: sampleItem.id });
    state = appReducer(state, {
      type: "FETCH_DETAILS_START",
      id: sampleItem.id,
      requestId: 1,
    });

    // Deselected while the answer for request 1 was still on its way.
    state = appReducer(state, { type: "SELECT_ITEM", id: null });
    state = appReducer(state, {
      type: "FETCH_DETAILS_SUCCESS",
      requestId: 1,
      details,
    });
    expect(state.details).toBe(null);

    // Another row selected that needs no request (e.g. an error row).
    state = appReducer(state, { type: "SELECT_ITEM", id: sampleItem.id });
    state = appReducer(state, {
      type: "FETCH_DETAILS_SUCCESS",
      requestId: 1,
      details,
    });
    expect(state.details).toBe(null);
  });

  it("resets selection when selected item is removed or cleared", () => {
    let state = createInitialAppState();
    state = appReducer(state, { type: "ADD_ITEMS", items: [sampleItem] });
    state = appReducer(state, { type: "SELECT_ITEM", id: 1 });
    expect(state.selectedId).toBe(1);

    state = appReducer(state, { type: "REMOVE_ITEMS", ids: [1] });
    expect(state.selectedId).toBe(null);

    // With clear items
    state = appReducer(state, { type: "ADD_ITEMS", items: [sampleItem] });
    state = appReducer(state, { type: "SELECT_ITEM", id: 1 });
    expect(state.selectedId).toBe(1);

    state = appReducer(state, { type: "CLEAR_ITEMS" });
    expect(state.selectedId).toBe(null);
  });

  it("preserves finished job results when ADD_ITEMS adds no items (e.g. duplicates only)", () => {
    let state = createInitialAppState();
    state = appReducer(state, { type: "ADD_ITEMS", items: [sampleItem] });

    // Simulate running then finished job
    state = appReducer(state, {
      type: "JOB_STARTED",
      targets: [{ id: sampleItem.id, name: sampleItem.name }],
    });
    const finishedResult = {
      succeeded: 1,
      failed: 0,
      unprocessed: 0,
      cancelled: false,
    };
    state = appReducer(state, {
      type: "JOB_FINISHED",
      finished: finishedResult,
    });
    expect(state.job.finished).toEqual(finishedResult);

    // Adding no items (e.g. duplicate skipped) updates lastSkipped and preserves job.finished
    state = appReducer(state, {
      type: "ADD_ITEMS",
      items: [],
      skipped: { folders: 0, unsupported: 0, duplicates: 1 },
    });
    expect(state.items).toEqual([sampleItem]);
    expect(state.lastSkipped).toEqual({
      folders: 0,
      unsupported: 0,
      duplicates: 1,
    });
    expect(state.job.finished).toEqual(finishedResult);

    // Adding actual items drops stale job result
    state = appReducer(state, {
      type: "ADD_ITEMS",
      items: [{ ...sampleItem, id: 2 }],
    });
    expect(state.items.length).toBe(2);
    expect(state.job.finished).toBe(null);
  });
});
