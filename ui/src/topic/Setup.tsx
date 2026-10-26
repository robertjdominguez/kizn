import {
  ArrowLeft,
  ArrowRight,
  ChevronDown,
  ChevronUp,
  Circle,
  CircleAlert,
  CircleCheck,
  LoaderCircle,
  PencilRuler,
  Plus,
  RotateCw,
  Sparkles,
  Target,
  Trash2,
  X,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { Progress, type ProgressStep } from "../components/Progress";
import { subscribe } from "../events";
import { settingsCanFix } from "../fixable";
import { EASE, presence, rise } from "../motion";
import { call, type TopicView } from "../rpc";

// Starting a topic, one card per step under the progress line: Goal → Draft
// → Shape → Confirm, then the topic page. The author model drafts; the
// learner decides. Built in small reviewed steps: Goal, Draft, Shape, Confirm.

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
    <div
      className={`mx-auto flex w-full flex-col gap-5 px-6 py-8 transition-[max-width] duration-300 ${step === 2 ? "max-w-3xl" : "max-w-2xl"}`}
    >
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
          {step === 2 && topic && <ShapeStep topic={topic} onContinue={() => setStep(3)} />}
          {step === 3 && topic && <ConfirmStep topic={topic} onBack={() => setStep(2)} />}
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
          <motion.div
            key="failed"
            variants={rise}
            {...presence}
            role="alert"
            className="flex flex-col gap-2 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2.5 text-sm"
          >
            <p className="flex items-start gap-2 text-ink-2">
              <CircleAlert size={14} className="mt-0.5 shrink-0 text-warn" aria-hidden />
              <span>
                {topic.draft_error.charAt(0).toUpperCase()}
                {topic.draft_error.slice(1)}.
              </span>
            </p>
            <div className="flex items-center gap-2 pl-6">
              <button
                type="button"
                onClick={() => call({ type: "resume_drafting", topic_id: topic.id }).catch(() => undefined)}
                className="flex items-center gap-1.5 rounded-md bg-warn/15 px-2.5 py-1 text-xs font-medium text-warn transition-colors hover:bg-warn/25"
              >
                <RotateCw size={12} aria-hidden /> Try again
              </button>
              {settingsCanFix(topic.draft_error) && (
                <a href="#/settings" className="rounded-md px-2 py-1 text-xs text-ink-3 hover:bg-raised hover:text-ink">
                  Settings
                </a>
              )}
              <span className="text-xs text-ink-4">It carries on from where it stopped.</span>
            </div>
          </motion.div>
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

const TARGETS: { tier: string; plain: string }[] = [
  { tier: "remember", plain: "recall it" },
  { tier: "understand", plain: "explain it" },
  { tier: "apply", plain: "use it" },
  { tier: "analyze", plain: "work out why" },
  { tier: "evaluate", plain: "judge and recommend" },
  { tier: "create", plain: "design it" },
];
const MAX_CONCEPTS = 7;

type ShapedOutcome = {
  key: string;
  id?: string;
  statement: string;
  target: string;
  reason: string;
  concepts: string[];
};

let keys = 0;
const nextKey = () => `k${++keys}`;

// The draft as an editor: every change shows at once and saves in the
// background (the whole draft each time). Continue needs every outcome to
// have a concept.
function ShapeStep({ topic, onContinue }: { topic: TopicView; onContinue: () => void }) {
  const [outcomes, setOutcomes] = useState<ShapedOutcome[]>(() =>
    topic.outcomes.map((o) => ({
      key: nextKey(),
      id: o.id,
      statement: o.statement,
      target: o.target,
      reason: o.reason,
      concepts: o.concepts.map((c) => c.statement),
    })),
  );
  const [saving, setSaving] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [error, setError] = useState<string | null>(null);
  // What was last saved: only a real change saves (and says "Saved").
  const lastSaved = useRef(JSON.stringify(outcomes));

  const save = useCallback(
    async (list: ShapedOutcome[]) => {
      setSaving("saving");
      try {
        await call({
          type: "save_draft",
          topic_id: topic.id,
          draft: {
            outcomes: list.map((o) => ({
              id: o.id,
              statement: o.statement,
              target: o.target,
              reason: o.reason,
              concepts: o.concepts.filter((c) => c.trim()),
            })),
          },
        });
        setSaving("saved");
        setError(null);
        return true;
      } catch (e) {
        setSaving("error");
        setError((e as Error).message);
        return false;
      }
    },
    [topic.id],
  );

  // Save a moment after the last change.
  useEffect(() => {
    const now = JSON.stringify(outcomes);
    if (now === lastSaved.current) return;
    const t = setTimeout(() => {
      save(outcomes).then((ok) => {
        if (ok) lastSaved.current = now;
      });
    }, 600);
    return () => clearTimeout(t);
  }, [outcomes, save]);

  const edit = (key: string, change: Partial<ShapedOutcome>) =>
    setOutcomes((list) => list.map((o) => (o.key === key ? { ...o, ...change } : o)));
  const move = (key: string, by: number) =>
    setOutcomes((list) => {
      const i = list.findIndex((o) => o.key === key);
      const j = i + by;
      if (i < 0 || j < 0 || j >= list.length) return list;
      const next = [...list];
      [next[i], next[j]] = [next[j] as ShapedOutcome, next[i] as ShapedOutcome];
      return next;
    });
  const remove = (key: string) => setOutcomes((list) => list.filter((o) => o.key !== key));
  const add = () =>
    setOutcomes((list) => [
      ...list,
      { key: nextKey(), statement: "", target: "understand", reason: "", concepts: [""] },
    ]);

  const missing = outcomes.findIndex((o) => !o.statement.trim() || !o.concepts.some((c) => c.trim()));
  const ready = outcomes.length > 0 && missing === -1;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-start gap-3">
        <div className="flex flex-1 flex-col gap-0.5">
          <h2 className="text-base font-semibold text-ink">Shape it</h2>
          <p className="text-sm text-ink-3">
            Reword, reorder, change a target, add or remove concepts. Nothing is written until you confirm.
          </p>
        </div>
        <AnimatePresence mode="wait" initial={false}>
          <motion.span
            key={saving}
            variants={rise}
            {...presence}
            className="shrink-0 pt-1 text-xs text-ink-4"
            aria-live="polite"
          >
            {saving === "saving" ? "Saving…" : saving === "saved" ? "Saved" : saving === "error" ? "" : ""}
          </motion.span>
        </AnimatePresence>
      </div>

      <ol className="flex flex-col gap-3">
        <AnimatePresence initial={false}>
          {outcomes.map((o, i) => (
            <motion.li
              key={o.key}
              layout
              variants={rise}
              {...presence}
              className="flex gap-3 rounded-lg border border-hairline bg-canvas p-3"
            >
              <div className="flex flex-col items-center gap-1 pt-1">
                <span className="text-sm tabular-nums text-ink-4">{i + 1}</span>
                <button
                  type="button"
                  aria-label={`Move outcome ${i + 1} up`}
                  disabled={i === 0}
                  onClick={() => move(o.key, -1)}
                  className="rounded p-0.5 text-ink-4 transition-colors hover:bg-raised hover:text-ink disabled:opacity-30"
                >
                  <ChevronUp size={14} aria-hidden />
                </button>
                <button
                  type="button"
                  aria-label={`Move outcome ${i + 1} down`}
                  disabled={i === outcomes.length - 1}
                  onClick={() => move(o.key, 1)}
                  className="rounded p-0.5 text-ink-4 transition-colors hover:bg-raised hover:text-ink disabled:opacity-30"
                >
                  <ChevronDown size={14} aria-hidden />
                </button>
              </div>
              <div className="flex min-w-0 flex-1 flex-col gap-2">
                <div className="flex items-start gap-2">
                  <textarea
                    aria-label={`Outcome ${i + 1}`}
                    rows={2}
                    value={o.statement}
                    placeholder="What you'll be able to do, starting with a verb"
                    onChange={(e) => edit(o.key, { statement: e.target.value })}
                    className="min-h-12 flex-1 resize-none rounded-md border border-transparent bg-transparent px-2 py-1 text-sm text-ink outline-none transition-colors placeholder:text-ink-4 hover:border-hairline focus:border-hairline-strong focus:ring-2 focus:ring-accent/30"
                  />
                  <button
                    type="button"
                    aria-label={`Remove outcome ${i + 1}`}
                    onClick={() => remove(o.key)}
                    className="rounded p-1 text-ink-4 transition-colors hover:bg-raised hover:text-danger"
                  >
                    <Trash2 size={14} aria-hidden />
                  </button>
                </div>
                <label className="flex items-center gap-2 px-2 text-xs text-ink-3">
                  Target
                  <select
                    value={o.target}
                    onChange={(e) => edit(o.key, { target: e.target.value })}
                    className="rounded-md border border-hairline-strong bg-canvas px-2 py-1 text-xs text-ink outline-none focus:ring-2 focus:ring-accent/30"
                  >
                    {TARGETS.map((t) => (
                      <option key={t.tier} value={t.tier}>
                        {t.plain}
                      </option>
                    ))}
                  </select>
                  {o.reason && <span className="truncate text-ink-4">{o.reason}</span>}
                </label>
                <ConceptsEditor index={i} concepts={o.concepts} onChange={(concepts) => edit(o.key, { concepts })} />
              </div>
            </motion.li>
          ))}
        </AnimatePresence>
      </ol>

      <button
        type="button"
        onClick={add}
        className="flex w-fit items-center gap-1.5 rounded-md px-2 py-1 text-sm text-accent transition-colors hover:bg-accent/10"
      >
        <Plus size={14} aria-hidden /> Add an outcome
      </button>

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
            <span>Couldn't save: {error}</span>
          </motion.p>
        )}
      </AnimatePresence>

      <div className="flex items-center gap-3">
        <span className="flex-1 text-xs text-ink-4">
          {outcomes.length === 0
            ? "Add at least one outcome."
            : missing >= 0
              ? `Outcome ${missing + 1} needs wording and at least one concept.`
              : ""}
        </span>
        <button
          type="button"
          disabled={!ready || saving === "saving"}
          onClick={async () => {
            if (await save(outcomes)) onContinue();
          }}
          className="flex items-center gap-1.5 rounded-md bg-accent px-3.5 py-1.5 text-sm font-medium text-canvas transition-opacity hover:opacity-90 disabled:opacity-50"
        >
          Continue <ArrowRight size={14} aria-hidden />
        </button>
      </div>
    </div>
  );
}

function ConceptsEditor({
  index,
  concepts,
  onChange,
}: {
  index: number;
  concepts: string[];
  onChange: (c: string[]) => void;
}) {
  const set = (j: number, value: string) => onChange(concepts.map((c, k) => (k === j ? value : c)));
  return (
    <div className="flex flex-col gap-1 border-l border-hairline-strong pl-3 ml-2">
      <ul className="flex flex-col gap-1">
        <AnimatePresence initial={false}>
          {concepts.map((c, j) => (
            <motion.li
              // biome-ignore lint/suspicious/noArrayIndexKey: concepts are edited in place by position
              key={j}
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              className="group flex items-center gap-2"
            >
              <Circle size={9} className="shrink-0 text-ink-4" aria-hidden />
              <input
                aria-label={`Outcome ${index + 1}, concept ${j + 1}`}
                value={c}
                placeholder="An idea or fact, as you'd want it understood"
                onChange={(e) => set(j, e.target.value)}
                className="flex-1 rounded border border-transparent bg-transparent px-1.5 py-0.5 text-xs text-ink-2 outline-none transition-colors placeholder:text-ink-4 hover:border-hairline focus:border-hairline-strong focus:ring-2 focus:ring-accent/30"
              />
              <button
                type="button"
                aria-label={`Remove concept ${j + 1}`}
                onClick={() => onChange(concepts.filter((_, k) => k !== j))}
                className="rounded p-0.5 text-ink-4 opacity-0 transition-opacity hover:text-danger focus:opacity-100 group-hover:opacity-100"
              >
                <X size={12} aria-hidden />
              </button>
            </motion.li>
          ))}
        </AnimatePresence>
      </ul>
      {concepts.length < MAX_CONCEPTS && (
        <button
          type="button"
          onClick={() => onChange([...concepts, ""])}
          className="flex w-fit items-center gap-1 rounded px-1.5 py-0.5 text-xs text-ink-3 transition-colors hover:text-accent"
        >
          <Plus size={12} aria-hidden /> Add a concept
        </button>
      )}
    </div>
  );
}

// A last look before kizn starts writing practice: confirming locks the
// topic (it can't be edited after) and lands on the topic page.
function ConfirmStep({ topic, onBack }: { topic: TopicView; onBack: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const concepts = topic.outcomes.reduce((n, o) => n + o.concepts.length, 0);
  const confirm = async () => {
    setBusy(true);
    setError(null);
    try {
      await call({ type: "confirm_topic", topic_id: topic.id });
      window.location.hash = `#/topic/${encodeURIComponent(topic.id)}`;
    } catch (e) {
      setError((e as Error).message);
      setBusy(false);
    }
  };
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-0.5">
        <h2 className="text-base font-semibold text-ink">Ready to prepare your practice?</h2>
        <p className="text-sm text-ink-3">
          kizn writes and checks practice questions for each outcome in the background. You can close it: Today will say
          when they're ready. Once confirmed, the outcomes can't be changed.
        </p>
      </div>
      <div className="rounded-lg border border-hairline bg-canvas px-4 py-3">
        <p className="text-sm font-medium text-ink">{topic.title}</p>
        <ol className="mt-2 flex flex-col gap-1.5">
          {topic.outcomes.map((o, i) => (
            <motion.li
              key={o.id}
              initial={{ opacity: 0, y: 4 }}
              animate={{ opacity: 1, y: 0, transition: { delay: i * 0.05, duration: 0.2, ease: EASE } }}
              className="flex gap-2 text-sm"
            >
              <span className="w-4 shrink-0 tabular-nums text-ink-4">{o.ord}</span>
              <span className="flex-1 text-ink-2">{o.statement}</span>
            </motion.li>
          ))}
        </ol>
        <p className="mt-3 border-t border-hairline pt-2 text-xs text-ink-3">
          {topic.outcomes.length} outcomes · {concepts} concepts · about {topic.expected} practice questions to prepare
        </p>
      </div>
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
            <span>{error}</span>
          </motion.p>
        )}
      </AnimatePresence>
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={onBack}
          className="flex items-center gap-1 rounded-md px-2 py-1.5 text-sm text-ink-3 hover:bg-raised hover:text-ink"
        >
          <ArrowLeft size={14} aria-hidden /> Back
        </button>
        <span className="flex-1" />
        <button
          type="button"
          onClick={confirm}
          disabled={busy}
          className="flex items-center gap-1.5 rounded-md bg-accent px-3.5 py-1.5 text-sm font-medium text-canvas transition-opacity hover:opacity-90 disabled:opacity-50"
        >
          {busy ? <LoaderCircle size={14} className="animate-spin" aria-hidden /> : null}
          {busy ? "Confirming…" : "Confirm"}
          {!busy && <ArrowRight size={14} aria-hidden />}
        </button>
      </div>
    </div>
  );
}
