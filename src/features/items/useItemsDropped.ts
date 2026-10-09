import { useEffect } from "react";
import { onItemsDropped, type ItemsDropped, type UnlistenFn } from "../../ipc";
import { useAppDispatch } from "../../state";

export interface UseItemsDroppedOptions {
  onDropped?: (payload: ItemsDropped) => void;
}

/**
 * Listens for the `items-dropped` event from the backend and adds newly
 * inspected items to the app state (design §7.2).
 */
export function useItemsDropped(options?: UseItemsDroppedOptions): void {
  const dispatch = useAppDispatch();

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let unmounted = false;

    async function subscribe() {
      const u = await onItemsDropped((payload) => {
        if (payload.added.length > 0) {
          dispatch({ type: "ADD_ITEMS", items: payload.added });
        }
        options?.onDropped?.(payload);
      });
      if (unmounted) {
        try {
          u();
        } catch {
          // Internals may be torn down in tests
        }
      } else {
        unlisten = u;
      }
    }

    void subscribe();

    return () => {
      unmounted = true;
      if (unlisten !== null) {
        try {
          unlisten();
        } catch {
          // Internals may be torn down in tests
        }
      }
    };
  }, [dispatch, options]);
}
