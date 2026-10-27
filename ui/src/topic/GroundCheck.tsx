import { ArrowRight, Check, ChevronLeft, LoaderCircle, Play } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useState } from "react";
import type { GroundFoundation } from "../bindings/GroundFoundation";
import type { GroundView } from "../bindings/GroundView";
import { Screen } from "../components/Screen";
import { Stage } from "../components/Stage";
import { subscribe } from "../events";
import { bead, button as landing, pop, popStage, stage } from "../motion";
import { call } from "../rpc";

const LETTERS = ["A", "B", "C", "D", "E"];

// Checking the ground, the last step of setup: a few questions on each
// foundation kept for the topic. Not a test: each comes out "seems solid" or
// "might be shaky", and a shaky one is the learner's to start now, leave for
// later (it stays on the map) or set aside.
export function GroundCheck({ topicId }: { topicId: string }) {
  const [g, setG] = useState<GroundView | null>(null);
  const [error, setError] = useState<string | null>(null);
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

  const done = !!g && g.foundations.every((f) => f.result !== null || f.status !== "kept");
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
            A few questions on each, while your practice is being prepared. Not a test: they show whether each
            foundation seems solid, so you can decide what to learn underneath.
          </motion.p>
          {g.foundations.map((f) => (
            <Foundation key={f.suggestion_id} f={f} onChange={setG} />
          ))}
          <motion.div variants={landing} className="flex items-center gap-3 pt-2">
            <a
              href={topicHref}
              className={`flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-semibold transition-opacity hover:opacity-90 ${done ? "bg-accent text-canvas" : "text-ink-3 ring-1 ring-hairline"}`}
            >
              {done ? `Go to ${g.topic_title}` : "Finish later"} <ArrowRight size={14} aria-hidden />
            </a>
          </motion.div>
        </Stage>
      )}
    </Screen>
  );
}

function Foundation({ f, onChange }: { f: GroundFoundation; onChange: (g: GroundView) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const next = f.questions.findIndex((q) => q.answered === null);
  const current = next >= 0 ? f.questions[next] : undefined;
  const last = [...f.questions].reverse().find((q) => q.answered !== null);
  const [showLast, setShowLast] = useState(false);

  const answer = (item: string, order: number[], chosen: number) => {
    setBusy(true);
    setError(null);
    call({ type: "answer_ground", item_id: item, order, chosen })
      .then((g) => {
        onChange(g);
        setShowLast(true);
      })
      .catch((e: Error) => setError(e.message))
      .finally(() => setBusy(false));
  };

  const resolve = (action: "start" | "later" | "dismiss") => {
    call({ type: "resolve_foundation", suggestion_id: f.suggestion_id, action })
      .then((t) => {
        if (action === "start") window.location.hash = `#/setup/${encodeURIComponent(t.id)}`;
      })
      .catch((e: Error) => setError(e.message));
  };

  const answered = f.questions.filter((q) => q.answered !== null).length;
  return (
    <motion.section
      variants={popStage(0.06, 0.08)}
      aria-label={f.title}
      className="flex flex-col gap-4 rounded-2xl bg-subtle px-5 py-5 ring-1 ring-hairline"
    >
      <motion.div variants={pop} className="flex items-start gap-3">
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <h2 className="text-base font-semibold text-ink">{f.title}</h2>
          {f.why && <p className="text-xs text-ink-3">{f.why}</p>}
        </div>
        <motion.span
          variants={stage(0.06, 0.2)}
          className="flex items-center gap-1 pt-1"
          aria-label={`${answered} of ${f.questions.length} answered`}
        >
          {f.questions.map((q) => (
            <motion.span
              key={q.id}
              variants={bead}
              className={`block size-2 rounded-full ${q.answered === null ? "ring-1 ring-inset ring-overlay" : q.correct ? "bg-teal" : "bg-warn"}`}
            />
          ))}
        </motion.span>
      </motion.div>

      {f.questions.length === 0 && (
        <p className="flex items-center gap-2 text-sm text-ink-3">
          <LoaderCircle size={14} className="animate-spin text-accent" aria-hidden /> Writing questions…
        </p>
      )}

      <AnimatePresence mode="wait" initial={false}>
        {showLast && last ? (
          <motion.div
            key={`fb-${last.id}`}
            variants={pop}
            initial="hidden"
            animate="shown"
            exit="gone"
            className="flex flex-col gap-2"
          >
            <p className={last.correct ? "text-sm text-teal" : "text-sm text-warn"}>
              {last.correct ? "Right." : "Not quite."}{" "}
              <span className="text-ink-3">{last.feedback.find((o) => o.correct)?.why}</span>
            </p>
            <button
              type="button"
              onClick={() => setShowLast(false)}
              className="flex w-fit items-center gap-1.5 rounded-md px-2.5 py-1 text-xs text-ink-2 ring-1 ring-hairline hover:bg-raised"
            >
              {current ? "Next question" : "See how it went"} <ArrowRight size={12} aria-hidden />
            </button>
          </motion.div>
        ) : current ? (
          <motion.div
            key={current.id}
            variants={stage(0.05, 0)}
            initial="hidden"
            animate="shown"
            exit="gone"
            className="flex flex-col gap-2.5"
            aria-label="Check question"
          >
            <motion.p variants={pop} className="text-[15px] leading-relaxed text-ink">
              {current.stem}
            </motion.p>
            <ol className="flex flex-col gap-1.5">
              {current.options.map((text, i) => (
                <motion.li key={text} variants={pop}>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => answer(current.id, current.order, i)}
                    className="flex w-full items-start gap-2.5 rounded-lg border border-hairline px-3 py-2 text-left text-sm transition-colors hover:border-teal/50 hover:bg-teal/5"
                  >
                    <span className="shrink-0 pt-0.5 font-mono text-xs text-ink-3">{LETTERS[i]}</span>
                    <span className="text-ink">{text}</span>
                  </button>
                </motion.li>
              ))}
            </ol>
          </motion.div>
        ) : f.result ? (
          <Result key="result" f={f} onResolve={resolve} />
        ) : null}
      </AnimatePresence>
      {error && <p className="text-xs text-danger">{error}</p>}
    </motion.section>
  );
}

function Result({ f, onResolve }: { f: GroundFoundation; onResolve: (a: "start" | "later" | "dismiss") => void }) {
  const solid = f.result === "solid";
  return (
    <motion.div variants={stage(0.08, 0)} initial="hidden" animate="shown" exit="gone" className="flex flex-col gap-3">
      <motion.p
        variants={pop}
        className={`flex items-center gap-1.5 text-sm font-medium ${solid ? "text-teal" : "text-warn"}`}
      >
        {solid ? <Check size={14} aria-hidden /> : null}
        {solid ? "Seems solid" : "Might be shaky"}
      </motion.p>
      <motion.p variants={pop} className="text-xs text-ink-3">
        {solid
          ? "Good ground to build on."
          : f.status === "later"
            ? "On your map, to come back to."
            : f.status === "dismissed"
              ? "Set aside: you've got this."
              : "Learning it first would make the topic easier. It's your call."}
      </motion.p>
      {!solid && f.status === "kept" && (
        <motion.div variants={pop} className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => onResolve("start")}
            className="flex items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-xs font-semibold text-canvas hover:opacity-90"
          >
            <Play size={12} aria-hidden /> Start it
          </button>
          <button
            type="button"
            onClick={() => onResolve("later")}
            className="rounded-lg px-3 py-1.5 text-xs text-ink-2 ring-1 ring-hairline hover:bg-raised"
          >
            Later
          </button>
          <button
            type="button"
            onClick={() => onResolve("dismiss")}
            className="rounded-lg px-3 py-1.5 text-xs text-ink-3 hover:bg-raised hover:text-ink"
          >
            I've got this
          </button>
        </motion.div>
      )}
    </motion.div>
  );
}
