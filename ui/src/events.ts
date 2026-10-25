import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Event } from "./bindings/Event";

// kizn-app's pushes: Tauri events ("kizn") in the window, server-sent events
// from the bridge (/events) in the browser loop. Returns an unsubscribe.
export function subscribe(onEvent: (e: Event) => void): () => void {
  if (isTauri()) {
    const off = listen<Event>("kizn", (e) => onEvent(e.payload));
    return () => {
      off.then((f) => f());
    };
  }
  const source = new EventSource("/events");
  source.onmessage = (m) => {
    try {
      onEvent(JSON.parse(m.data) as Event);
    } catch {
      // Not ours; ignore.
    }
  };
  return () => source.close();
}
