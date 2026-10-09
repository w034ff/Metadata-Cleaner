import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AppStateProvider, createInitialAppState } from "../../state";
import { OutputDirField } from "./OutputDirField";

const jaState = createInitialAppState("ja");

afterEach(() => {
  clearMocks();
});

describe("OutputDirField", () => {
  it("renders unselected state when value is null", () => {
    render(
      <AppStateProvider initialState={jaState}>
        <OutputDirField value={null} onChange={vi.fn()} />
      </AppStateProvider>,
    );

    expect(screen.getByTestId("output-dir-name")).toHaveTextContent("未選択");
    expect(
      screen.getByRole("button", { name: "フォルダを選ぶ" }),
    ).not.toBeDisabled();
  });

  it("renders folder name when value is provided", () => {
    render(
      <AppStateProvider initialState={jaState}>
        <OutputDirField
          value={{ dirLabel: "export_photos" }}
          onChange={vi.fn()}
        />
      </AppStateProvider>,
    );

    expect(screen.getByTestId("output-dir-name")).toHaveTextContent(
      "export_photos",
    );
  });

  it("disables button when disabled prop is true", () => {
    render(
      <AppStateProvider initialState={jaState}>
        <OutputDirField value={null} onChange={vi.fn()} disabled={true} />
      </AppStateProvider>,
    );

    expect(
      screen.getByRole("button", { name: "フォルダを選ぶ" }),
    ).toBeDisabled();
  });

  it("calls onChange when a directory is chosen", async () => {
    mockIPC((cmd) => {
      if (cmd === "pick_output_dir") {
        return { dirLabel: "my_chosen_dir" };
      }
      return null;
    });

    const onChange = vi.fn();
    render(
      <AppStateProvider initialState={jaState}>
        <OutputDirField value={null} onChange={onChange} />
      </AppStateProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "フォルダを選ぶ" }));

    await waitFor(() => {
      expect(onChange).toHaveBeenCalledWith({ dirLabel: "my_chosen_dir" });
    });
  });

  it("does not call onChange when picker is cancelled", async () => {
    mockIPC((cmd) => {
      if (cmd === "pick_output_dir") {
        return null;
      }
      return null;
    });

    const onChange = vi.fn();
    render(
      <AppStateProvider initialState={jaState}>
        <OutputDirField value={null} onChange={onChange} />
      </AppStateProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "フォルダを選ぶ" }));

    // Wait for async handler
    await waitFor(() => {
      expect(onChange).not.toHaveBeenCalled();
    });
  });

  it("displays error message if pickOutputDir fails", async () => {
    mockIPC((cmd) => {
      if (cmd === "pick_output_dir") {
        throw { code: "WriteFailed", detail: null };
      }
      return null;
    });

    render(
      <AppStateProvider initialState={jaState}>
        <OutputDirField value={null} onChange={vi.fn()} />
      </AppStateProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "フォルダを選ぶ" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
  });
});
