import { Check, ChevronRight } from "lucide-react";
import { motion } from "motion/react";
import { useCallback, useEffect, useState } from "react";
import type { ConceptLine } from "../bindings/ConceptLine";
import type { SummaryView } from "../bindings/SummaryView";
import { ConceptMark } from "../components/ConceptMark";
import { bead, button as motionButton, pop, popStage, stage } from "../motion";
import { call } from "../rpc";
import { ConceptSheet } from "../study/ConceptSheet";

const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? "" : "s"}`;

// When the next recall is, in plain words (the app works out the days).
export function whenDue(days: number | null): string | null {
  if (days === null) return null;
  if (days <= 0) return "now";
  if (days === 1) return "tomorrow";
  return `in ${days} days`;
}

// The end of a set: what you know, what to learn first, and when things come
// back. A starting point reads as finding where you are, never as a score.
export function Summary({ attemptIds }: { attemptIds: string[] }) {
  const [s, setS] = useState<SummaryView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const load = useCallback(() => {
    call({ type: "get_summary", attempt_ids: attemptIds })
      .then(setS)
      .catch((e: Error) => setError(e.message));
  }, [attemptIds]);
  useEffect(load, [load]);
  const close = useCallback(() => {
    setOpen(null);
    load();
  }, [load]);

  if (error) return <p className="text-sm text-danger">Couldn't read where things stand: {error}</p>;
  if (!s) return null;
  const when = whenDue(s.next_due_in_days);
  return (
    <motion.section
      variants={stage(0.14, 0.05)}
      initial="hidden"
      animate="shown"
      exit="gone"
      className="flex flex-col gap-5"
      aria-label="Summary"
    >
      <motion.div variants={stage(0.08, 0)} className="flex flex-col gap-1.5">
        <motion.h2 variants={pop} className="text-lg font-semibold tracking-tight text-ink">
          {s.first_set ? "Where you're starting" : "Where things stand"}
        </motion.h2>
        <motion.p variants={pop} className="text-sm text-ink-2">
          {framing(s)}
        </motion.p>
      </motion.div>
      <Group title="You know these" lines={s.held} />
      <Group title="Learn these first" lines={s.slipped} onOpen={setOpen} />
      <motion.div variants={pop} className="flex flex-col gap-1 border-t border-hairline pt-4 text-xs text-ink-3">
        {s.outcomes.map((o) => (
          <p key={o.outcome_id}>
            Outcome {o.ord} · {plural(Number(o.concepts), "concept")}
            {o.recalled + o.retained > 0 && ` · ${o.recalled + o.retained} recalled`}
          </p>
        ))}
        {when && <p className="text-ink-2">Next recall: {when}</p>}
      </motion.div>
      <motion.a
        variants={motionButton}
        whileHover={{ y: -1 }}
        href="#/"
        className="w-fit rounded-md border border-accent/40 bg-accent/15 px-3 py-1.5 text-sm font-medium text-accent transition-colors hover:bg-accent/25"
      >
        Back to Today
      </motion.a>
      <ConceptSheet conceptId={open} onClose={close} />
    </motion.section>
  );
}

function framing(s: SummaryView): string {
  const known = s.held.length;
  const toLearn = s.slipped.length;
  if (s.first_set) {
    if (known === 0)
      return "None of these were familiar yet. That's what the starting point is for. Read something for each, and they come back tomorrow.";
    if (toLearn === 0) return "You knew all of these already. They'll come back in a few days to check they've stuck.";
    return `You already know ${known}. Read up on the other ${toLearn}; they all come back as recall.`;
  }
  if (toLearn === 0) return "Everything held.";
  return `${plural(known, "concept")} held, ${toLearn} slipped. The ones that slipped come back tomorrow.`;
}

function Group({
  title,
  lines,
  onOpen,
}: {
  title: string;
  lines: ConceptLine[];
  /** Rows open the concept to learn it. */
  onOpen?: (conceptId: string) => void;
}) {
  if (lines.length === 0) return null;
  return (
    <motion.div variants={stage(0.07, 0)} className="flex flex-col gap-2">
      <motion.h3 variants={pop} className="text-[11px] font-semibold uppercase tracking-wide text-ink-3">
        {title}
      </motion.h3>
      <motion.ul
        variants={popStage(0.08, 0.1)}
        className="flex flex-col divide-y divide-hairline overflow-hidden rounded-lg border border-hairline bg-subtle"
      >
        {lines.map((c) => (
          <motion.li key={c.concept_id} variants={pop} className="text-sm text-ink">
            {onOpen ? (
              <button
                type="button"
                onClick={() => onOpen(c.concept_id)}
                className="group flex w-full items-center gap-2.5 px-3 py-2.5 text-left transition-colors hover:bg-raised/40"
              >
                <motion.span variants={bead} className="inline-flex text-warn">
                  <ConceptMark state={c.state} />
                </motion.span>
                <span className="flex-1">{c.concept}</span>
                {c.studied ? (
                  <Check size={14} className="shrink-0 text-success" aria-label="Studied" />
                ) : (
                  <ChevronRight
                    size={14}
                    className="shrink-0 text-ink-4 transition-colors group-hover:text-ink"
                    aria-hidden
                  />
                )}
              </button>
            ) : (
              <span className="flex items-center gap-2.5 px-3 py-2.5">
                <motion.span variants={bead} className="inline-flex text-teal">
                  <ConceptMark state={c.state} />
                </motion.span>
                {c.concept}
              </span>
            )}
          </motion.li>
        ))}
      </motion.ul>
    </motion.div>
  );
}
