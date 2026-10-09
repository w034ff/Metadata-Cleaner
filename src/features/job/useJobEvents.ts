import { useEffect } from "react";
import {
  onJobFinished,
  onJobItem,
  onJobProgress,
  type UnlistenFn,
} from "../../ipc";
import { useAppDispatch } from "../../state";

/**
 * Listens for job events from the Rust backend and dispatches them into state (design §7.2).
 */
export function useJobEvents(): void {
  const dispatch = useAppDispatch();

  useEffect(() => {
    let unmounted = false;
    const unlistens: UnlistenFn[] = [];

    async function subscribe() {
      try {
        const [uProgress, uItem, uFinished] = await Promise.all([
          onJobProgress((progress) => {
            dispatch({ type: "JOB_PROGRESS", progress });
          }),
          onJobItem((item) => {
            dispatch({ type: "JOB_ITEM", item });
          }),
          onJobFinished((finished) => {
            dispatch({ type: "JOB_FINISHED", finished });
          }),
        ]);

        if (unmounted) {
          try {
            uProgress();
            uItem();
            uFinished();
          } catch {
            // Internals may be torn down in tests
          }
          return;
        }

        unlistens.push(uProgress, uItem, uFinished);
      } catch {
        // Event registration failure
      }
    }

    void subscribe();

    return () => {
      unmounted = true;
      for (const unlisten of unlistens) {
        try {
          unlisten();
        } catch {
          // Internals may be torn down in tests
        }
      }
    };
  }, [dispatch]);
}
