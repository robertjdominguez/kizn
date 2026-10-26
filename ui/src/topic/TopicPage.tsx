import {
  ChevronDown,
  ChevronLeft,
  Circle,
  CircleAlert,
  CircleCheck,
  LoaderCircle,
  RotateCw,
  Target,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useId, useState } from "react";
import type { OutcomeView } from "../bindings/OutcomeView";
import type { SlotView } from "../bindings/SlotView";
import { subscribe } from "../events";
import { EASE, presence, rise } from "../motion";
import { call, type TopicView } from "../rpc";

// A topic: what you'll be able to do, and how its practice is coming along.
// Built in small reviewed steps: the header, then the outcomes card.
export function TopicPage({ topicId }: { topicId: string }) {
  const [topic, setTopic] = useState<TopicView | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    call({ type: "get_topic", topic_id: topicId })
      .then((t) => {
        setTopic(t);
        setError(null);
      })
      .catch((e: Error) => setError(e.message));
  }, [topicId]);

  useEffect(() => {
    load();
    return subscribe((e) => {
      if (e.type === "topic_changed" && e.topic_id === topicId) load();
    });
  }, [load, topicId]);

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-6 px-6 py-8">
      <motion.a
        variants={rise}
        {...presence}
        href="#/"
        className="flex w-fit items-center gap-0.5 rounded-md py-1 pr-2 text-sm text-ink-3 transition-colors hover:text-ink"
      >
        <ChevronLeft size={15} aria-hidden /> Today
      </motion.a>
      <AnimatePresence mode="wait">
        {error && (
          <motion.p key="error" variants={rise} {...presence} className="text-sm text-danger">
            Couldn't load this topic: {error}
          </motion.p>
        )}
        {topic && (
          <motion.header key="header" variants={rise} {...presence} className="flex flex-col gap-1.5">
            <h1 className="text-2xl font-semibold tracking-tight text-ink">{topic.title}</h1>
            <p className="flex items-start gap-1.5 text-sm text-ink-3">
              <Target size={15} className="mt-0.5 shrink-0 text-ink-4" aria-label="Your goal" />
              <span>“{topic.goal}”</span>
            </p>
          </motion.header>
        )}
      </AnimatePresence>
      {topic?.status === "confirmed" && <OutcomesCard topic={topic} />}
    </div>
  );
}

const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? "" : "s"}`;

// What you'll be able to do: every outcome, enumerated, with how its
// practice questions are coming along. The topic's status sits in the
// card's header.
function OutcomesCard({ topic }: { topic: TopicView }) {
  const ready = topic.outcomes.length > 0 && topic.outcomes.every((o) => o.bank_state === "ready");
  return (
    <motion.section
      variants={rise}
      {...presence}
      aria-labelledby="outcomes-title"
      className="overflow-hidden rounded-xl border border-hairline bg-subtle"
    >
      <div className="flex flex-col gap-1 border-b border-hairline px-5 py-3.5">
        <div className="flex items-center gap-3">
          <h2 id="outcomes-title" className="flex-1 text-sm font-semibold text-ink">
            What you'll be able to do
          </h2>
          <AnimatePresence mode="wait" initial={false}>
            {ready ? (
              <motion.span
                key="ready"
                variants={rise}
                {...presence}
                className="flex items-center gap-1.5 text-xs text-success"
              >
                <CircleCheck size={14} aria-hidden /> Ready · {plural(topic.active, "question")}
              </motion.span>
            ) : (
              <motion.span
                key="preparing"
                variants={rise}
                {...presence}
                className="flex items-center gap-1.5 text-xs text-accent"
              >
                <LoaderCircle size={14} className="animate-spin" aria-hidden />
                Preparing ·{" "}
                <span className="tabular-nums">
                  {topic.active} of ~{topic.expected}
                </span>{" "}
                ready
              </motion.span>
            )}
          </AnimatePresence>
        </div>
        <AnimatePresence initial={false}>
          {!ready && (
            <motion.p key="leave" variants={rise} {...presence} className="text-xs text-ink-3">
              kizn is writing and checking practice questions. You can close it: Today will say when they're ready.
            </motion.p>
          )}
        </AnimatePresence>
      </div>
      <ol className="flex flex-col divide-y divide-hairline">
        <AnimatePresence initial={false}>
          {topic.outcomes.map((o, i) => (
            <OutcomeRow key={o.id} outcome={o} index={i} />
          ))}
        </AnimatePresence>
      </ol>
    </motion.section>
  );
}

function OutcomeRow({ outcome: o, index }: { outcome: OutcomeView; index: number }) {
  const state = o.bank_state;
  const dim = state === "waiting";
  const [open, setOpen] = useState(false);
  const listId = useId();
  return (
    <motion.li
      layout
      initial={{ opacity: 0, y: 6 }}
      animate={{
        opacity: dim ? 0.55 : 1,
        y: 0,
        transition: { duration: 0.24, ease: EASE, delay: Math.min(index, 6) * 0.04 },
      }}
      exit={{ opacity: 0, transition: { duration: 0.12 } }}
      className={`relative flex flex-col gap-1.5 px-5 py-4 ${state === "writing" ? "bg-raised/25" : ""}`}
    >
      <AnimatePresence>
        {state === "writing" && (
          <motion.span
            key="edge"
            initial={{ scaleY: 0 }}
            animate={{ scaleY: 1 }}
            exit={{ scaleY: 0 }}
            className="absolute inset-y-0 left-0 w-0.5 origin-top bg-accent"
            aria-hidden
          />
        )}
      </AnimatePresence>
      <div className="flex gap-3">
        <span className="w-4 shrink-0 text-sm tabular-nums text-ink-4">{o.ord}</span>
        <div className="flex min-w-0 flex-1 flex-col gap-1.5">
          <p className="text-sm text-ink">{o.statement}</p>
          <div className="flex items-start gap-3">
            <p className="flex-1 text-xs text-ink-3">
              Target: <span className="text-ink-2">{o.target_plain}</span>
              {o.reason && <span className="text-ink-4"> · {o.reason}</span>}
            </p>
            <button
              type="button"
              aria-expanded={open}
              aria-controls={listId}
              onClick={() => setOpen((v) => !v)}
              className="flex shrink-0 items-center gap-1 rounded px-1 text-xs text-ink-3 transition-colors hover:text-ink"
            >
              {plural(o.concepts.length, "concept")}
              <motion.span
                animate={{ rotate: open ? 180 : 0 }}
                transition={{ duration: 0.18, ease: EASE }}
                className="flex"
              >
                <ChevronDown size={13} aria-hidden />
              </motion.span>
            </button>
          </div>
          <AnimatePresence initial={false}>
            {open && (
              <motion.div
                key="concepts"
                id={listId}
                initial={{ height: 0, opacity: 0 }}
                animate={{
                  height: "auto",
                  opacity: 1,
                  transition: { height: { duration: 0.22, ease: EASE }, opacity: { duration: 0.18, delay: 0.05 } },
                }}
                exit={{
                  height: 0,
                  opacity: 0,
                  transition: { height: { duration: 0.18, ease: EASE }, opacity: { duration: 0.1 } },
                }}
                className="overflow-hidden"
                aria-label={`Concepts for outcome ${o.ord}`}
              >
                <ul className="mt-1 flex flex-col gap-1.5 border-l border-hairline-strong py-1 pl-3">
                  {o.concepts.map((c) => (
                    <li key={c.id} className="flex items-start gap-2 text-xs">
                      <Circle size={10} className="mt-[3px] shrink-0 text-ink-4" aria-hidden />
                      <span className="flex-1 text-ink-2">{c.statement}</span>
                    </li>
                  ))}
                </ul>
              </motion.div>
            )}
          </AnimatePresence>
          <div className="mt-1 flex items-center gap-4">
            <AnimatePresence initial={false}>
              {state !== "waiting" && (
                <motion.div key="bar" variants={rise} {...presence} className="min-w-0 flex-1">
                  <SlotBar slots={o.slots} target={o.target_plain} />
                </motion.div>
              )}
            </AnimatePresence>
            {state === "waiting" && <span className="flex-1" />}
            <AnimatePresence mode="wait" initial={false}>
              <motion.span key={state} variants={rise} {...presence} className="shrink-0 text-xs" aria-live="polite">
                <OutcomeStatus outcome={o} />
              </motion.span>
            </AnimatePresence>
          </div>
          <AnimatePresence initial={false}>
            {state === "failed" && <FailurePanel key="failure" outcome={o} />}
          </AnimatePresence>
        </div>
      </div>
    </motion.li>
  );
}

function OutcomeStatus({ outcome: o }: { outcome: OutcomeView }) {
  switch (o.bank_state) {
    case "ready":
      return (
        <span className="flex items-center gap-1.5 text-success">
          <CircleCheck size={13} aria-hidden /> {plural(o.active, "question")} ready
        </span>
      );
    case "writing":
      return (
        <span className="flex items-center gap-1.5 text-accent">
          <LoaderCircle size={13} className="animate-spin" aria-hidden />
          Writing…{" "}
          <span className="tabular-nums">
            {o.active} of {o.expected}
          </span>
        </span>
      );
    case "failed":
      return (
        <span className="flex items-center gap-1.5 text-warn">
          <CircleAlert size={13} aria-hidden /> Can't write questions yet
        </span>
      );
    default:
      return <span className="text-ink-3">Waiting its turn</span>;
  }
}

const SLOT_LABEL: Record<string, string> = {
  recall: "Recall",
  apply: "Use it",
  target: "At the target",
  explain: "Explain it back",
};

// One group per kind of practice the outcome gets: recall for each concept,
// then using it, the target, explaining it back. Each group's cells fill as
// questions go live.
function SlotBar({ slots, target }: { slots: SlotView[]; target: string }) {
  return (
    <ul className="flex items-center gap-1.5" aria-label="Practice questions by kind">
      {slots.map((slot, i) => {
        const label =
          slot.kind === "recall"
            ? `${SLOT_LABEL.recall}: ${slot.concept ?? ""}`
            : slot.kind === "target"
              ? `${SLOT_LABEL.target} (${target})`
              : (SLOT_LABEL[slot.kind] ?? slot.kind);
        return (
          <li
            // biome-ignore lint/suspicious/noArrayIndexKey: slots are positional
            key={i}
            title={`${label} · ${slot.have} of ${slot.size}`}
            aria-label={`${label}: ${slot.have} of ${slot.size}`}
            className={`flex gap-0.5 ${slot.kind !== "recall" && slots[i - 1]?.kind === "recall" ? "ml-1.5" : ""}`}
          >
            {Array.from({ length: slot.size }, (_, j) => (
              <span
                // biome-ignore lint/suspicious/noArrayIndexKey: cells are positional
                key={j}
                className="h-1.5 w-2.5 overflow-hidden rounded-xs bg-raised"
              >
                <motion.span
                  className="block h-full rounded-xs bg-accent"
                  initial={false}
                  animate={{ width: j < slot.have ? "100%" : "0%" }}
                  transition={{ duration: 0.3, ease: EASE, delay: j * 0.03 }}
                />
              </span>
            ))}
          </li>
        );
      })}
    </ul>
  );
}

// Why an outcome's questions can't be written, in plain words, with a way to
// try again now (kizn also retries by itself).
function FailurePanel({ outcome: o }: { outcome: OutcomeView }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const retry = async () => {
    setBusy(true);
    setError(null);
    try {
      await call({ type: "retry_bank", outcome_id: o.id });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const reason = o.bank_error ?? "something went wrong writing questions";
  return (
    <motion.div
      variants={rise}
      {...presence}
      className="mt-1 flex flex-col gap-2 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2.5 text-xs"
    >
      <p className="text-ink-2">
        {reason.charAt(0).toUpperCase()}
        {reason.slice(1)}
        {/[.!?]$/.test(reason) ? "" : "."} <span className="text-ink-3">kizn will try again in a few minutes.</span>
      </p>
      {error && <p className="text-danger">{error}</p>}
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={retry}
          disabled={busy}
          className="flex items-center gap-1.5 rounded-md bg-warn/15 px-2.5 py-1 font-medium text-warn transition-colors hover:bg-warn/25 disabled:opacity-60"
        >
          <RotateCw size={12} className={busy ? "animate-spin" : ""} aria-hidden /> Try now
        </button>
        <a
          href="#/settings"
          className="rounded-md px-2 py-1 text-ink-3 transition-colors hover:bg-raised hover:text-ink"
        >
          Settings
        </a>
      </div>
    </motion.div>
  );
}
