import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppShell } from "./App";
import * as ipcModule from "./ipc";
import {
  AppStateProvider,
  createInitialAppState,
  type AppState,
} from "./state";

const samplePhoto: ipcModule.FileItem = {
  id: 1,
  name: "photo.jpg",
  format: "jpeg",
  bytes: 3.2 * 1024 * 1024,
  kinds: ["location"],
  error: null,
};

const sampleErrorItem: ipcModule.FileItem = {
  id: 2,
  name: "corrupt.png",
  format: null,
  bytes: 1024,
  kinds: [],
  error: { code: "DecodeFailed", detail: null },
};

const sampleDoc: ipcModule.FileItem = {
  id: 3,
  name: "doc.pdf",
  format: "pdf",
  bytes: 500 * 1024,
  kinds: ["author"],
  error: null,
};

beforeEach(() => {
  mockIPC(() => undefined, { shouldMockEvents: true });
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

describe("T12 Completion Criteria Integration Tests", () => {
  it("criterion 1: allows adding and removing items (追加と外す)", async () => {
    vi.spyOn(ipcModule, "addFiles").mockResolvedValue({
      added: [samplePhoto, sampleErrorItem],
      skipped: { unsupported: 0, folders: 0, duplicates: 0 },
    });
    const removeSpy = vi
      .spyOn(ipcModule, "removeItems")
      .mockResolvedValue(undefined);

    const state = createInitialAppState("ja");
    render(
      <AppStateProvider initialState={state}>
        <AppShell />
      </AppStateProvider>,
    );

    // Initial state: DropZone is shown
    expect(
      screen.getByText("ファイルまたはフォルダをここにドロップ"),
    ).toBeInTheDocument();

    // Click "ファイルを追加"
    const addBtn = screen.getByRole("button", { name: "ファイルを追加" });
    await act(async () => {
      fireEvent.click(addBtn);
    });

    // Both items are added
    expect(await screen.findByText("photo.jpg")).toBeInTheDocument();
    expect(screen.getByText("corrupt.png")).toBeInTheDocument();
    expect(screen.getByText("ファイル 2 件")).toBeInTheDocument();

    // Click "すべて外す"
    const clearBtn = screen.getByRole("button", { name: "すべて外す" });
    await act(async () => {
      fireEvent.click(clearBtn);
    });

    expect(removeSpy).toHaveBeenCalledWith([1, 2]);
    expect(
      await screen.findByText("ファイルまたはフォルダをここにドロップ"),
    ).toBeInTheDocument();
  });

  it("criterion 2: excludes error items from start_clean (エラーの行が処理の対象から外れる)", async () => {
    const startCleanSpy = vi
      .spyOn(ipcModule, "startClean")
      .mockResolvedValue(undefined);

    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [samplePhoto, sampleErrorItem],
      outputDir: { dirLabel: "cleaned" },
    };

    render(
      <AppStateProvider initialState={state}>
        <AppShell />
      </AppStateProvider>,
    );

    // Footer counts only valid items (1 item, not 2)
    expect(
      screen.getByText("1 件から情報を消して保存します"),
    ).toBeInTheDocument();

    const startBtn = screen.getByRole("button", { name: "消して保存" });
    await act(async () => {
      fireEvent.click(startBtn);
    });

    // Only ID 1 was passed to startClean
    expect(startCleanSpy).toHaveBeenCalledWith([1]);
  });

  it("criterion 3: selecting an item invokes get_details and shows details (行の選択で get_details が呼ばれて詳細が出る)", async () => {
    const getDetailsSpy = vi.spyOn(ipcModule, "getDetails").mockResolvedValue({
      groups: [
        {
          kind: "location",
          entries: [
            {
              field: "latitude",
              name: null,
              value: { type: "text", value: "12°34′56″ N" },
            },
          ],
        },
      ],
      kept: [{ type: "colorProfile", description: "sRGB" }],
      truncated: false,
    });

    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [samplePhoto],
    };

    render(
      <AppStateProvider initialState={state}>
        <AppShell />
      </AppStateProvider>,
    );

    const rowBtn = screen.getByRole("button", { name: "photo.jpg" });
    await act(async () => {
      fireEvent.click(rowBtn);
    });

    expect(getDetailsSpy).toHaveBeenCalledWith(1);
    expect(await screen.findByText("12°34′56″ N")).toBeInTheDocument();
    expect(screen.getByText("色のプロファイル（sRGB）")).toBeInTheDocument();
  });

  it("criterion 4: clicking selected row again deselects and restores 1-line hint (同じ行をもう一度押すと選択が外れて 1 行の説明に戻る)", async () => {
    vi.spyOn(ipcModule, "getDetails").mockResolvedValue({
      groups: [],
      kept: [],
      truncated: false,
    });

    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [samplePhoto],
    };

    render(
      <AppStateProvider initialState={state}>
        <AppShell />
      </AppStateProvider>,
    );

    const rowBtn = screen.getByRole("button", { name: "photo.jpg" });

    // Select
    await act(async () => {
      fireEvent.click(rowBtn);
    });
    expect(
      await screen.findByText("メタデータは見つかりませんでした"),
    ).toBeInTheDocument();

    // Deselect
    await act(async () => {
      fireEvent.click(rowBtn);
    });
    expect(
      screen.getByText("写っているもの、PDF の本文、ファイル名は消せません"),
    ).toBeInTheDocument();
  });

  it("criterion 5: drops stale details response when switching rows while waiting (答えを待つ間に別の行を選ぶと古い答えが捨てられる)", async () => {
    let resolveFirst: ((d: ipcModule.Details) => void) | null = null;
    const firstPromise = new Promise<ipcModule.Details>((resolve) => {
      resolveFirst = resolve;
    });

    const secondDetails: ipcModule.Details = {
      groups: [
        {
          kind: "author",
          entries: [
            {
              field: "author",
              name: null,
              value: { type: "text", value: "Target Author" },
            },
          ],
        },
      ],
      kept: [],
      truncated: false,
    };

    vi.spyOn(ipcModule, "getDetails").mockImplementation((id: number) => {
      if (id === 1) {
        return firstPromise;
      }
      return Promise.resolve(secondDetails);
    });

    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [samplePhoto, sampleDoc],
    };

    render(
      <AppStateProvider initialState={state}>
        <AppShell />
      </AppStateProvider>,
    );

    // Click item 1
    const row1Btn = screen.getByRole("button", { name: "photo.jpg" });
    await act(async () => {
      fireEvent.click(row1Btn);
    });

    // Immediately click item 3
    const row3Btn = screen.getByRole("button", { name: "doc.pdf" });
    await act(async () => {
      fireEvent.click(row3Btn);
    });

    // Item 3 details appear
    expect(await screen.findByText("Target Author")).toBeInTheDocument();

    // Now first promise resolves with old details
    await act(async () => {
      if (resolveFirst) {
        resolveFirst({
          groups: [
            {
              kind: "location",
              entries: [
                {
                  field: "latitude",
                  name: null,
                  value: { type: "text", value: "Stale Latitude" },
                },
              ],
            },
          ],
          kept: [],
          truncated: false,
        });
      }
    });

    // Stale latitude should NOT appear
    expect(screen.queryByText("Stale Latitude")).not.toBeInTheDocument();
    expect(screen.getByText("Target Author")).toBeInTheDocument();
  });

  it("criterion 6: table columns remain identical before and after processing (処理の前と後で表の列が同じ)", () => {
    const preState: AppState = {
      ...createInitialAppState("ja"),
      items: [samplePhoto],
    };

    const { rerender } = render(
      <AppStateProvider initialState={preState}>
        <AppShell />
      </AppStateProvider>,
    );

    const colsBefore = screen
      .getAllByRole("columnheader")
      .map((th) => th.textContent);

    const postState: AppState = {
      ...preState,
      job: {
        ...preState.job,
        phase: "finished",
        finished: { succeeded: 1, failed: 0, unprocessed: 0, cancelled: false },
        results: {
          1: { id: 1, status: "ok", savedName: null, removed: ["location"] },
        },
      },
    };

    rerender(
      <AppStateProvider initialState={postState}>
        <AppShell />
      </AppStateProvider>,
    );

    const colsAfter = screen
      .getAllByRole("columnheader")
      .map((th) => th.textContent);
    expect(colsBefore).toEqual(colsAfter);
  });

  it("criterion 7: displays savedName only when present (savedName があるときだけ「保存した名前」が出る)", async () => {
    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [samplePhoto, sampleDoc],
      selectedId: 1,
      job: {
        ...createInitialAppState("ja").job,
        phase: "finished",
        finished: { succeeded: 2, failed: 0, unprocessed: 0, cancelled: false },
        results: {
          1: { id: 1, status: "ok", savedName: "photo (1).jpg", removed: [] },
          3: { id: 3, status: "ok", savedName: null, removed: [] },
        },
      },
    };

    render(
      <AppStateProvider initialState={state}>
        <AppShell />
      </AppStateProvider>,
    );

    // Item 1 has savedName
    expect(screen.getByText("保存した名前")).toBeInTheDocument();
    expect(screen.getByText("photo (1).jpg")).toBeInTheDocument();

    // Click row 3 which has savedName === null
    const row3Btn = screen.getByRole("button", { name: "doc.pdf" });
    await act(async () => {
      fireEvent.click(row3Btn);
    });

    expect(screen.queryByText("保存した名前")).not.toBeInTheDocument();
  });

  it("criterion 8: disables list actions and output folder during processing (処理中に一覧と保存先が無効になる)", () => {
    const runningState: AppState = {
      ...createInitialAppState("ja"),
      items: [samplePhoto],
      job: {
        ...createInitialAppState("ja").job,
        phase: "running",
      },
    };

    render(
      <AppStateProvider initialState={runningState}>
        <AppShell />
      </AppStateProvider>,
    );

    expect(
      screen.getByRole("button", { name: "ファイルを追加" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "フォルダを追加" }),
    ).toBeDisabled();
    expect(screen.getByRole("button", { name: "すべて外す" })).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "フォルダを選ぶ" }),
    ).toBeDisabled();
  });

  it("criterion 9: immediately enters cancelling state when cancel is pressed (キャンセルを押すと直ちに「キャンセル中…」になる)", async () => {
    let cancelResolve: (() => void) | null = null;
    const cancelPromise = new Promise<void>((resolve) => {
      cancelResolve = resolve;
    });
    vi.spyOn(ipcModule, "cancelJob").mockImplementation(() => cancelPromise);

    const runningState: AppState = {
      ...createInitialAppState("ja"),
      items: [samplePhoto],
      job: {
        ...createInitialAppState("ja").job,
        phase: "running",
        targets: [{ id: 1, name: "photo.jpg" }],
        progress: { done: 0, total: 1, current: "photo.jpg" },
      },
    };

    render(
      <AppStateProvider initialState={runningState}>
        <AppShell />
      </AppStateProvider>,
    );

    const cancelBtn = screen.getByRole("button", { name: "キャンセル" });
    await act(async () => {
      fireEvent.click(cancelBtn);
    });

    // Button immediately becomes disabled and changes label to "キャンセル中…"
    expect(
      screen.getByRole("button", { name: "キャンセル中…" }),
    ).toBeDisabled();

    // Clean up promise
    await act(async () => {
      if (cancelResolve) {
        cancelResolve();
      }
    });
  });

  it("criterion 10: renders SameFolderAsSource above the list (SameFolderAsSource が一覧の上に出る)", () => {
    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [samplePhoto],
      job: {
        ...createInitialAppState("ja").job,
        error: { code: "SameFolderAsSource", detail: null },
      },
    };

    render(
      <AppStateProvider initialState={state}>
        <AppShell />
      </AppStateProvider>,
    );

    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(
      screen.getByText(/元のファイルと同じフォルダには保存できません/),
    ).toBeInTheDocument();
  });

  it("criterion 11: renders status indicators with symbols (✓ / ✕) (状態の表示に記号が付く)", () => {
    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [samplePhoto, sampleErrorItem, sampleDoc],
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
            error: { code: "WorkerTimeout", detail: null },
          },
        },
      },
    };

    render(
      <AppStateProvider initialState={state}>
        <AppShell />
      </AppStateProvider>,
    );

    // ok item: ✓ 完了
    expect(screen.getByText("✓ 完了")).toBeInTheDocument();

    // inspection error item: ✕ 読み込めません
    expect(screen.getByText("✕ 読み込めません")).toBeInTheDocument();

    // job failed item: ✕ 失敗
    expect(screen.getByText("✕ 失敗")).toBeInTheDocument();
  });
});
