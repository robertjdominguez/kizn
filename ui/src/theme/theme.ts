// The theme follows the OS unless the learner picks one. The choice is a
// viewer convenience kept in localStorage (it can be missing or blocked, so
// every access is guarded), never app data.

export type ThemeChoice = "system" | "light" | "dark";

const KEY = "kizn.theme";

export function loadTheme(): ThemeChoice {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

export function applyTheme(choice: ThemeChoice) {
  const root = document.documentElement;
  if (choice === "system") delete root.dataset.theme;
  else root.dataset.theme = choice;
  try {
    if (choice === "system") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, choice);
  } catch {
    // Not remembered; still applied.
  }
}
