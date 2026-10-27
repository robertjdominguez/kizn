import { Layers, Plus, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useState } from "react";
import type { LinkedTopic } from "../bindings/LinkedTopic";
import type { TopicSummary } from "../bindings/TopicSummary";
import { pop } from "../motion";
import { call, type TopicView } from "../rpc";

// Where a topic sits on the map: what it rests on (its foundations), what
// rests on it, its parts and what it's part of. Links are added by picking
// another topic; kizn refuses one that would make a loop, and says why.
export function MapLinks({ topic: t }: { topic: TopicView }) {
  const [adding, setAdding] = useState<"rests_on" | "part_of" | null>(null);
  const [others, setOthers] = useState<TopicSummary[]>([]);
  const [error, setError] = useState<string | null>(null);

  const open = (kind: "rests_on" | "part_of") => {
    setError(null);
    setAdding(kind);
    call({ type: "get_today" })
      .then((today) => {
        const linked = new Set([...t.rests_on, ...t.supports, ...t.parts, ...t.part_of].map((l) => l.topic_id));
        setOthers(today.topics.filter((o) => o.id !== t.id && o.status === "confirmed" && !linked.has(o.id)));
      })
      .catch((e: Error) => setError(e.message));
  };

  const link = (other: string) => {
    if (!adding) return;
    // A foundation goes below this topic; so does a part (this topic is part
    // of the one picked, which goes above).
    const [upper_id, lower_id] = adding === "rests_on" ? [t.id, other] : [other, t.id];
    call({ type: "link_topics", upper_id, lower_id, kind: adding })
      .then(() => setAdding(null))
      .catch((e: Error) => setError(e.message));
  };

  const unlink = (l: LinkedTopic) => {
    call({ type: "unlink", link_id: l.link_id }).catch((e: Error) => setError(e.message));
  };

  // Foundations set aside for later (from the check or a flag): dashed
  // suggestions, to start or dismiss.
  const suggested = t.foundations.filter((f) => f.status === "later" && !f.topic_id);
  const resolve = (id: string, action: "start" | "dismiss") => {
    call({ type: "resolve_foundation", suggestion_id: id, action })
      .then((started) => {
        if (action === "start") window.location.hash = `#/setup/${encodeURIComponent(started.id)}`;
      })
      .catch((e: Error) => setError(e.message));
  };

  const groups: [string, LinkedTopic[]][] = [
    ["Rests on", t.rests_on],
    ["Supports", t.supports],
    ["Made up of", t.parts],
    ["Part of", t.part_of],
  ];
  return (
    <motion.section variants={pop} aria-label="On your map" className="flex flex-col gap-2">
      <p className="flex items-center gap-1.5 text-xs text-ink-4">
        <Layers size={12} aria-hidden /> On your map
      </p>
      {groups
        .filter(([, ls]) => ls.length > 0)
        .map(([label, ls]) => (
          <div key={label} className="flex flex-col gap-0.5">
            <p className="text-[11px] text-ink-4">{label}</p>
            {ls.map((l) => (
              <div key={l.link_id} className="group flex items-center gap-1.5">
                <a
                  href={`#/topic/${encodeURIComponent(l.topic_id)}`}
                  className="min-w-0 flex-1 truncate text-sm text-ink-2 hover:text-ink"
                >
                  {l.title}
                </a>
                <button
                  type="button"
                  onClick={() => unlink(l)}
                  aria-label={`Unlink ${l.title}`}
                  className="shrink-0 text-ink-4 opacity-0 transition-opacity hover:text-ink group-hover:opacity-100 focus-visible:opacity-100"
                >
                  <X size={12} aria-hidden />
                </button>
              </div>
            ))}
          </div>
        ))}
      {suggested.length > 0 && (
        <div className="flex flex-col gap-0.5">
          <p className="text-[11px] text-ink-4">Suggested underneath</p>
          {suggested.map((f) => (
            <div key={f.id} className="group flex items-center gap-1.5">
              <span
                className="min-w-0 flex-1 truncate border-b border-dashed border-ink-4/50 text-sm text-ink-3"
                title={f.why ?? undefined}
              >
                {f.title}
              </span>
              <button
                type="button"
                onClick={() => resolve(f.id, "start")}
                className="shrink-0 text-[11px] text-accent hover:underline"
              >
                Start it
              </button>
              <button
                type="button"
                onClick={() => resolve(f.id, "dismiss")}
                aria-label={`Dismiss ${f.title}`}
                className="shrink-0 text-ink-4 opacity-0 transition-opacity hover:text-ink group-hover:opacity-100 focus-visible:opacity-100"
              >
                <X size={12} aria-hidden />
              </button>
            </div>
          ))}
        </div>
      )}
      <div className="flex flex-wrap gap-1.5">
        {(
          [
            ["rests_on", "Foundation"],
            ["part_of", "Part of…"],
          ] as const
        ).map(([kind, label]) => (
          <button
            key={kind}
            type="button"
            onClick={() => (adding === kind ? setAdding(null) : open(kind))}
            aria-pressed={adding === kind}
            className={`flex items-center gap-1 rounded-md px-2 py-1 text-xs ring-1 transition-colors ${adding === kind ? "bg-raised text-ink ring-hairline-strong" : "text-ink-3 ring-hairline hover:bg-raised hover:text-ink"}`}
          >
            <Plus size={11} aria-hidden /> {label}
          </button>
        ))}
      </div>
      <AnimatePresence>
        {adding && (
          <motion.div
            key="pick"
            variants={pop}
            initial="hidden"
            animate="shown"
            exit="gone"
            role="listbox"
            aria-label={adding === "rests_on" ? "Pick a foundation" : "Pick what it's part of"}
            className="flex flex-col overflow-hidden rounded-lg ring-1 ring-hairline"
          >
            {others.length === 0 ? (
              <p className="px-2 py-1.5 text-xs text-ink-4">No other topics to link yet.</p>
            ) : (
              others.map((o) => (
                <button
                  key={o.id}
                  type="button"
                  role="option"
                  aria-selected={false}
                  onClick={() => link(o.id)}
                  className="truncate px-2 py-1.5 text-left text-sm text-ink-2 transition-colors hover:bg-raised hover:text-ink"
                >
                  {o.title}
                </button>
              ))
            )}
          </motion.div>
        )}
      </AnimatePresence>
      <AnimatePresence>
        {error && (
          <motion.p key="err" variants={pop} initial="hidden" animate="shown" exit="gone" className="text-xs text-warn">
            {error.charAt(0).toUpperCase() + error.slice(1)}.
          </motion.p>
        )}
      </AnimatePresence>
    </motion.section>
  );
}
