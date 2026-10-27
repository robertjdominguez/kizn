import { Layers, Link2 } from "lucide-react";
import { motion } from "motion/react";
import { useState } from "react";
import { EASE } from "../motion";
import { call, type TopicView } from "../rpc";

// In Shape: the foundations kizn thinks the topic rests on, to keep or drop.
// One that's a topic you have is linked when you confirm; new ones get a few
// questions each, to check the ground.
export function FoundationsEditor({ topic }: { topic: TopicView }) {
  const [error, setError] = useState<string | null>(null);
  // The box flips at once; the topic catches up when kizn has saved it.
  const [pending, setPending] = useState<Record<string, boolean>>({});
  const drafted = topic.foundations.filter((f) => f.source === "draft");
  if (drafted.length === 0) return null;
  const toggle = (id: string, keep: boolean) => {
    setError(null);
    setPending((p) => ({ ...p, [id]: keep }));
    call({ type: "keep_foundation", suggestion_id: id, keep }).catch((e: Error) => {
      setError(e.message);
      setPending(({ [id]: _, ...rest }) => rest);
    });
  };
  return (
    <section aria-labelledby="rests-title" className="flex flex-col gap-2 border-t border-hairline pt-4">
      <h3 id="rests-title" className="flex items-center gap-1.5 text-sm font-semibold text-ink">
        <Layers size={14} className="text-ink-3" aria-hidden /> What it rests on
      </h3>
      <p className="text-xs text-ink-3">
        kizn's guess at the more basic topics this builds on. Keep the ones that matter: after you confirm, you'll
        answer a few questions on each new one, to see whether it's solid.
      </p>
      <ul className="flex flex-col gap-1.5">
        {drafted.map((f, i) => {
          const kept = pending[f.id] ?? f.status === "kept";
          return (
            <motion.li
              key={f.id}
              initial={{ opacity: 0, y: 4 }}
              animate={{ opacity: 1, y: 0, transition: { duration: 0.2, ease: EASE, delay: i * 0.05 } }}
              className={`flex items-start gap-3 rounded-lg px-3 py-2 ring-1 transition-colors ${kept ? "bg-canvas ring-hairline" : "ring-transparent opacity-60"}`}
            >
              <input
                type="checkbox"
                checked={kept}
                aria-label={`Keep ${f.title}`}
                onChange={() => toggle(f.id, !kept)}
                className="mt-0.5 size-4 shrink-0 accent-[var(--color-accent)]"
              />
              <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span className="text-sm text-ink">{f.title}</span>
                {f.why && <span className="text-xs text-ink-3">{f.why}</span>}
              </span>
              {f.topic_id && (
                <span className="flex shrink-0 items-center gap-1 text-[11px] text-teal">
                  <Link2 size={11} aria-hidden /> a topic you have
                </span>
              )}
            </motion.li>
          );
        })}
      </ul>
      {error && <p className="text-xs text-danger">{error}</p>}
    </section>
  );
}
