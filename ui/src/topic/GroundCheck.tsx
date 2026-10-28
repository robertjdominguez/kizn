import { ArrowRight, Check, ChevronLeft, LoaderCircle } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { GroundFoundation } from "../bindings/GroundFoundation";
import type { GroundQuestion } from "../bindings/GroundQuestion";
import type { GroundView } from "../bindings/GroundView";
import { Screen } from "../components/Screen";
import { Stage } from "../components/Stage";
import { subscribe } from "../events";
import { bead, button as landing, pop, stage } from "../motion";
import { call } from "../rpc";

const LETTERS = ["A", "B", "C", "D", "E"];

// Checking the ground, the last step of setup: one run of a few questions on
// each foundation kept for the topic, in order. Not a test: each comes out
// "seems solid" or "might be shaky", and the evidence sets what happens (a
// shaky one goes on the map to start with), which the learner can change.
export function GroundCheck({ topicId }: { topicId: string }) {
  const [g, setG] = useState<GroundView | null>(null);
  const [error, setError] = useState<string | null>(null);
  // The question just answered, kept up for its feedback until "Next".
  const [showing, setShowing] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const load = useCallback(() => {
    call({ type: "get_ground", topic_id: topicId })
      .then(setG)
      .catch((e: Error) => setError(e.message));
  }, [topicId]);
  useEffect(() => {
    load();
    return subscribe((e) => {
      if (e.type === "topic_changed" && e.topic_id === topicId) load();
    });
  }, [load, topicId]);

  const steps = (g?.foundations ?? []).flatMap((f, fi) => f.questions.map((q) => ({ f, fi, q })));
  const at = showing ? steps.findIndex((s) => s.q.id === showing) : steps.findIndex((s) => s.q.answered === null);
  const step = at >= 0 ? steps[at] : undefined;
  const after = at >= 0 ? steps[at + 1] : undefined;
  // Waiting on a foundation whose questions aren't written yet.
  const waiting = !!g?.writing && (g.foundations.some((f) => f.questions.length === 0) || !step);
  const done = !!g && !step && !waiting && g.foundations.length > 0;

  const answer = (q: GroundQuestion, chosen: number) => {
    setBusy(true);
    setError(null);
    call({ type: "answer_ground", item_id: q.id, order: q.order, chosen })
      .then((next) => {
        setG(next);
        setShowing(q.id);
      })
      .catch((e: Error) => setError(e.message))
      .finally(() => setBusy(false));
  };

  const topicHref = `#/topic/${encodeURIComponent(topicId)}`;
  return (
    <Screen
      width="max-w-2xl"
      header={
        <motion.div variants={pop} initial="hidden" animate="shown" className="flex flex-col gap-1">
          <a href={topicHref} className="flex w-fit items-center gap-0.5 py-1 pr-2 text-sm text-ink-3 hover:text-ink">
            <ChevronLeft size={15} aria-hidden /> {g?.topic_title ?? "Topic"}
          </a>
          <p className="text-[11px] font-semibold uppercase tracking-wider text-accent">Check the ground</p>
          <h1 className="text-2xl font-semibold tracking-tight text-ink">What {g?.topic_title ?? "it"} rests on</h1>
        </motion.div>
      }
    >
      {error && <p className="text-sm text-danger">{error}</p>}
      {g && (
        <Stage className="flex flex-col gap-6" stagger={0.12}>
          <motion.p variants={pop} className="text-sm text-ink-2">
            A few questions on each, while your practice is being prepared. Not a test: they show where the ground seems
            solid, and where to start.
          </motion.p>
          <Progress g={g} current={done ? -1 : (step?.fi ?? -1)} />
          <AnimatePresence mode="wait" initial={false}>
            {done ? (
              <Summary key="summary" g={g} onChange={load} />
            ) : step ? (
              <Card
                key={step.q.id}
                f={step.f}
                index={step.fi}
                of={g.foundations.length}
                q={step.q}
                busy={busy}
                next={!after ? "See how it went" : after.fi !== step.fi ? `Next: ${after.f.title}` : "Next question"}
                onAnswer={(i) => answer(step.q, i)}
                onNext={() => setShowing(null)}
              />
            ) : waiting ? (
              <motion.p
                key="writing"
                variants={pop}
                initial="hidden"
                animate="shown"
                exit="gone"
                className="flex items-center gap-2 text-sm text-ink-3"
              >
                <LoaderCircle size={14} className="animate-spin text-accent" aria-hidden /> Writing the questions…
              </motion.p>
            ) : null}
          </AnimatePresence>
          {!done && (
            <motion.div variants={landing} className="flex items-center gap-3 pt-2">
              <a
                href={topicHref}
                className="flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm text-ink-3 ring-1 ring-hairline transition-colors hover:text-ink"
              >
                Finish later <ArrowRight size={14} aria-hidden />
              </a>
            </motion.div>
          )}
        </Stage>
      )}
    </Screen>
  );
}

// Where the run is: each foundation's name with a dot per question, the
// current one brighter.
function Progress({ g, current }: { g: GroundView; current: number }) {
  return (
    <motion.ol variants={stage(0.08, 0)} className="flex flex-wrap gap-x-6 gap-y-2">
      {g.foundations.map((f, i) => {
        const answered = f.questions.filter((q) => q.answered !== null).length;
        return (
          <motion.li
            key={f.suggestion_id}
            variants={pop}
            aria-label={`${f.title}: ${answered} of ${f.questions.length} answered`}
            className={`flex items-center gap-2 text-xs transition-colors ${i === current ? "text-ink" : "text-ink-4"}`}
          >
            {f.title}
            <span className="flex items-center gap-1">
              {f.questions.map((q) => (
                <motion.span
                  key={q.id}
                  variants={bead}
                  className={`block size-2 rounded-full transition-colors ${q.answered === null ? "ring-1 ring-inset ring-overlay" : q.correct ? "bg-teal" : "bg-warn"}`}
                />
              ))}
            </span>
          </motion.li>
        );
      })}
    </motion.ol>
  );
}

function Card({
  f,
  index,
  of,
  q,
  busy,
  next,
  onAnswer,
  onNext,
}: {
  f: GroundFoundation;
  index: number;
  of: number;
  q: GroundQuestion;
  busy: boolean;
  next: string;
  onAnswer: (i: number) => void;
  onNext: () => void;
}) {
  const answered = q.answered !== null;
  const nextRef = useRef<HTMLButtonElement>(null);
  // Enter moves on, as in practice.
  useEffect(() => {
    if (answered) nextRef.current?.focus();
  }, [answered]);
  const tone = (i: number) =>
    !answered
      ? "border-hairline hover:border-teal/50 hover:bg-teal/5"
      : i === q.key
        ? "border-teal/60 bg-teal/5"
        : i === q.answered
          ? "border-warn/60 bg-warn/5"
          : "border-hairline opacity-60";
  return (
    <motion.section
      variants={stage(0.05, 0)}
      initial="hidden"
      animate="shown"
      exit="gone"
      aria-label="Check question"
      className="flex flex-col gap-3 rounded-2xl bg-subtle px-5 py-5 ring-1 ring-hairline"
    >
      <motion.p variants={pop} className="text-xs text-ink-4">
        {of > 1 ? `Foundation ${index + 1} of ${of} · ` : ""}
        <span className="text-ink-3">{f.title}</span>
      </motion.p>
      <motion.p variants={pop} className="text-[15px] leading-relaxed text-ink">
        {q.stem}
      </motion.p>
      <ol className="flex flex-col gap-1.5">
        {q.options.map((text, i) => (
          <motion.li key={text} variants={pop}>
            <button
              type="button"
              disabled={busy || answered}
              onClick={() => onAnswer(i)}
              className={`flex w-full items-start gap-2.5 rounded-lg border px-3 py-2 text-left text-sm transition-colors ${tone(i)}`}
            >
              <span className="shrink-0 pt-0.5 font-mono text-xs text-ink-3">{LETTERS[i]}</span>
              <span className="text-ink">{text}</span>
            </button>
          </motion.li>
        ))}
      </ol>
      <AnimatePresence>
        {answered && (
          <motion.div
            key="feedback"
            variants={stage(0.06, 0)}
            initial="hidden"
            animate="shown"
            exit="gone"
            className="flex flex-col gap-3"
          >
            <motion.p variants={pop} className={q.correct ? "text-sm text-teal" : "text-sm text-warn"}>
              {q.correct ? "Right." : "Not quite."}{" "}
              <span className="text-ink-3">{q.feedback.find((o) => o.correct)?.why}</span>
            </motion.p>
            <motion.button
              ref={nextRef}
              variants={pop}
              type="button"
              onClick={onNext}
              className="flex w-fit items-center gap-1.5 rounded-md px-3 py-1.5 text-xs text-ink-2 ring-1 ring-hairline hover:bg-raised"
            >
              {next} <ArrowRight size={12} aria-hidden />
            </motion.button>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.section>
  );
}

// How it went: each foundation's result, what kizn did about it (a shaky one
// is on the map to start with), and a way to change it.
function Summary({ g, onChange }: { g: GroundView; onChange: () => void }) {
  const [error, setError] = useState<string | null>(null);
  const resolve = (f: GroundFoundation, action: "prepare" | "dismiss") => {
    setError(null);
    call({ type: "resolve_foundation", suggestion_id: f.suggestion_id, action })
      .then(onChange)
      .catch((e: Error) => setError(e.message));
  };
  // The foundations kizn is preparing: where to start, on the map.
  const onMap = g.foundations.filter((f) => f.status === "started" && f.topic_id);
  const mapHref = `#/map?${new URLSearchParams({
    arrived: onMap.map((f) => f.topic_id as string).join(","),
    under: g.topic_id,
    from: "check",
  })}`;
  return (
    <motion.section
      variants={stage(0.1, 0)}
      initial="hidden"
      animate="shown"
      exit="gone"
      aria-label="How it went"
      className="flex flex-col gap-4"
    >
      <ul className="flex flex-col gap-2">
        {g.foundations.map((f) => (
          <motion.li
            key={f.suggestion_id}
            variants={pop}
            aria-label={f.title}
            className="flex flex-col gap-2 rounded-2xl bg-subtle px-5 py-4 ring-1 ring-hairline"
          >
            <div className="flex items-center gap-3">
              <p className="flex-1 text-sm font-medium text-ink">{f.title}</p>
              <span
                className={`flex items-center gap-1 text-xs font-medium ${f.result === "solid" ? "text-teal" : "text-warn"}`}
              >
                {f.result === "solid" && <Check size={12} aria-hidden />}
                {f.result === "solid" ? "Seems solid" : "Might be shaky"}
              </span>
            </div>
            <Choice f={f} onResolve={(a) => resolve(f, a)} />
          </motion.li>
        ))}
      </ul>
      {error && <p className="text-xs text-danger">{error}</p>}
      <motion.div variants={landing}>
        <a
          href={onMap.length ? mapHref : `#/topic/${encodeURIComponent(g.topic_id)}`}
          className="flex w-fit items-center gap-1.5 rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-canvas transition-opacity hover:opacity-90"
        >
          {onMap.length ? "See where to start" : `Go to ${g.topic_title}`} <ArrowRight size={14} aria-hidden />
        </a>
      </motion.div>
    </motion.section>
  );
}

// What happened to a foundation, in a few words, and the one or two ways to
// change it: one kizn is preparing can go ("not needed"); one that seemed
// solid, or was let go, can be prepared after all.
function Choice({ f, onResolve }: { f: GroundFoundation; onResolve: (a: "prepare" | "dismiss") => void }) {
  const quiet = "rounded-md px-2 py-1 text-xs text-ink-3 transition-colors hover:bg-raised hover:text-ink";
  const quietly = (label: string, action: "prepare" | "dismiss") => (
    <button type="button" onClick={() => onResolve(action)} className={quiet}>
      {label}
    </button>
  );
  const { line, actions } =
    f.status === "started"
      ? {
          line: "kizn is preparing it underneath, to start with.",
          actions: (
            <>
              {f.topic_id && (
                <a href={`#/topic/${encodeURIComponent(f.topic_id)}`} className={quiet}>
                  Open
                </a>
              )}
              {quietly("Not needed", "dismiss")}
            </>
          ),
        }
      : f.status === "dismissed"
        ? { line: "Not needed.", actions: quietly("Prepare it after all", "prepare") }
        : { line: "Good ground to build on.", actions: quietly("Prepare it anyway", "prepare") };
  return (
    <AnimatePresence mode="wait" initial={false}>
      <motion.div
        key={f.status}
        initial={{ opacity: 0, y: 3 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: -3 }}
        transition={{ duration: 0.15 }}
        className="flex flex-wrap items-center gap-1"
      >
        <p className="mr-auto text-xs text-ink-3">{line}</p>
        {actions}
      </motion.div>
    </AnimatePresence>
  );
}
