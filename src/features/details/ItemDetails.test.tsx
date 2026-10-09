import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Details, FileItem } from "../../ipc";
import {
  AppStateProvider,
  createInitialAppState,
  type AppState,
} from "../../state";
import { ItemDetails } from "./ItemDetails";

const sampleItem: FileItem = {
  id: 1,
  name: "photo.jpg",
  format: "jpeg",
  bytes: 3.2 * 1024 * 1024,
  kinds: ["location", "device"],
  error: null,
};

const errorItem: FileItem = {
  id: 2,
  name: "契約書-署名済み.pdf",
  format: null,
  bytes: 1.1 * 1024 * 1024,
  kinds: [],
  error: { code: "PdfSigned", detail: null },
};

describe("ItemDetails", () => {
  it("renders single hint line when no item is selected", () => {
    const state = createInitialAppState("ja");

    render(
      <AppStateProvider initialState={state}>
        <ItemDetails />
      </AppStateProvider>,
    );

    expect(
      screen.getByText("写っているもの、PDF の本文、ファイル名は消せません"),
    ).toBeInTheDocument();
  });

  it("renders metadata details, fields, bytes, counts, other name, and kept info", () => {
    const details: Details = {
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
        {
          kind: "other",
          entries: [
            {
              field: "other",
              name: "CustomTag",
              value: { type: "bytes", value: 256 },
            },
          ],
        },
        {
          kind: "history",
          entries: [
            {
              field: "earlierVersions",
              name: null,
              value: { type: "count", value: 3 },
            },
          ],
        },
      ],
      kept: [
        { type: "orientation", value: 6 },
        { type: "colorProfile", description: "sRGB" },
      ],
      truncated: true,
    };

    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [sampleItem],
      selectedId: 1,
      details,
    };

    render(
      <AppStateProvider initialState={state}>
        <ItemDetails />
      </AppStateProvider>,
    );

    // Title and meta
    expect(screen.getByText("photo.jpg")).toBeInTheDocument();
    expect(screen.getByText(/JPEG · 3.2 MB/)).toBeInTheDocument();

    // Location group title styled with location text color
    const locTitle = screen.getByText("位置情報");
    expect(locTitle).toHaveStyle({ color: "var(--color-location-text)" });
    expect(screen.getByText("緯度")).toBeInTheDocument();
    expect(screen.getByText("12°34′56″ N")).toBeInTheDocument();

    // Field::Other uses name
    expect(screen.getByText("CustomTag")).toBeInTheDocument();
    expect(screen.getByText("256 バイト")).toBeInTheDocument();

    // Count value and earlierVersions field name
    expect(screen.getByText("追記保存")).toBeInTheDocument();
    expect(
      screen.getByText("3 回分の以前の内容が残っています"),
    ).toBeInTheDocument();

    // Truncated indicator
    expect(screen.getByText("…")).toBeInTheDocument();

    // Kept info
    expect(screen.getByText("残す情報")).toBeInTheDocument();
    expect(
      screen.getByText("向き（右に 90 度回転）、色のプロファイル（sRGB）"),
    ).toBeInTheDocument();
  });

  it("renders 'no metadata found' message when groups is empty and kept is empty", () => {
    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [sampleItem],
      selectedId: 1,
      details: { groups: [], kept: [], truncated: false },
    };

    render(
      <AppStateProvider initialState={state}>
        <ItemDetails />
      </AppStateProvider>,
    );

    expect(
      screen.getByText("メタデータは見つかりませんでした"),
    ).toBeInTheDocument();
    expect(screen.queryByText("残す情報")).not.toBeInTheDocument();
  });

  it("renders specific reason for PdfSigned error without calling get_details", () => {
    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [errorItem],
      selectedId: 2,
      details: null,
    };

    render(
      <AppStateProvider initialState={state}>
        <ItemDetails />
      </AppStateProvider>,
    );

    expect(screen.getByText("契約書-署名済み.pdf")).toBeInTheDocument();
    expect(
      screen.getByText(
        /電子署名付きの PDF は扱えません。消すと署名が無効になるためです/,
      ),
    ).toBeInTheDocument();
  });

  it("renders post-processing result view with status, savedName, removed list, and kept info", () => {
    const details: Details = {
      groups: [
        {
          kind: "location",
          entries: [
            {
              field: "latitude",
              name: null,
              value: { type: "text", value: "12°34′56″ N" },
            },
            {
              field: "longitude",
              name: null,
              value: { type: "text", value: "65°43′21″ E" },
            },
          ],
        },
        {
          kind: "device",
          entries: [
            {
              field: "cameraModel",
              name: null,
              value: { type: "text", value: "Cam X1" },
            },
          ],
        },
      ],
      kept: [{ type: "colorProfile", description: "sRGB" }],
      truncated: false,
    };

    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [sampleItem],
      selectedId: 1,
      details,
      job: {
        ...createInitialAppState("ja").job,
        phase: "finished",
        finished: { succeeded: 1, failed: 0, unprocessed: 0, cancelled: false },
        results: {
          1: {
            id: 1,
            status: "ok",
            savedName: "photo (1).jpg",
            removed: ["location", "device"],
          },
        },
      },
    };

    render(
      <AppStateProvider initialState={state}>
        <ItemDetails />
      </AppStateProvider>,
    );

    // Status: ✓ 完了
    expect(screen.getByText("✓ 完了")).toBeInTheDocument();

    // Saved name section
    expect(screen.getByText("保存した名前")).toBeInTheDocument();
    expect(screen.getByText("photo (1).jpg")).toBeInTheDocument();

    // Removed information list with field names joined by ・
    expect(screen.getByText("消した情報")).toBeInTheDocument();
    expect(screen.getByText(/緯度・経度/)).toBeInTheDocument();
    expect(screen.getByText(/カメラの機種/)).toBeInTheDocument();

    // Kept information section (見出し「残した情報」)
    expect(screen.getByText("残した情報")).toBeInTheDocument();
    expect(screen.getByText("色のプロファイル（sRGB）")).toBeInTheDocument();
  });

  it("renders failed item post-processing without removed information section", () => {
    const state: AppState = {
      ...createInitialAppState("ja"),
      items: [sampleItem],
      selectedId: 1,
      details: null,
      job: {
        ...createInitialAppState("ja").job,
        phase: "finished",
        finished: { succeeded: 0, failed: 1, unprocessed: 0, cancelled: false },
        results: {
          1: {
            id: 1,
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
        <ItemDetails />
      </AppStateProvider>,
    );

    expect(screen.getByText("✕ 失敗")).toBeInTheDocument();
    expect(
      screen.getByText(/ファイルの書き込みに失敗しました/),
    ).toBeInTheDocument();
    expect(screen.queryByText("消した情報")).not.toBeInTheDocument();
    expect(screen.queryByText("保存した名前")).not.toBeInTheDocument();
  });
});
