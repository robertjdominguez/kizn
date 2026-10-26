import { ChevronRight, CircleCheck, LoaderCircle, PencilLine, Plug, Plus, Sprout } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useState } from "react";
import type { TopicSummary } from "../bindings/TopicSummary";
import { subscribe } from "../events";
import { presence, rise } from "../motion";
import { call, type TodayView } from "../rpc";

// Home: whether a model is connected, and your topics with how their
// practice is coming along. Due recalls, fringe outcomes, flags and the
// Practice button arrive with the phases that make them.
export function Today() {
  const [today, setToday] = useState<TodayView | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    call({ type: "get_today" })
      .then(setToday)
      .catch((e: Error) => setError(e.message));
  }, []);

  // Follows topics as they're drafted and their questions are written.
  useEffect(() => {
    load();
    return subscribe((e) => {
      if (e.type === "topic_changed" || e.type === "config_changed") load();
    });
  }, [load]);

  if (error) {
    return (
      <motion.p variants={rise} initial="hidden" animate="shown" className="p-6 text-sm text-danger">
        Couldn't load Today: {error}
      </motion.p>
    );
  }
  if (!today) return null;

  return (
    <div className="mx-auto w-full max-w-2xl px-6 py-10 flex flex-col gap-6">
      <AnimatePresence initial={false}>
        {!today.model_configured && (
          <motion.section
            key="model"
            variants={rise}
            {...presence}
            aria-label="Model"
            className="flex items-start gap-3 rounded-lg border border-warn/40 bg-warn/5 px-4 py-3 text-sm"
          >
            <Plug size={16} className="mt-0.5 shrink-0 text-warn" aria-hidden />
            <div className="flex flex-col gap-0.5">
              <p className="font-medium text-ink">No model connected</p>
              <p className="text-ink-3">
                kizn writes practice questions and tutors with a model you choose: a local one or a provider you have a
                key for.{" "}
                <a href="#/settings" className="text-accent hover:underline">
                  Connect a model
                </a>
              </p>
            </div>
          </motion.section>
        )}
      </AnimatePresence>

      <AnimatePresence initial={false}>
        {today.topics.length > 0 && (
          <motion.section
            key="topics"
            variants={rise}
            {...presence}
            aria-label="Your topics"
            className="flex flex-col gap-2"
          >
            <div className="flex items-center">
              <h2 className="flex-1 text-[11px] font-medium uppercase tracking-wide text-ink-4">Your topics</h2>
              <a
                href="#/new-topic"
                className="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-accent transition-colors hover:bg-accent/10"
              >
                <Plus size={13} aria-hidden /> Start a topic
              </a>
            </div>
            <ul className="flex flex-col gap-2">
              <AnimatePresence initial={false}>
                {today.topics.map((t) => (
                  <TopicCard key={t.id} topic={t} />
                ))}
              </AnimatePresence>
            </ul>
          </motion.section>
        )}
      </AnimatePresence>

      <AnimatePresence initial={false}>
        {today.topics.length === 0 && (
          <motion.section
            key="empty"
            variants={rise}
            {...presence}
            aria-label="Topics"
            className="flex flex-col items-center gap-4 rounded-lg border border-hairline bg-subtle px-6 py-12 text-center"
          >
            <Sprout size={28} className="text-teal" aria-hidden />
            <div className="flex flex-col gap-1">
              <h2 className="text-base font-medium text-ink">Nothing to practise yet</h2>
              <p className="max-w-sm text-sm text-ink-3">
                Say what you want to be able to do and why. kizn drafts outcomes and concepts for you to edit, then
                writes practice for them.
              </p>
            </div>
            <a
              href="#/new-topic"
              className="rounded-md bg-teal/15 px-3 py-1.5 text-sm font-medium text-teal transition-colors hover:bg-teal/25"
            >
              Start a topic
            </a>
          </motion.section>
        )}
      </AnimatePresence>
    </div>
  );
}

const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? "" : "s"}`;

function TopicCard({ topic: t }: { topic: TopicSummary }) {
  return (
    <motion.li variants={rise} {...presence} layout>
      <a
        href={t.status === "draft" ? `#/setup/${encodeURIComponent(t.id)}` : `#/topic/${encodeURIComponent(t.id)}`}
        className="group flex items-center gap-4 rounded-lg border border-hairline bg-subtle px-4 py-3 transition-colors hover:border-accent/40 hover:bg-raised/30"
      >
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="truncate text-sm font-medium text-ink">{t.title}</span>
          <span className="text-xs text-ink-3">
            {plural(Number(t.outcomes), "outcome")} · {plural(Number(t.concepts), "concept")}
          </span>
        </div>
        <AnimatePresence mode="wait" initial={false}>
          <motion.span key={status(t)} variants={rise} {...presence} className="shrink-0 text-xs">
            <TopicStatus topic={t} />
          </motion.span>
        </AnimatePresence>
        <ChevronRight size={15} className="shrink-0 text-ink-4 transition-colors group-hover:text-accent" aria-hidden />
      </a>
    </motion.li>
  );
}

const status = (t: TopicSummary) => (t.status === "draft" ? "draft" : t.preparing ? "preparing" : "ready");

/** An always-visible anchor (icon and a word); its detail grows in beside it
 *  while the status is hovered, or the card has keyboard focus. CSS only: a
 *  grid column that animates from zero width to its content.
 *
 *  Hover intent: the hit area is padded (with matching negative margins, so
 *  nothing moves) and the detail opens after 75 ms but closes only after
 *  200 ms, so brushing past or slipping off an edge doesn't make it flash. */
function TopicStatus({ topic: t }: { topic: TopicSummary }) {
  const s = status(t);
  const { icon, word, detail, spoken, tone } =
    s === "draft"
      ? {
          icon: <PencilLine size={13} aria-hidden />,
          word: "Draft",
          detail: "finish setting it up",
          spoken: "Draft: finish setting it up",
          tone: "text-ink-3",
        }
      : s === "preparing"
        ? {
            icon: <LoaderCircle size={13} className="animate-spin" aria-hidden />,
            word: "Preparing",
            detail: `${t.active} of ~${t.expected} ready`,
            spoken: `Preparing: ${t.active} of about ${t.expected} questions ready`,
            tone: "text-accent",
          }
        : {
            icon: <CircleCheck size={13} aria-hidden />,
            word: "Ready",
            detail: plural(Number(t.active), "question"),
            spoken: `Ready: ${plural(Number(t.active), "question")}`,
            tone: "text-success",
          };
  return (
    <span className={`group/status -mx-2 -my-2.5 flex items-center gap-1.5 px-2 py-2.5 ${tone}`}>
      {icon}
      <span aria-hidden>{word}</span>
      <span
        aria-hidden
        className="grid grid-cols-[0fr] opacity-0 transition-[grid-template-columns,opacity] delay-200 duration-200 ease-out group-hover/status:grid-cols-[1fr] group-hover/status:opacity-100 group-hover/status:delay-75 group-focus-visible:grid-cols-[1fr] group-focus-visible:opacity-100 group-focus-visible:delay-0"
      >
        <span className="overflow-hidden whitespace-nowrap tabular-nums">· {detail}</span>
      </span>
      <span className="sr-only">{spoken}</span>
    </span>
  );
}
