import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it } from "vitest";
import type { FileItem } from "../../ipc";
import {
  AppStateProvider,
  createInitialAppState,
  useAppState,
} from "../../state";
import { useJobRunner } from "./useJobRunner";

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
  {
    id: 2,
    name: "doc.pdf",
    format: "pdf",
    bytes: 2048,
    kinds: ["author"],
    error: null,
  },
];

function createWrapper(
  items: FileItem[] = sampleItems,
  outputDirLabel: string | null = "my_out",
) {
  const baseState = createInitialAppState("ja");
  const initialState = {
    ...baseState,
    items,
    outputDir: outputDirLabel !== null ? { dirLabel: outputDirLabel } : null,
  };
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <AppStateProvider initialState={initialState}>
        {children}
      </AppStateProvider>
    );
  };
}

describe("useJobRunner", () => {
  it("does not start job when items list is empty", async () => {
    let cleanCalled = false;
    mockIPC((cmd) => {
      if (cmd === "start_clean") {
        cleanCalled = true;
      }
      return null;
    });

    const { result } = renderHook(() => useJobRunner(), {
      wrapper: createWrapper([]),
    });

    await act(async () => {
      await result.current.startJob();
    });

    expect(cleanCalled).toBe(false);
  });

  it("starts clean job with items ids when outputDir is present", async () => {
    let receivedIds: unknown = null;
    mockIPC((cmd, args) => {
      if (cmd === "start_clean" && typeof args === "object" && args !== null) {
        receivedIds = Reflect.get(args, "ids");
      }
      return null;
    });

    const { result } = renderHook(
      () => {
        const runner = useJobRunner();
        const state = useAppState();
        return { runner, state };
      },
      { wrapper: createWrapper() },
    );

    await act(async () => {
      await result.current.runner.startJob();
    });

    expect(receivedIds).toEqual([1, 2]);
    expect(result.current.state.job.phase).toBe("running");
  });

  it("prompts for outputDir if not set before calling start_clean", async () => {
    let receivedIds: unknown = null;
    mockIPC((cmd, args) => {
      if (cmd === "pick_output_dir") {
        return { dirLabel: "picked_folder" };
      }
      if (cmd === "start_clean" && typeof args === "object" && args !== null) {
        receivedIds = Reflect.get(args, "ids");
      }
      return null;
    });

    const { result } = renderHook(
      () => {
        const runner = useJobRunner();
        const state = useAppState();
        return { runner, state };
      },
      { wrapper: createWrapper(sampleItems, null) },
    );

    await act(async () => {
      await result.current.runner.startJob();
    });

    expect(receivedIds).toEqual([1, 2]);
    expect(result.current.state.outputDir).toEqual({
      dirLabel: "picked_folder",
    });
    expect(result.current.state.job.phase).toBe("running");
  });

  it("handles start_clean failure (e.g. SameFolderAsSource)", async () => {
    mockIPC((cmd) => {
      if (cmd === "start_clean") {
        throw { code: "SameFolderAsSource", detail: null };
      }
      return null;
    });

    const { result } = renderHook(
      () => {
        const runner = useJobRunner();
        const state = useAppState();
        return { runner, state };
      },
      { wrapper: createWrapper() },
    );

    await act(async () => {
      await result.current.runner.startJob();
    });

    expect(result.current.state.job.phase).toBe("idle");
    expect(result.current.state.job.error).toEqual({
      code: "SameFolderAsSource",
      detail: null,
    });
  });

  it("handles start_clean failure for OutputDirMissing and OutputDirNotWritable", async () => {
    mockIPC((cmd) => {
      if (cmd === "start_clean") {
        throw { code: "OutputDirMissing", detail: null };
      }
      return null;
    });

    const { result } = renderHook(
      () => {
        const runner = useJobRunner();
        const state = useAppState();
        return { runner, state };
      },
      { wrapper: createWrapper() },
    );

    await act(async () => {
      await result.current.runner.startJob();
    });

    expect(result.current.state.job.phase).toBe("idle");
    expect(result.current.state.job.error).toEqual({
      code: "OutputDirMissing",
      detail: null,
    });
  });

  it("cancels running job", async () => {
    let cancelCalled = false;
    mockIPC((cmd) => {
      if (cmd === "cancel_job") {
        cancelCalled = true;
      }
      return null;
    });

    const baseState = createInitialAppState("ja");
    const runningState = {
      ...baseState,
      items: sampleItems,
      outputDir: { dirLabel: "out" },
      job: {
        ...baseState.job,
        phase: "running" as const,
        targets: sampleItems.map((it) => ({ id: it.id, name: it.name })),
      },
    };

    const wrapper = ({ children }: { children: ReactNode }) => (
      <AppStateProvider initialState={runningState}>
        {children}
      </AppStateProvider>
    );

    const { result } = renderHook(() => useJobRunner(), { wrapper });

    await act(async () => {
      await result.current.cancelJob();
    });

    expect(cancelCalled).toBe(true);
  });
});
