import { clearMocks } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { FileItem } from "../../ipc";
import { AppStateProvider, createInitialAppState } from "../../state";
import { JobFooter } from "./JobFooter";

afterEach(() => {
  clearMocks();
});

const sampleItems: FileItem[] = [
  {
    id: 1,
    name: "photo.jpg",
    format: "jpeg",
    bytes: 1024,
    kinds: ["location"],
    error: null,
  },
];

describe("JobFooter", () => {
  it("renders disabled state when there are no items", () => {
    const state = createInitialAppState("ja");
    render(
      <AppStateProvider initialState={state}>
        <JobFooter />
      </AppStateProvider>,
    );

    expect(screen.getByText("ファイルがありません")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "消して保存" })).toBeDisabled();
  });

  it("renders item count and enabled button when items are present", () => {
    const baseState = createInitialAppState("ja");
    const state = {
      ...baseState,
      items: sampleItems,
    };

    render(
      <AppStateProvider initialState={state}>
        <JobFooter />
      </AppStateProvider>,
    );

    expect(
      screen.getByText("1 件から情報を消して保存します"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "消して保存" }),
    ).not.toBeDisabled();
  });

  it("renders progress bar and cancel button when running", () => {
    const baseState = createInitialAppState("ja");
    const state = {
      ...baseState,
      items: sampleItems,
      job: {
        ...baseState.job,
        phase: "running" as const,
        targets: [{ id: 1, name: "photo.jpg" }],
        progress: { done: 0, current: "photo.jpg", total: 1 },
      },
    };

    render(
      <AppStateProvider initialState={state}>
        <JobFooter />
      </AppStateProvider>,
    );

    expect(screen.getByRole("progressbar")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "キャンセル" }),
    ).not.toBeDisabled();
  });

  it("renders disabled cancel button when cancelling", () => {
    const baseState = createInitialAppState("ja");
    const state = {
      ...baseState,
      items: sampleItems,
      job: {
        ...baseState.job,
        phase: "cancelling" as const,
        targets: [{ id: 1, name: "photo.jpg" }],
        progress: { done: 0, current: "photo.jpg", total: 1 },
      },
    };

    render(
      <AppStateProvider initialState={state}>
        <JobFooter />
      </AppStateProvider>,
    );

    expect(
      screen.getByRole("button", { name: "キャンセル中…" }),
    ).toBeDisabled();
  });

  it("displays failure summary after job finishes with errors", () => {
    const baseState = createInitialAppState("ja");
    const state = {
      ...baseState,
      items: sampleItems,
      job: {
        ...baseState.job,
        phase: "finished" as const,
        finished: {
          succeeded: 0,
          failed: 1,
          unprocessed: 0,
          cancelled: false,
        },
      },
    };

    render(
      <AppStateProvider initialState={state}>
        <JobFooter />
      </AppStateProvider>,
    );

    expect(screen.getByText(/失敗があります/)).toBeInTheDocument();
  });
});
