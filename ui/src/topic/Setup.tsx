import { ArrowRight, CircleAlert, CircleCheck, LoaderCircle, PencilRuler, Sparkles, Target, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { Progress, type ProgressStep } from "../components/Progress";
import { subscribe } from "../events";
import { EASE, presence, rise } from "../motion";
import { call, type TopicView } from "../rpc";

// Starting a topic, one card per step under the progress line: Goal → Draft
// → Shape → Confirm, then the topic page. The author model drafts; the
// learner decides. Built in small reviewed steps: this is the Goal card.

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

  const step = !topicId ? 0 : 1;

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

// Placeholder until the next slice: the outcomes as they arrive.
function DraftStep({ topic }: { topic: TopicView | null }) {
  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-base font-semibold text-ink">Drafting what you'll be able to do…</h2>
      {topic && <p className="text-sm text-ink-3">“{topic.goal}”</p>}
      <ol className="flex flex-col gap-2 text-sm text-ink-2">
        <AnimatePresence initial={false}>
          {topic?.outcomes.map((o) => (
            <motion.li key={o.id} variants={rise} {...presence}>
              {o.ord}. {o.statement}
            </motion.li>
          ))}
        </AnimatePresence>
      </ol>
    </div>
  );
}
