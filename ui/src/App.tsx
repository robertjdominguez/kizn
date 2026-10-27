import { Info, Monitor, Moon, Sun } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useState } from "react";
import { About } from "./about/About";
import { type Added, MapView } from "./map/MapView";
import { fade, presence } from "./motion";
import { Practice } from "./practice/Practice";
import { Settings } from "./settings/Settings";
import { applyTheme, loadTheme, type ThemeChoice } from "./theme/theme";
import { Today } from "./today/Today";
import { GroundCheck } from "./topic/GroundCheck";
import { Setup } from "./topic/Setup";
import { TopicPage } from "./topic/TopicPage";

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

// Screens by URL hash (#/settings, #/topic/<id>), so a reload or hot reload
// stays put.
type View =
  | { name: "today" }
  | { name: "settings" }
  | { name: "topic"; id: string; from?: "map" }
  | { name: "setup"; id?: string; foundation?: string }
  | { name: "practice"; id?: string }
  | { name: "check"; id: string }
  | { name: "map"; added?: Added };
const viewFromHash = (): View => {
  // A query after the route: where a topic was opened from, or what was just
  // added to the map.
  const [h = "", query = ""] = window.location.hash.split("?");
  const q = new URLSearchParams(query);
  if (h === "#/settings") return { name: "settings" };
  // From a suggested foundation: new-topic?foundation=<suggestion>.
  if (h === "#/new-topic") return { name: "setup", foundation: q.get("foundation") ?? undefined };
  const setup = h.match(/^#\/setup\/(.+)$/);
  if (setup?.[1]) return { name: "setup", id: decodeURIComponent(setup[1]) };
  if (h === "#/practice") return { name: "practice" };
  if (h === "#/map") {
    // From a flag: added=<title> (and node=<topic> when a topic you have was
    // linked); from a check: arrived=<id>,<id>.
    const under = q.get("under");
    const nodes = [...(q.get("arrived")?.split(",") ?? []), ...(q.get("node") ? [q.get("node") as string] : [])];
    const title = q.get("added") ?? undefined;
    const from = q.get("from") === "check" ? "check" : "flag";
    return {
      name: "map",
      added: under && (title || nodes.length) ? { under, title, nodes: nodes.filter(Boolean), from } : undefined,
    };
  }
  const check = h.match(/^#\/check\/(.+)$/);
  if (check?.[1]) return { name: "check", id: decodeURIComponent(check[1]) };
  const practice = h.match(/^#\/practice\/(.+)$/);
  if (practice?.[1]) return { name: "practice", id: decodeURIComponent(practice[1]) };
  const topic = h.match(/^#\/topic\/(.+)$/);
  if (topic?.[1])
    return { name: "topic", id: decodeURIComponent(topic[1]), from: q.get("from") === "map" ? "map" : undefined };
  return { name: "today" };
};

// Topics live under Today in the nav.
const current = (v: View) =>
  v.name === "settings" ? "settings" : v.name === "map" || (v.name === "topic" && v.from === "map") ? "map" : "today";
const NAV = { today: "#/", map: "#/map", settings: "#/settings" } as const;
const viewKey = (v: View) =>
  v.name === "topic"
    ? `topic-${v.id}`
    : v.name === "practice"
      ? `practice-${v.id ?? ""}`
      : v.name === "check"
        ? `check-${v.id}`
        : v.name;

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
          {(["today", "map", "settings"] as const).map((v) => (
            <a
              key={v}
              href={NAV[v]}
              aria-current={current(view) === v ? "page" : undefined}
              className={`rounded-md px-2 py-1 text-sm capitalize transition-colors ${current(view) === v ? "bg-raised/60 text-ink" : "text-ink-3 hover:text-ink"}`}
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
      <main className="min-h-0 flex-1 overflow-hidden">
        <AnimatePresence mode="wait">
          <motion.div key={viewKey(view)} variants={fade} {...presence} className="h-full">
            {view.name === "today" && <Today />}
            {view.name === "settings" && <Settings />}
            {view.name === "topic" && <TopicPage topicId={view.id} from={view.from} />}
            {view.name === "setup" && <Setup topicId={view.id} foundation={view.foundation} />}
            {view.name === "practice" && <Practice topicId={view.id} />}
            {view.name === "check" && <GroundCheck topicId={view.id} />}
            {view.name === "map" && <MapView added={view.added} />}
          </motion.div>
        </AnimatePresence>
      </main>
      <About open={about} onClose={closeAbout} />
    </div>
  );
}
