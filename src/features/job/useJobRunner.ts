import { useCallback } from "react";
import {
  cancelJob as cancelJobIpc,
  normalizeIpcError,
  startClean,
} from "../../ipc";
import { isJobActive, useAppDispatch, useAppState } from "../../state";
import { useEnsureOutputDir } from "../output";

export interface JobRunnerResult {
  startJob: () => Promise<void>;
  cancelJob: () => Promise<void>;
  isRunning: boolean;
  isCancelling: boolean;
  isPickingOutputDir: boolean;
}

/**
 * Coordinates starting and cancelling a cleaning job, ensuring an output directory
 * is selected first (design §6.5, §7.1).
 */
export function useJobRunner(): JobRunnerResult {
  const { items, outputDir, job } = useAppState();
  const dispatch = useAppDispatch();
  const { runWithOutputDir, isPicking } = useEnsureOutputDir();

  const startJob = useCallback(async () => {
    if (isJobActive(job) || items.length === 0) {
      return;
    }

    await runWithOutputDir(outputDir, async () => {
      const targets = items.map((it) => ({ id: it.id, name: it.name }));
      const ids = items.map((it) => it.id);

      dispatch({ type: "JOB_STARTED", targets });

      try {
        await startClean(ids);
      } catch (err: unknown) {
        dispatch({
          type: "JOB_NOT_STARTED",
          error: normalizeIpcError(err),
        });
      }
    });
  }, [job, items, outputDir, runWithOutputDir, dispatch]);

  const cancelJob = useCallback(async () => {
    if (job.phase !== "running") {
      return;
    }

    dispatch({ type: "JOB_CANCEL_REQUESTED" });

    try {
      await cancelJobIpc();
    } catch (err: unknown) {
      dispatch({
        type: "JOB_FAILED",
        error: normalizeIpcError(err),
      });
    }
  }, [job.phase, dispatch]);

  return {
    startJob,
    cancelJob,
    isRunning: job.phase === "running",
    isCancelling: job.phase === "cancelling",
    isPickingOutputDir: isPicking,
  };
}
