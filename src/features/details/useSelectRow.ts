import { useCallback, useRef } from "react";
import { getDetails, normalizeIpcError } from "../../ipc";
import { useAppDispatch, useAppState } from "../../state";

/**
 * Hook to manage item row selection and fetching metadata details (design §6.2).
 */
export function useSelectRow() {
  const { items, selectedId, job } = useAppState();
  const dispatch = useAppDispatch();
  // Numbered here rather than from the rendered state, so two selections made
  // before the next render still get different numbers.
  const lastRequestId = useRef(0);

  const selectRow = useCallback(
    async (id: number | null) => {
      if (id === null || id === selectedId) {
        dispatch({ type: "SELECT_ITEM", id: null });
        return;
      }

      const item = items.find((it) => it.id === id);
      if (!item) {
        return;
      }

      // If the item had an error during inspection or processing failed, don't call get_details
      const jobResult = job.results[id];
      const isFailed = item.error !== null || jobResult?.status === "failed";

      dispatch({ type: "SELECT_ITEM", id });

      if (isFailed) {
        return;
      }

      lastRequestId.current += 1;
      const requestId = lastRequestId.current;
      dispatch({ type: "FETCH_DETAILS_START", id, requestId });

      try {
        const details = await getDetails(id);
        dispatch({ type: "FETCH_DETAILS_SUCCESS", requestId, details });
      } catch (err: unknown) {
        dispatch({
          type: "FETCH_DETAILS_FAILURE",
          requestId,
          error: normalizeIpcError(err),
        });
      }
    },
    [items, selectedId, job.results, dispatch],
  );

  return { selectedId, selectRow };
}
