import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";

beforeEach(() => {
  mockIPC(() => undefined, { shouldMockEvents: true });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("App", () => {
  it("shows app title and language toggle in the header", () => {
    render(<App />);

    expect(
      screen.getByRole("heading", { name: "Metadata Cleaner" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "日本語" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "English" })).toBeInTheDocument();
  });

  it("switches language when language toggle is clicked", () => {
    render(<App />);

    // Default or switch to English
    fireEvent.click(screen.getByRole("button", { name: "English" }));
    expect(screen.getByText("Output folder")).toBeInTheDocument();
    expect(screen.getByText("No files selected")).toBeInTheDocument();
    expect(
      screen.getByText(
        "Visible contents, PDF body text, and file names cannot be removed",
      ),
    ).toBeInTheDocument();

    // Switch back to Japanese
    fireEvent.click(screen.getByRole("button", { name: "日本語" }));
    expect(screen.getByText("保存先フォルダ")).toBeInTheDocument();
    expect(screen.getByText("ファイルがありません")).toBeInTheDocument();
    expect(
      screen.getByText("写っているもの、PDF の本文、ファイル名は消せません"),
    ).toBeInTheDocument();
  });

  it("shows drop zone when file list is empty", () => {
    render(<App />);

    expect(
      screen.getByRole("region", {
        name: /ファイルまたはフォルダをここにドロップ|Drop files or folders here/,
      }),
    ).toBeInTheDocument();
  });

  it("shows hint text in the right aside when nothing is selected", () => {
    render(<App />);

    const aside = screen.getByRole("complementary", {
      name: /選んだファイルの詳細|Details of selected file/,
    });
    expect(aside).toBeInTheDocument();
    expect(
      screen.getByText(
        /写っているもの、PDF の本文、ファイル名は消せません|Visible contents, PDF body text, and file names cannot be removed/,
      ),
    ).toBeInTheDocument();
  });
});
