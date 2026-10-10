import { formatJobSummary, isJobSuccessful, jobOutcome } from "./jobSummary";

describe("jobOutcome", () => {
  it("returns success when no failures and not cancelled", () => {
    expect(
      jobOutcome({
        succeeded: 5,
        failed: 0,
        unprocessed: 0,
        cancelled: false,
      }),
    ).toBe("success");
  });

  it("returns failure when there are failures", () => {
    expect(
      jobOutcome({
        succeeded: 4,
        failed: 1,
        unprocessed: 0,
        cancelled: false,
      }),
    ).toBe("failure");
  });

  it("returns cancelled when cancelled without failures (unprocessed not counted as failure)", () => {
    expect(
      jobOutcome({
        succeeded: 2,
        failed: 0,
        unprocessed: 3,
        cancelled: true,
      }),
    ).toBe("cancelled");
  });

  it("returns failure when cancelled but has failures", () => {
    expect(
      jobOutcome({
        succeeded: 2,
        failed: 1,
        unprocessed: 2,
        cancelled: true,
      }),
    ).toBe("failure");
  });
});

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
    expect(summary).toBe("保存 4 件 · 失敗 1 件 · 未処理 1 件");
  });

  it("formats cancelled summary in Japanese", () => {
    const summary = formatJobSummary(
      { succeeded: 2, failed: 0, unprocessed: 3, cancelled: true },
      "ja",
    );
    expect(summary).toBe("キャンセルしました · 保存 2 件 · 未処理 3 件");
  });

  it("formats summary in English", () => {
    const summary = formatJobSummary(
      { succeeded: 4, failed: 1, unprocessed: 0, cancelled: false },
      "en",
    );
    expect(summary).toBe("4 saved · 1 failed");
  });

  it("includes unreadable count when greater than zero", () => {
    const jaSummary = formatJobSummary(
      { succeeded: 4, failed: 1, unprocessed: 0, cancelled: false },
      "ja",
      1,
    );
    expect(jaSummary).toBe("保存 4 件 · 失敗 1 件 · 対象外 1 件");

    const enSummary = formatJobSummary(
      { succeeded: 4, failed: 1, unprocessed: 0, cancelled: false },
      "en",
      1,
    );
    expect(enSummary).toBe("4 saved · 1 failed · 1 excluded");
  });

  it("omits unreadable count when zero", () => {
    const summary = formatJobSummary(
      { succeeded: 4, failed: 0, unprocessed: 0, cancelled: false },
      "ja",
      0,
    );
    expect(summary).toBe("保存 4 件");
    expect(summary).not.toContain("対象外");
  });

  it("considers job unsuccessful when unreadableCount > 0", () => {
    expect(
      isJobSuccessful(
        { succeeded: 4, failed: 0, unprocessed: 0, cancelled: false },
        1,
      ),
    ).toBe(false);
  });
});
