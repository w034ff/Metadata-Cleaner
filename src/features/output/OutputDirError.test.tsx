import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AppStateProvider, createInitialAppState } from "../../state";
import { OutputDirError } from "./OutputDirError";

describe("OutputDirError", () => {
  it("renders nothing when job has no error", () => {
    const { container } = render(
      <AppStateProvider>
        <OutputDirError />
      </AppStateProvider>,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing when job error is not related to output directory", () => {
    const state = createInitialAppState("ja");
    const stateWithError = {
      ...state,
      job: {
        ...state.job,
        error: { code: "UnsupportedFormat" as const, detail: null },
      },
    };

    const { container } = render(
      <AppStateProvider initialState={stateWithError}>
        <OutputDirError />
      </AppStateProvider>,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders error display when job has SameFolderAsSource error", () => {
    const state = createInitialAppState("ja");
    const stateWithError = {
      ...state,
      job: {
        ...state.job,
        error: { code: "SameFolderAsSource" as const, detail: null },
      },
    };

    render(
      <AppStateProvider initialState={stateWithError}>
        <OutputDirError />
      </AppStateProvider>,
    );

    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText(/元のファイルと同じフォルダ/)).toBeInTheDocument();
  });

  it("renders English message when language is en", () => {
    const state = createInitialAppState("en");
    const stateWithError = {
      ...state,
      job: {
        ...state.job,
        error: { code: "SameFolderAsSource" as const, detail: null },
      },
    };

    render(
      <AppStateProvider initialState={stateWithError}>
        <OutputDirError />
      </AppStateProvider>,
    );

    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(
      screen.getByText(/Choose a folder other than the one the files are in/),
    ).toBeInTheDocument();
  });
});
