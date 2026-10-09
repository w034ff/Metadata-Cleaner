import { describe, expect, it } from "vitest";
import { formatJobSummary, isJobSuccessful } from "./jobSummary";

describe("jobSummary", () => {
  it("determines whether a job was completely successful", () => {
    expect(
      isJobSuccessful({
        succeeded: 5,
        failed: 0,
        unprocessed: 0,
        cancelled: false,
      }),
    ).toBe(true);

    expect(
      isJobSuccessful({
        succeeded: 4,
        failed: 1,
        unprocessed: 0,
        cancelled: false,
      }),
    ).toBe(false);

    expect(
      isJobSuccessful({
        succeeded: 4,
        failed: 0,
        unprocessed: 1,
        cancelled: false,
      }),
    ).toBe(false);

    expect(
      isJobSuccessful({
        succeeded: 5,
        failed: 0,
        unprocessed: 0,
        cancelled: true,
      }),
    ).toBe(false);
  });

  it("formats completed summary in Japanese", () => {
    const summary = formatJobSummary(
      { succeeded: 4, failed: 1, unprocessed: 1, cancelled: false },
      "ja",
    );
    expect(summary).toBe("終わりました：成功 4 件 · 失敗 1 件 · 未処理 1 件");
  });

  it("formats cancelled summary in Japanese", () => {
    const summary = formatJobSummary(
      { succeeded: 2, failed: 0, unprocessed: 3, cancelled: true },
      "ja",
    );
    expect(summary).toBe("キャンセルしました：成功 2 件 · 未処理 3 件");
  });

  it("formats summary in English", () => {
    const summary = formatJobSummary(
      { succeeded: 4, failed: 1, unprocessed: 0, cancelled: false },
      "en",
    );
    expect(summary).toBe("Finished: 4 succeeded · 1 failed");
  });
});
