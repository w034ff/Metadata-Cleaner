import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AppStateProvider, useAppState } from "../../state";
import { useEnsureOutputDir } from "./useEnsureOutputDir";

afterEach(() => {
  clearMocks();
});

function createWrapper() {
  return function Wrapper({ children }: { children: ReactNode }) {
    return <AppStateProvider>{children}</AppStateProvider>;
  };
}

describe("useEnsureOutputDir", () => {
  it("runs onConfirmed directly when outputDir is already set", async () => {
    const { result } = renderHook(() => useEnsureOutputDir(), {
      wrapper: createWrapper(),
    });

    const onConfirmed = vi.fn();
    await act(async () => {
      await result.current.runWithOutputDir(
        { dirLabel: "existing" },
        onConfirmed,
      );
    });

    expect(onConfirmed).toHaveBeenCalledTimes(1);
  });

  it("prompts for outputDir when none is set and executes onConfirmed on selection", async () => {
    mockIPC((cmd) => {
      if (cmd === "pick_output_dir") {
        return { dirLabel: "chosen_folder" };
      }
      return null;
    });

    const { result } = renderHook(
      () => {
        const ensure = useEnsureOutputDir();
        const state = useAppState();
        return { ensure, state };
      },
      { wrapper: createWrapper() },
    );

    const onConfirmed = vi.fn();
    await act(async () => {
      await result.current.ensure.runWithOutputDir(null, onConfirmed);
    });

    expect(onConfirmed).toHaveBeenCalledTimes(1);
    expect(result.current.state.outputDir).toEqual({
      dirLabel: "chosen_folder",
    });
  });

  it("does not call onConfirmed when dialog is cancelled", async () => {
    mockIPC((cmd) => {
      if (cmd === "pick_output_dir") {
        return null;
      }
      return null;
    });

    const { result } = renderHook(() => useEnsureOutputDir(), {
      wrapper: createWrapper(),
    });

    const onConfirmed = vi.fn();
    await act(async () => {
      await result.current.runWithOutputDir(null, onConfirmed);
    });

    expect(onConfirmed).not.toHaveBeenCalled();
  });

  it("dispatches error when pickOutputDir fails", async () => {
    mockIPC((cmd) => {
      if (cmd === "pick_output_dir") {
        throw { code: "SameFolderAsSource", detail: null };
      }
      return null;
    });

    const { result } = renderHook(
      () => {
        const ensure = useEnsureOutputDir();
        const state = useAppState();
        return { ensure, state };
      },
      { wrapper: createWrapper() },
    );

    const onConfirmed = vi.fn();
    await act(async () => {
      await result.current.ensure.runWithOutputDir(null, onConfirmed);
    });

    expect(onConfirmed).not.toHaveBeenCalled();
    expect(result.current.state.job.error).toEqual({
      code: "SameFolderAsSource",
      detail: null,
    });
  });
});
