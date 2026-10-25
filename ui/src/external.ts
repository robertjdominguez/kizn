import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

// Opens a link in the learner's browser. In the window that goes through the
// opener plugin, which only allows the sites in capabilities/default.json.
export function openExternal(url: string) {
  if (isTauri()) {
    openUrl(url).catch(() => undefined);
  } else {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}
