import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AppStateProvider, createInitialAppState } from "../../state";
import { JobSummaryBanner } from "./JobSummaryBanner";

const jaState = createInitialAppState("ja");

describe("JobSummaryBanner", () => {
  it("renders nothing when finished is null", () => {
    const { container } = render(
      <AppStateProvider initialState={jaState}>
        <JobSummaryBanner finished={null} />
      </AppStateProvider>,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders success banner when all items succeed", () => {
    render(
      <AppStateProvider initialState={jaState}>
        <JobSummaryBanner
          finished={{
            succeeded: 4,
            failed: 0,
            unprocessed: 0,
            cancelled: false,
          }}
        />
      </AppStateProvider>,
    );

    const banner = screen.getByRole("status");
    expect(banner).toHaveClass("job-summary-banner-success");
    expect(banner).toHaveTextContent("保存 4 件");
    expect(banner).toHaveTextContent("✓");
  });

  it("renders warning banner when some items failed", () => {
    render(
      <AppStateProvider initialState={jaState}>
        <JobSummaryBanner
          finished={{
            succeeded: 4,
            failed: 1,
            unprocessed: 1,
            cancelled: false,
          }}
        />
      </AppStateProvider>,
    );

    const banner = screen.getByRole("status");
    expect(banner).toHaveClass("job-summary-banner-warning");
    expect(banner).toHaveTextContent("保存 4 件 · 失敗 1 件 · 未処理 1 件");
    expect(banner).toHaveTextContent("✕");
  });

  it("includes unreadable count when items have errors and omits it when zero", () => {
    const stateWithErrors = {
      ...jaState,
      items: [
        {
          id: 1,
          name: "corrupt.jpg",
          format: null,
          bytes: 100,
          kinds: [],
          error: { code: "DecodeFailed" as const, detail: null },
        },
      ],
    };

    render(
      <AppStateProvider initialState={stateWithErrors}>
        <JobSummaryBanner
          finished={{
            succeeded: 4,
            failed: 1,
            unprocessed: 0,
            cancelled: false,
          }}
        />
      </AppStateProvider>,
    );

    const banner = screen.getByRole("status");
    expect(banner).toHaveTextContent("保存 4 件 · 失敗 1 件 · 対象外 1 件");
  });
});
