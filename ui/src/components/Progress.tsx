import type { LucideIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { EASE } from "../motion";

// The segmented progress line above a step-by-step flow (connecting a model,
// starting a topic): a milestone icon and label per step over a rounded
// segment. All or nothing: a segment fills only when its step is done.

export type ProgressStep = { label: string; icon: LucideIcon };

/** How long one segment takes to fill; the next waits for it. */
const FILL_SECONDS = 0.45;

export function Progress({ steps, index }: { steps: ProgressStep[]; index: number }) {
  const last = index === steps.length - 1;
  // Segments fill one at a time: when several become done at once (Test and
  // Ready on the last step), each waits for the one before it. Going back
  // empties them straight away.
  const target = last ? steps.length : index;
  const [filled, setFilled] = useState(target);
  // Where filling started for this move: the first new segment fills at
  // once, each after it waits for the one before.
  const from = useRef(filled);
  // biome-ignore lint/correctness/useExhaustiveDependencies: only when the target moves
  useEffect(() => {
    from.current = filled;
  }, [target]);
  useEffect(() => {
    if (filled > target) {
      setFilled(target);
      return;
    }
    if (filled === target) return;
    const t = setTimeout(() => setFilled((f) => f + 1), filled === from.current ? 0 : FILL_SECONDS * 1000);
    return () => clearTimeout(t);
  }, [filled, target]);
  return (
    <ol aria-label="Progress" className="grid gap-1.5" style={{ gridTemplateColumns: `repeat(${steps.length}, 1fr)` }}>
      {steps.map((s, i) => {
        const done = i < filled;
        const current = i === index && !last;
        const Icon = s.icon;
        return (
          // Keyed by position on purpose: a step's label can change (API key
          // / Install / Address) while its segment stays put.
          // biome-ignore lint/suspicious/noArrayIndexKey: segments are positional
          <li key={i} aria-current={current ? "step" : undefined} className="flex flex-col gap-2">
            <span
              className={`flex items-center gap-1.5 text-xs transition-colors ${done || current ? "text-ink" : "text-ink-4"}`}
            >
              <motion.span
                animate={{ scale: done ? [1, 1.18, 1] : 1 }}
                transition={{ duration: 0.3, ease: EASE }}
                className={`flex size-5 items-center justify-center rounded-full border transition-colors ${
                  done
                    ? "border-accent bg-accent text-canvas"
                    : current
                      ? "border-accent text-accent"
                      : "border-hairline-strong text-ink-4"
                }`}
              >
                <Icon size={11} aria-hidden />
              </motion.span>
              <AnimatePresence mode="wait" initial={false}>
                <motion.span
                  key={s.label}
                  initial={{ opacity: 0, y: 3 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -3 }}
                  transition={{ duration: 0.15 }}
                  className="truncate"
                >
                  {s.label}
                </motion.span>
              </AnimatePresence>
            </span>
            <span className="h-1.5 overflow-hidden rounded-full bg-raised">
              <motion.span
                className="block h-full rounded-full bg-accent"
                initial={false}
                animate={{ width: done ? "100%" : "0%" }}
                transition={{ duration: FILL_SECONDS, ease: EASE }}
              />
            </span>
          </li>
        );
      })}
    </ol>
  );
}
