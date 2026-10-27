import { motion } from "motion/react";
import { EASE } from "../motion";

// A concept's state as a filling circle: ○ not started, ◔ seen, ◑ recalled,
// ● stuck. The fill animates when the state moves.
const FILL: Record<string, number> = { unseen: 0, seen: 0.25, recalled: 0.5, retained: 1 };
export const STATE_WORD: Record<string, string> = {
  unseen: "not started",
  seen: "seen",
  recalled: "recalled",
  retained: "stuck",
};

export function ConceptMark({ state, size = 14 }: { state: string; size?: number }) {
  const f = FILL[state] ?? 0;
  const r = 5;
  // A pie slice from 12 o'clock, clockwise; a full circle when stuck.
  const a = f * 2 * Math.PI;
  const x = 7 + r * Math.sin(a);
  const y = 7 - r * Math.cos(a);
  const slice = f >= 1 ? null : `M7 7 L7 ${7 - r} A${r} ${r} 0 ${f > 0.5 ? 1 : 0} 1 ${x} ${y} Z`;
  return (
    <svg width={size} height={size} viewBox="0 0 14 14" role="img" aria-label={STATE_WORD[state] ?? state}>
      <circle cx="7" cy="7" r="5.5" fill="none" stroke="currentColor" strokeOpacity={0.55} strokeWidth="1.2" />
      {f >= 1 ? (
        <motion.circle
          cx="7"
          cy="7"
          r={r}
          fill="currentColor"
          initial={{ scale: 0 }}
          animate={{ scale: 1 }}
          transition={{ duration: 0.24, ease: EASE }}
        />
      ) : (
        f > 0 &&
        slice && (
          <motion.path
            d={slice}
            fill="currentColor"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.24, ease: EASE }}
          />
        )
      )}
    </svg>
  );
}
