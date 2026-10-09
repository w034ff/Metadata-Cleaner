import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { FileItem } from "../../ipc";
import { AppStateProvider, useAppState } from "../../state";
import { useItemsDropped } from "./useItemsDropped";

afterEach(() => {
  clearMocks();
});

const sampleItem: FileItem = {
  id: 1,
  name: "photo.jpg",
  format: "jpeg",
  bytes: 1024,
  kinds: ["location"],
  error: null,
};

function createWrapper() {
  return function Wrapper({ children }: { children: ReactNode }) {
    return <AppStateProvider>{children}</AppStateProvider>;
  };
}

describe("useItemsDropped", () => {
  it("subscribes to items-dropped and dispatches ADD_ITEMS when items are added", async () => {
    mockIPC(() => undefined, { shouldMockEvents: true });

    // Using vi.spyOn on onItemsDropped or mocking the IPC event directly
    const onDropped = vi.fn();
    const { result, unmount } = renderHook(
      () => {
        useItemsDropped({ onDropped });
        return useAppState();
      },
      { wrapper: createWrapper() },
    );

    // Emitting via Tauri's mock event system:
    // With shouldMockEvents: true, window.__TAURI_INTERNALS__.emit or emit can trigger events.
    // Let's import emit from @tauri-apps/api/event
    const { emit } = await import("@tauri-apps/api/event");
    await act(async () => {
      await emit("items-dropped", {
        added: [sampleItem],
        skipped: { count: 0, reason: "Unreadable" },
        error: null,
      });
    });

    await waitFor(() => {
      expect(onDropped).toHaveBeenCalled();
      expect(result.current.items).toEqual([sampleItem]);
    });

    unmount();
  });
});
