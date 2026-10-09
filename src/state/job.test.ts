import { describe, expect, it } from "vitest";
import type { JobFinishedPayload, JobItemPayload } from "../ipc";
import {
  initialJobState,
  isJobActive,
  jobReducer,
  jobRowStatus,
  runningIds,
  type JobAction,
  type JobState,
  type JobTarget,
} from "./job";

const targets: JobTarget[] = [
  { id: 1, name: "a.jpg" },
  { id: 2, name: "b.png" },
  { id: 3, name: "c.pdf" },
];

const finished: JobFinishedPayload = {
  succeeded: 1,
  failed: 0,
  unprocessed: 2,
  cancelled: true,
};

function okItem(id: number): JobItemPayload {
  return { id, status: "ok", savedName: null, removed: ["location"] };
}

function progress(done: number, current: string | null): JobAction {
  return { type: "JOB_PROGRESS", progress: { done, total: 3, current } };
}

function reduce(actions: JobAction[], from = initialJobState): JobState {
  return actions.reduce(jobReducer, from);
}

const started: JobAction = {
  type: "JOB_STARTED",
  targets,
};

describe("jobReducer", () => {
  it("starts a job afresh, dropping previous state", () => {
    const previous = reduce([
      started,
      progress(0, "a.jpg"),
      { type: "JOB_ITEM", item: okItem(1) },
      { type: "JOB_FINISHED", finished },
    ]);
    const state = jobReducer(previous, {
      type: "JOB_STARTED",
      targets: [{ id: 9, name: "x.jpg" }],
    });

    expect(state).toEqual({
      ...initialJobState,
      phase: "running",
      targets: [{ id: 9, name: "x.jpg" }],
    });
  });

  it("marks named item as started from progress", () => {
    const state = reduce([started, progress(0, "a.jpg"), progress(0, "b.png")]);

    expect(state.progress).toEqual({ done: 0, total: 3, current: "b.png" });
    expect(runningIds(state)).toEqual([1, 2]);
  });

  it("does not start duplicate item when same name is already running", () => {
    const state = reduce([
      started,
      progress(0, "a.jpg"),
      progress(1, null),
      progress(1, "a.jpg"),
    ]);

    expect(runningIds(state)).toEqual([1]);
  });

  it("handles cancellation request", () => {
    const state = reduce([started, { type: "JOB_CANCEL_REQUESTED" }]);
    expect(state.phase).toBe("cancelling");
    expect(isJobActive(state)).toBe(true);
  });

  it("records job item results and removes from runningIds", () => {
    const state = reduce([
      started,
      progress(0, "a.jpg"),
      { type: "JOB_ITEM", item: okItem(1) },
    ]);
    expect(state.results[1]).toEqual(okItem(1));
    expect(runningIds(state)).toEqual([]);
  });

  it("marks finished on JOB_FINISHED", () => {
    const state = reduce([started, { type: "JOB_FINISHED", finished }]);
    expect(state.phase).toBe("finished");
    expect(state.finished).toEqual(finished);
    expect(isJobActive(state)).toBe(false);
  });

  it("handles JOB_FAILED", () => {
    const err = { code: "WriteFailed" as const, detail: null };
    const state = reduce([started, { type: "JOB_FAILED", error: err }]);
    expect(state.phase).toBe("idle");
    expect(state.error).toEqual(err);
  });

  it("handles JOB_NOT_STARTED", () => {
    const err = { code: "SameFolderAsSource" as const, detail: null };
    const state = reduce([{ type: "JOB_NOT_STARTED", error: err }]);
    expect(state.phase).toBe("idle");
    expect(state.error).toEqual(err);
  });

  it("computes jobRowStatus correctly across phases", () => {
    let state = reduce([started]);
    expect(jobRowStatus(state, 1)).toBe("waiting");

    state = reduce([progress(0, "a.jpg")], state);
    expect(jobRowStatus(state, 1)).toBe("running");

    state = reduce([{ type: "JOB_ITEM", item: okItem(1) }], state);
    expect(jobRowStatus(state, 1)).toBe("ok");

    state = reduce([{ type: "JOB_FINISHED", finished }], state);
    expect(jobRowStatus(state, 2)).toBe("unprocessed");
    expect(jobRowStatus(state, 999)).toBe(null);
  });
});
