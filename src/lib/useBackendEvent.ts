import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

export const BackendEvent = {
  DOWNLOADS_CHANGED: "downloads:changed",
  LIBRARY_CHANGED: "library:changed",
  ENTRIES_CHANGED: "entries:changed",
  CONFIG_CHANGED: "config:changed",
  TASKS_CHANGED: "tasks:changed",
} as const;

export type BackendEventName = (typeof BackendEvent)[keyof typeof BackendEvent];

/**
 * Listen to a Tauri backend event and invoke a callback on each occurrence.
 * Callback is stored in a ref so the listener never re-subscribes.
 */
export function useBackendEvent(eventName: BackendEventName, callback: () => void): void {
  const cbRef = useRef(callback);

  useEffect(() => {
    cbRef.current = callback;
  });

  useEffect(() => {
    const unlisten = listen(eventName, () => cbRef.current());
    return () => { unlisten.then((f) => f()); };
  }, [eventName]);
}
