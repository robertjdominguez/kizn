import { Monitor, Moon, Sun } from "lucide-react";
import { useState } from "react";
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

export function App() {
  return (
    <div className="flex h-full flex-col">
      <header className="flex h-11 shrink-0 items-center gap-2 border-b border-hairline px-4">
        <span className="text-sm font-semibold tracking-tight text-ink">kizn</span>
        <span className="text-sm text-ink-4">Today</span>
        <span className="flex-1" />
        <ThemeToggle />
      </header>
      <main className="min-h-0 flex-1 overflow-auto">
        <Today />
      </main>
    </div>
  );
}
