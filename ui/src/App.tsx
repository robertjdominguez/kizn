import { Info, Monitor, Moon, Sun } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { About } from "./about/About";
import { Settings } from "./settings/Settings";
import { applyTheme, loadTheme, type ThemeChoice } from "./theme/theme";
import { Today } from "./today/Today";

const THEMES: { value: ThemeChoice; label: string; Icon: typeof Sun }[] = [
  { value: "system", label: "Follow the system", Icon: Monitor },
  { value: "light", label: "Light", Icon: Sun },
  { value: "dark", label: "Dark", Icon: Moon },
];

function ThemeToggle() {
  const [choice, setChoice] = useState<ThemeChoice>(loadTheme);
  const pick = (c: ThemeChoice) => {
    applyTheme(c);
    setChoice(c);
  };
  return (
    <fieldset className="m-0 flex rounded-md border-0 bg-raised/50 p-0.5" aria-label="Theme">
      {THEMES.map(({ value, label, Icon }) => (
        <button
          key={value}
          type="button"
          aria-pressed={choice === value}
          aria-label={label}
          title={label}
          onClick={() => pick(value)}
          className={`rounded px-1.5 py-1 transition-colors ${choice === value ? "bg-canvas text-ink" : "text-ink-3 hover:text-ink"}`}
        >
          <Icon size={13} aria-hidden />
        </button>
      ))}
    </fieldset>
  );
}

// Screens by URL hash (#/settings), so a reload or hot reload stays put.
type View = "today" | "settings";
const viewFromHash = (): View => (window.location.hash === "#/settings" ? "settings" : "today");

export function App() {
  const [view, setView] = useState<View>(viewFromHash);
  useEffect(() => {
    const onHash = () => setView(viewFromHash());
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);
  const [about, setAbout] = useState(false);
  const closeAbout = useCallback(() => setAbout(false), []);
  return (
    <div className="flex h-full flex-col">
      <header className="flex h-11 shrink-0 items-center gap-2 border-b border-hairline px-4">
        <span className="mr-2 text-sm font-semibold tracking-tight text-ink">kizn</span>
        <nav aria-label="Main" className="flex items-center gap-1">
          {(["today", "settings"] as const).map((v) => (
            <a
              key={v}
              href={v === "today" ? "#/" : "#/settings"}
              aria-current={view === v ? "page" : undefined}
              className={`rounded-md px-2 py-1 text-sm capitalize transition-colors ${view === v ? "bg-raised/60 text-ink" : "text-ink-3 hover:text-ink"}`}
            >
              {v}
            </a>
          ))}
        </nav>
        <span className="flex-1" />
        <ThemeToggle />
        <button
          type="button"
          aria-label="About"
          title="About"
          onClick={() => setAbout(true)}
          className="rounded-md p-1.5 text-ink-3 transition-colors hover:bg-raised hover:text-ink"
        >
          <Info size={14} aria-hidden />
        </button>
      </header>
      <main className="min-h-0 flex-1 overflow-auto">{view === "today" ? <Today /> : <Settings />}</main>
      <About open={about} onClose={closeAbout} />
    </div>
  );
}
