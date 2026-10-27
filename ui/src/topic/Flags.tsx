import { CornerLeftDown, Link2, Plus } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useState } from "react";
import type { FlagView } from "../bindings/FlagView";
import { pop } from "../motion";
import { call } from "../rpc";

// Misses that point deeper: a quiet card per flag, with what the learner can
// do about it. kizn notices; the learner decides.
export function Flags({ flags, showTopic = false }: { flags: FlagView[]; showTopic?: boolean }) {
  const [error, setError] = useState<string | null>(null);
  const act = (f: FlagView, action: "add" | "link" | "dismiss") => {
    setError(null);
    call({ type: "resolve_flag", for_topic: f.for_topic, title: f.title, action })
      .then(() => {
        // Added or linked: go and watch it land on the map.
        if (action === "dismiss") return;
        const q = new URLSearchParams({ added: f.title, under: f.for_topic });
        if (action === "link" && f.topic_id) q.set("node", f.topic_id);
        window.location.hash = `#/map?${q}`;
      })
      .catch((e: Error) => setError(e.message));
  };
  return (
    <AnimatePresence initial={false}>
      {flags.length > 0 && (
        <motion.section
          key="flags"
          variants={pop}
          initial="hidden"
          animate="shown"
          exit="gone"
          aria-label="Misses that point deeper"
          className="flex flex-col gap-3 rounded-2xl bg-mauve/[0.06] px-5 py-4 ring-1 ring-mauve/20"
        >
          <p className="flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-mauve">
            <CornerLeftDown size={12} aria-hidden /> Some misses point deeper
          </p>
          <AnimatePresence initial={false}>
            {flags.map((f) => (
              <motion.div
                key={`${f.for_topic}-${f.title}`}
                layout
                variants={pop}
                initial="hidden"
                animate="shown"
                exit="gone"
                className="flex flex-col gap-2"
              >
                <p className="text-sm text-ink-2">
                  {f.misses} misses{showTopic ? ` in ${f.for_title}` : ""} point at{" "}
                  <span className="font-medium text-ink">{f.title}</span>
                  {f.kind === "review"
                    ? ", already one of its foundations: worth a review."
                    : f.kind === "link"
                      ? ", a topic you have."
                      : ", something more basic underneath."}
                </p>
                <div className="flex flex-wrap items-center gap-2">
                  {f.kind === "new" && (
                    <button
                      type="button"
                      onClick={() => act(f, "add")}
                      className="flex items-center gap-1.5 rounded-lg bg-mauve/15 px-3 py-1.5 text-xs font-medium text-mauve hover:bg-mauve/25"
                    >
                      <Plus size={12} aria-hidden /> Add it underneath
                    </button>
                  )}
                  {f.kind === "link" && (
                    <button
                      type="button"
                      onClick={() => act(f, "link")}
                      className="flex items-center gap-1.5 rounded-lg bg-mauve/15 px-3 py-1.5 text-xs font-medium text-mauve hover:bg-mauve/25"
                    >
                      <Link2 size={12} aria-hidden /> Link it underneath
                    </button>
                  )}
                  {f.kind === "review" && f.topic_id && (
                    <a
                      href={`#/topic/${encodeURIComponent(f.topic_id)}`}
                      className="rounded-lg bg-mauve/15 px-3 py-1.5 text-xs font-medium text-mauve hover:bg-mauve/25"
                    >
                      Open {f.title}
                    </a>
                  )}
                  <button
                    type="button"
                    onClick={() => act(f, "dismiss")}
                    className="rounded-lg px-3 py-1.5 text-xs text-ink-3 hover:bg-raised hover:text-ink"
                  >
                    {f.kind === "review" ? "Got it" : "Not now"}
                  </button>
                </div>
              </motion.div>
            ))}
          </AnimatePresence>
          {error && <p className="text-xs text-danger">{error}</p>}
        </motion.section>
      )}
    </AnimatePresence>
  );
}
