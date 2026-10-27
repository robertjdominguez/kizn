import { ArrowRight, Play } from "lucide-react";
import { motion } from "motion/react";
import { useEffect } from "react";
import { ConceptMark } from "../components/ConceptMark";
import { bead, deal, button as landing, pop, popAfter, stage } from "../motion";

// Before a topic's first set: how concepts move from new to stuck, and what
// the starting point is for. Never names a concept (that would give the first
// answers away). "Some misses point deeper" joins with the map (Phase 6).
const LADDER: [string, string, string][] = [
  ["unseen", "Not started", "new to you"],
  ["seen", "Seen", "you've met it"],
  ["recalled", "Recalled", "right once"],
  ["retained", "Stuck", "right again, days later, on a different question"],
];
const TONE: Record<string, string> = {
  unseen: "text-ink-4",
  seen: "text-warn",
  recalled: "text-teal",
  retained: "text-teal",
};

export function Onboarding({
  step,
  count,
  onNext,
  onSkip,
}: {
  step: 0 | 1;
  count: number;
  onNext: () => void;
  onSkip: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Enter" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        onNext();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onNext]);

  const cta =
    "flex items-center gap-1.5 rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-canvas transition-opacity hover:opacity-90";
  return (
    <motion.section
      variants={deal}
      initial="hidden"
      animate="shown"
      exit="gone"
      aria-labelledby="intro-title"
      className="flex flex-col gap-6 rounded-2xl bg-subtle px-7 py-7 ring-1 ring-hairline"
    >
      <motion.p variants={pop} className="text-[11px] font-semibold uppercase tracking-wider text-accent">
        Before you start · {step + 1} of 2
      </motion.p>
      {step === 0 ? (
        <>
          <motion.h2 variants={pop} id="intro-title" className="text-xl font-semibold tracking-tight text-ink">
            Concepts move from new to stuck
          </motion.h2>
          {/* One step at a time: each mark pops in, then its word and line,
              then the next step along. */}
          <motion.ol variants={stage(0.5, 0.25)} className="grid grid-cols-2 gap-x-4 gap-y-5 sm:grid-cols-4">
            {LADDER.map(([state, word, line]) => (
              <motion.li key={state} variants={stage(0.14, 0)} className="flex flex-col gap-1.5">
                <motion.span variants={bead} className={`inline-flex w-fit ${TONE[state]}`}>
                  <ConceptMark state={state} size={22} />
                </motion.span>
                <motion.span variants={pop} className="text-sm font-medium text-ink">
                  {word}
                </motion.span>
                <motion.span variants={pop} className="text-xs text-ink-3">
                  {line}
                </motion.span>
              </motion.li>
            ))}
          </motion.ol>
          <motion.p variants={popAfter(2.3)} className="text-sm text-ink-2">
            kizn brings each concept back just before you'd forget it, each time with a different question. An outcome
            is ready when its concepts have stuck and you can use them together.
          </motion.p>
        </>
      ) : (
        <>
          <motion.h2 variants={pop} id="intro-title" className="text-xl font-semibold tracking-tight text-ink">
            First, find where you're starting
          </motion.h2>
          <motion.p variants={stage(0.06, 0.1)} className="flex items-center gap-1.5 text-ink-4">
            {Array.from({ length: count }, (_, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: positional placeholders
              <motion.span key={i} variants={bead} className="inline-flex">
                <ConceptMark state="unseen" size={18} />
              </motion.span>
            ))}
          </motion.p>
          <motion.p variants={pop} className="text-sm text-ink-2">
            {count} questions on things you may not have learned yet. Misses are fine: they show kizn where to begin,
            and you'll get something to read for each one you miss.
          </motion.p>
        </>
      )}
      <motion.div variants={landing} className="flex items-center gap-3">
        <button type="button" onClick={onNext} className={cta}>
          {step === 0 ? (
            <>
              Next <ArrowRight size={14} aria-hidden />
            </>
          ) : (
            <>
              <Play size={14} aria-hidden /> Begin
            </>
          )}
        </button>
        <span className="text-xs text-ink-4">or press Enter</span>
        <span className="flex-1" />
        {step === 0 && (
          <button
            type="button"
            onClick={onSkip}
            className="rounded-md px-2 py-1 text-xs text-ink-3 transition-colors hover:bg-raised hover:text-ink"
          >
            Skip
          </button>
        )}
      </motion.div>
    </motion.section>
  );
}
