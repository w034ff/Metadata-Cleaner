import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import * as ipcModule from "../../ipc";
import {
  AppStateProvider,
  createInitialAppState,
  type AppState,
} from "../../state";
import { ItemList } from "./ItemList";

const sampleItems: ipcModule.FileItem[] = [
  {
    id: 1,
    name: "photo.jpg",
    format: "jpeg",
    bytes: 3.2 * 1024 * 1024,
    kinds: ["location", "dateTime", "device"],
    error: null,
  },
  {
    id: 2,
    name: "契約書-署名済み.pdf",
    format: null,
    bytes: 1.1 * 1024 * 1024,
    kinds: [],
    error: { code: "PdfSigned", detail: null },
  },
  {
    id: 3,
    name: "banner.webp",
    format: "webp",
    bytes: 88 * 1024,
    kinds: [],
    error: null,
  },
];

describe("ItemList", () => {
  it("renders header with total count and 3 action buttons", () => {
    const state: AppState = {
      ...createInitialAppState("ja"),
      items: sampleItems,
    };

    render(
      <AppStateProvider initialState={state}>
        <ItemList />
      </AppStateProvider>,
    );

    expect(screen.getByText("ファイル 3 件")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "ファイルを追加" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "フォルダを追加" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "すべて外す" }),
    ).toBeInTheDocument();
  });

  it("renders 5 fixed table headers that do not change before or after processing", () => {
    const state: AppState = {
      ...createInitialAppState("ja"),
      items: sampleItems,
    };

    const { rerender } = render(
      <AppStateProvider initialState={state}>
        <ItemList />
      </AppStateProvider>,
    );

    const headersBefore = screen
      .getAllByRole("columnheader")
      .map((th) => th.textContent);
    expect(headersBefore).toEqual([
      "ファイル名",
      "形式",
      "大きさ",
      "見つかった情報",
      "状態",
    ]);

    // After processing
    const finishedState: AppState = {
      ...state,
      job: {
        ...state.job,
        phase: "finished",
        finished: { succeeded: 2, failed: 0, unprocessed: 0, cancelled: false },
        results: {
          1: { id: 1, status: "ok", savedName: null, removed: ["location"] },
        },
      },
    };

    rerender(
      <AppStateProvider initialState={finishedState}>
        <ItemList />
      </AppStateProvider>,
    );

    const headersAfter = screen
      .getAllByRole("columnheader")
      .map((th) => th.textContent);
    expect(headersAfter).toEqual([
      "ファイル名",
      "形式",
      "大きさ",
      "見つかった情報",
      "状態",
    ]);
  });

  it("displays uppercase extension when format is null", () => {
    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [sampleItems[1]],
    };

    render(
      <AppStateProvider initialState={state}>
        <ItemList />
      </AppStateProvider>,
    );

    expect(screen.getByText("PDF")).toBeInTheDocument();
  });

  it("renders location chip with chip-loc class and pin symbol, and no metadata text when kinds is empty", () => {
    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [sampleItems[0], sampleItems[2]],
    };

    render(
      <AppStateProvider initialState={state}>
        <ItemList />
      </AppStateProvider>,
    );

    const locChip = screen.getByText("位置情報").closest(".chip");
    expect(locChip).toHaveClass("chip-loc");
    expect(locChip?.querySelector("svg")).toBeInTheDocument();

    expect(screen.getByText("メタデータなし")).toBeInTheDocument();
  });

  it("renders symbols and statuses properly: waiting, error, ok, failed", () => {
    const state: AppState = {
      ...createInitialAppState("ja"),
      items: sampleItems,
      job: {
        ...createInitialAppState("ja").job,
        phase: "finished",
        finished: { succeeded: 1, failed: 1, unprocessed: 0, cancelled: false },
        results: {
          1: { id: 1, status: "ok", savedName: null, removed: [] },
          3: {
            id: 3,
            status: "failed",
            savedName: null,
            removed: [],
            error: { code: "WriteFailed", detail: null },
          },
        },
      },
    };

    render(
      <AppStateProvider initialState={state}>
        <ItemList />
      </AppStateProvider>,
    );

    // Item 1: ok
    expect(screen.getByText("✓ 完了")).toBeInTheDocument();
    // Item 2: load failed
    expect(screen.getByText("✕ 読み込めません")).toBeInTheDocument();
    expect(
      screen.getByText("電子署名付きの PDF は扱えません"),
    ).toBeInTheDocument();
    // Item 3: failed
    expect(screen.getByText("✕ 失敗")).toBeInTheDocument();
    expect(
      screen.getByText("ファイルの書き込みに失敗しました"),
    ).toBeInTheDocument();
  });

  it("disables list action buttons during processing", () => {
    const state: AppState = {
      ...createInitialAppState("ja"),
      items: sampleItems,
      job: {
        ...createInitialAppState("ja").job,
        phase: "running",
      },
    };

    render(
      <AppStateProvider initialState={state}>
        <ItemList />
      </AppStateProvider>,
    );

    expect(
      screen.getByRole("button", { name: "ファイルを追加" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "フォルダを追加" }),
    ).toBeDisabled();
    expect(screen.getByRole("button", { name: "すべて外す" })).toBeDisabled();
  });

  it("calls removeItems with all IDs and clears items on clear all", async () => {
    const removeSpy = vi
      .spyOn(ipcModule, "removeItems")
      .mockResolvedValue(undefined);

    const state: AppState = {
      ...createInitialAppState("ja"),
      items: sampleItems,
    };

    render(
      <AppStateProvider initialState={state}>
        <ItemList />
      </AppStateProvider>,
    );

    const clearBtn = screen.getByRole("button", { name: "すべて外す" });
    await act(async () => {
      fireEvent.click(clearBtn);
    });

    expect(removeSpy).toHaveBeenCalledWith([1, 2, 3]);
  });

  it("selects a row from any cell and from its name, once per click", async () => {
    const getDetails = vi
      .spyOn(ipcModule, "getDetails")
      .mockResolvedValue({ groups: [], kept: [], truncated: false });
    const state: AppState = {
      ...createInitialAppState("ja"),
      items: sampleItems,
    };

    render(
      <AppStateProvider initialState={state}>
        <ItemList />
      </AppStateProvider>,
    );

    // A cell other than the name selects the row.
    await act(async () => {
      fireEvent.click(screen.getByText("WebP"));
    });
    expect(getDetails).toHaveBeenCalledTimes(1);
    expect(getDetails).toHaveBeenLastCalledWith(3);

    // The name button selects its row and is not handled twice.
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "photo.jpg" }));
    });
    expect(getDetails).toHaveBeenCalledTimes(2);
    expect(getDetails).toHaveBeenLastCalledWith(1);

    getDetails.mockRestore();
  });
});
