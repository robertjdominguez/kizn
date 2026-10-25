import { ArrowRight, CircleAlert, CircleCheck, LoaderCircle, PencilRuler, Sparkles, Target, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { Progress, type ProgressStep } from "../components/Progress";
import { subscribe } from "../events";
import { EASE, presence, rise } from "../motion";
import { call, type TopicView } from "../rpc";

// Starting a topic, one card per step under the progress line: Goal → Draft
// → Shape → Confirm, then the topic page. The author model drafts; the
// learner decides. Built in small reviewed steps: Goal, then Draft.

const STEPS: ProgressStep[] = [
  { label: "Goal", icon: Target },
  { label: "Draft", icon: Sparkles },
  { label: "Shape", icon: PencilRuler },
  { label: "Confirm", icon: CircleCheck },
];

const slide = {
  enter: { x: 48, opacity: 0 },
  center: { x: 0, opacity: 1, transition: { duration: 0.24, ease: EASE } },
  exit: { x: -48, opacity: 0, transition: { duration: 0.16, ease: EASE } },
};

/** `#/new-topic` (no id) starts at Goal; `#/setup/<id>` resumes a draft. */
export function Setup({ topicId }: { topicId?: string }) {
  const [topic, setTopic] = useState<TopicView | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    if (!topicId) return;
    call({ type: "get_topic", topic_id: topicId })
      .then(setTopic)
      .catch((e: Error) => setError(e.message));
  }, [topicId]);

  useEffect(() => {
    load();
    if (!topicId) return;
    return subscribe((e) => {
      if (e.type === "topic_changed" && e.topic_id === topicId) load();
    });
  }, [load, topicId]);

  // Drafting shows the Draft step; once it's done (or stopped), Shape
  // follows by itself after a moment, so the last concepts can land.
  const drafting = !topic || topic.draft_state === "drafting" || topic.draft_state === "failed";
  const [step, setStep] = useState(!topicId ? 0 : 1);
  useEffect(() => {
    if (!topicId) return setStep(0);
    if (drafting) return setStep(1);
    const t = setTimeout(() => setStep(2), 900);
    return () => clearTimeout(t);
  }, [topicId, drafting]);

  return (
    <div className="mx-auto flex w-full max-w-2xl flex-col gap-5 px-6 py-8">
      <Progress steps={STEPS} index={step} />
      {error && <p className="text-sm text-danger">{error}</p>}
      <AnimatePresence mode="wait" initial={false}>
        <motion.div
          key={step}
          variants={slide}
          initial="enter"
          animate="center"
          exit="exit"
          className="rounded-xl border border-hairline-strong bg-subtle p-5 shadow-xl shadow-black/20"
        >
          {step === 0 && <GoalStep />}
          {step === 1 && <DraftStep topic={topic} />}
          {step === 2 && topic && <ShapeStep topic={topic} />}
        </motion.div>
      </AnimatePresence>
    </div>
  );
}

const FIELD =
  "w-full rounded-md border border-hairline-strong bg-canvas px-3 py-2 text-sm text-ink outline-none placeholder:text-ink-4 focus:ring-2 focus:ring-accent/40";

function GoalStep() {
  const [title, setTitle] = useState("");
  const [goal, setGoal] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const first = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const t = setTimeout(() => first.current?.focus(), 250);
    return () => clearTimeout(t);
  }, []);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const topic = await call({ type: "start_topic", title, goal });
      window.location.hash = `#/setup/${encodeURIComponent(topic.id)}`;
    } catch (err) {
      setError((err as Error).message);
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit} aria-label="Goal step" className="flex flex-col gap-4">
      <div className="flex items-start gap-3">
        <div className="flex flex-1 flex-col gap-0.5">
          <h2 className="text-base font-semibold text-ink">What do you want to learn?</h2>
          <p className="text-sm text-ink-3">
            Say what you need to be able to do, and why. kizn drafts outcomes and concepts from it for you to shape.
          </p>
        </div>
        <a href="#/" aria-label="Close" className="rounded p-1 text-ink-3 hover:bg-raised hover:text-ink">
          <X size={15} aria-hidden />
        </a>
      </div>
      <label className="flex flex-col gap-1.5 text-xs text-ink-3">
        Name it
        <input
          ref={first}
          className={FIELD}
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Container fundamentals"
        />
      </label>
      <label className="flex flex-col gap-1.5 text-xs text-ink-3">
        What do you want to be able to do, and why?
        <textarea
          className={`${FIELD} min-h-28 resize-y leading-relaxed`}
          value={goal}
          onChange={(e) => setGoal(e.target.value)}
          placeholder="Explain to customers why their deployments behave differently, so I can answer escalations without pulling in an engineer."
        />
        <span className="text-[11px] text-ink-4">
          The why matters: "explain to customers" and "advise customers" ask for different things, and kizn sets each
          outcome's target from it.
        </span>
      </label>
      <AnimatePresence>
        {error && (
          <motion.p
            key="error"
            variants={rise}
            {...presence}
            role="alert"
            className="flex items-start gap-2 text-sm text-danger"
          >
            <CircleAlert size={14} className="mt-0.5 shrink-0" aria-hidden />
            <span>
              {error}
              {/Settings/.test(error) && (
                <>
                  {" "}
                  <a href="#/settings" className="underline">
                    Open Settings
                  </a>
                </>
              )}
            </span>
          </motion.p>
        )}
      </AnimatePresence>
      <div className="flex items-center justify-end">
        <button
          type="submit"
          disabled={busy || !title.trim() || !goal.trim()}
          className="flex items-center gap-1.5 rounded-md bg-accent px-3.5 py-1.5 text-sm font-medium text-canvas transition-opacity hover:opacity-90 disabled:opacity-50"
        >
          {busy ? <LoaderCircle size={14} className="animate-spin" aria-hidden /> : null}
          {busy ? "Starting…" : "Continue"}
          {!busy && <ArrowRight size={14} aria-hidden />}
        </button>
      </div>
    </form>
  );
}

// Outcomes stream in one by one: skeletons until the first request returns,
// then hierarchy: done outcomes settle, the one being drafted stands out,
// those still to come recede.
function DraftStep({ topic }: { topic: TopicView | null }) {
  const [stopping, setStopping] = useState(false);
  const outcomes = topic?.outcomes ?? [];
  const failed = topic?.draft_state === "failed";
  const stop = async () => {
    if (!topic) return;
    setStopping(true);
    await call({ type: "stop_drafting", topic_id: topic.id }).catch(() => undefined);
  };
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-0.5">
        <h2 className="text-base font-semibold text-ink">
          {failed ? "Drafting stopped" : "Drafting what you'll be able to do…"}
        </h2>
        {topic && <p className="text-sm text-ink-3">“{topic.goal}”</p>}
      </div>
      <ol className="flex flex-col gap-2">
        <AnimatePresence initial={false} mode="popLayout">
          {outcomes.length === 0 && !failed
            ? [0, 1, 2].map((i) => <SkeletonOutcome key={`skeleton-${i}`} index={i} />)
            : outcomes.map((o) => (
                <motion.li
                  key={o.id}
                  layout
                  initial={{ opacity: 0, y: 6 }}
                  animate={{
                    opacity: o.concepts_state === "pending" ? 0.5 : 1,
                    y: 0,
                    transition: { duration: 0.24, ease: EASE, delay: (o.ord - 1) * 0.18 },
                  }}
                  exit={{ opacity: 0, transition: { duration: 0.12 } }}
                  className={`relative flex gap-3 rounded-lg px-3 py-2.5 ${o.concepts_state === "drafting" ? "bg-raised/40" : ""}`}
                >
                  <AnimatePresence>
                    {o.concepts_state === "drafting" && (
                      <motion.span
                        key="edge"
                        initial={{ scaleY: 0 }}
                        animate={{ scaleY: 1 }}
                        exit={{ scaleY: 0 }}
                        className="absolute inset-y-1 left-0 w-0.5 origin-top rounded-full bg-accent"
                        aria-hidden
                      />
                    )}
                  </AnimatePresence>
                  <span className="w-4 shrink-0 text-sm tabular-nums text-ink-4">{o.ord}</span>
                  <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                    <p className="text-sm text-ink">{o.statement}</p>
                    <p className="text-xs text-ink-3">
                      Target: <span className="text-ink-2">{o.target_plain}</span>
                      {o.reason && <span className="text-ink-4"> · {o.reason}</span>}
                    </p>
                    <ConceptsLine outcome={o} />
                  </div>
                </motion.li>
              ))}
        </AnimatePresence>
      </ol>
      <AnimatePresence>
        {failed && topic?.draft_error && (
          <motion.p
            key="failed"
            variants={rise}
            {...presence}
            role="alert"
            className="flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2.5 text-sm text-ink-2"
          >
            <CircleAlert size={14} className="mt-0.5 shrink-0 text-warn" aria-hidden />
            <span>
              {topic.draft_error}{" "}
              <a href="#/settings" className="text-accent underline">
                Settings
              </a>
            </span>
          </motion.p>
        )}
      </AnimatePresence>
      {!failed && (
        <div className="flex items-center justify-end">
          <button
            type="button"
            onClick={stop}
            disabled={stopping || !topic}
            className="rounded-md px-3 py-1.5 text-sm text-ink-3 transition-colors hover:bg-raised hover:text-ink disabled:opacity-50"
          >
            {stopping ? "Stopping after this outcome…" : "Stop"}
          </button>
        </div>
      )}
    </div>
  );
}

function SkeletonOutcome({ index }: { index: number }) {
  return (
    <motion.li
      layout
      initial={{ opacity: 0 }}
      animate={{ opacity: 1, transition: { delay: index * 0.08 } }}
      exit={{ opacity: 0, transition: { duration: 0.15 } }}
      className="flex gap-3 px-3 py-2.5"
      aria-hidden
    >
      <span className="w-4 shrink-0 text-sm text-ink-4">{index + 1}</span>
      <div className="flex flex-1 flex-col gap-2">
        <span className="h-3.5 w-11/12 animate-pulse rounded-full bg-raised" />
        <span className="h-3 w-1/2 animate-pulse rounded-full bg-raised/70" />
      </div>
    </motion.li>
  );
}

function ConceptsLine({ outcome: o }: { outcome: TopicView["outcomes"][number] }) {
  if (o.concepts_state === "pending") return <p className="text-xs text-ink-4">Concepts next</p>;
  if (o.concepts_state === "done" && o.concepts.length > 0)
    return (
      <p className="flex items-center gap-1.5 text-xs text-success">
        <CircleCheck size={12} aria-hidden /> {o.concepts.length} concepts
      </p>
    );
  // Being drafted: skeleton chips until the concepts arrive.
  return (
    <ul className="flex flex-wrap gap-1.5" aria-label="Drafting concepts">
      <AnimatePresence initial={false} mode="popLayout">
        {o.concepts.length > 0
          ? o.concepts.map((c, i) => (
              <motion.li
                key={c.id}
                layout
                initial={{ opacity: 0, scale: 0.96 }}
                animate={{ opacity: 1, scale: 1, transition: { delay: i * 0.05 } }}
                className="rounded-full border border-hairline-strong px-2 py-0.5 text-xs text-ink-2"
              >
                {c.statement}
              </motion.li>
            ))
          : [96, 128, 80].map((w) => (
              <motion.li
                key={`chip-${w}`}
                layout
                exit={{ opacity: 0 }}
                className="h-5 animate-pulse rounded-full bg-raised"
                style={{ width: w }}
              />
            ))}
      </AnimatePresence>
    </ul>
  );
}

// Placeholder until the next slice.
function ShapeStep({ topic }: { topic: TopicView }) {
  return (
    <div className="flex flex-col gap-2">
      <h2 className="text-base font-semibold text-ink">Shape it</h2>
      <p className="text-sm text-ink-3">
        {topic.outcomes.length} outcomes drafted. Editing them comes in the next slice.
      </p>
    </div>
  );
}
