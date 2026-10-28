import {
  BookOpen,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  LoaderCircle,
  Play,
  RotateCw,
  Trash2,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useState } from "react";
import type { ConceptView } from "../bindings/ConceptView";
import type { OutcomeView } from "../bindings/OutcomeView";
import { ConceptMark } from "../components/ConceptMark";
import { Confirm } from "../components/Confirm";
import { Reveal } from "../components/Reveal";
import { Screen } from "../components/Screen";
import { Stage } from "../components/Stage";
import { Tip } from "../components/Tip";
import { subscribe } from "../events";
import { settingsCanFix } from "../fixable";
import { bead, EASE, pop, presence, rise, stage } from "../motion";
import { whenDue } from "../practice/Summary";
import { call, type TopicView } from "../rpc";
import { ConceptSheet } from "../study/ConceptSheet";
import { ReadingList } from "../study/ReadingList";
import { Flags } from "./Flags";
import { MapLinks } from "./MapLinks";

// A topic. The main column answers "what now?" (Next up) and "what do I
// know?" (Learn these, Your concepts); the sidebar holds the quiet rest (the
// goal, the counts, kizn's question writing). Any concept's reading is one
// click away, so the topic can be explored between sessions.
export function TopicPage({ topicId, from }: { topicId: string; from?: "map" }) {
  const [topic, setTopic] = useState<TopicView | null>(null);
  const [error, setError] = useState<string | null>(null);
  // The concept open in the sheet, to read about it.
  const [concept, setConcept] = useState<string | null>(null);
  const closeConcept = useCallback(() => setConcept(null), []);
  // "Start reading": everything to study in this topic.
  const [readingAll, setReadingAll] = useState(false);
  // (Marking studied sends topic_changed, which reloads the page.)
  const closeReadingAll = useCallback(() => setReadingAll(false), []);

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

  const confirmed = topic?.status === "confirmed" ? topic : null;
  // A foundation kizn is drafting by itself: nothing to do but wait a little.
  const preparing = topic?.status === "draft" && topic.origin === "kizn" ? topic : null;
  return (
    <Screen
      width="max-w-5xl"
      header={
        <>
          <motion.a
            variants={rise}
            {...presence}
            href={from === "map" ? "#/map" : "#/"}
            className="flex w-fit items-center gap-0.5 rounded-md py-1 pr-2 text-sm text-ink-3 transition-colors hover:text-ink"
          >
            <ChevronLeft size={15} aria-hidden /> {from === "map" ? "Map" : "Today"}
          </motion.a>
          <AnimatePresence mode="wait">
            {error && (
              <motion.p key="error" variants={rise} {...presence} className="text-sm text-danger">
                Couldn't load this topic: {error}
              </motion.p>
            )}
            {topic && (
              <motion.h1
                key="title"
                variants={rise}
                {...presence}
                className="text-2xl font-semibold tracking-tight text-ink"
              >
                {topic.title}
              </motion.h1>
            )}
          </AnimatePresence>
        </>
      }
    >
      <AnimatePresence>{preparing && <Preparing key="preparing" topic={preparing} />}</AnimatePresence>
      {confirmed && (
        <Stage stagger={0.12} className="grid grid-cols-1 gap-10 lg:grid-cols-[minmax(0,1fr)_15rem]">
          <motion.div variants={stage(0.14, 0)} className="flex min-w-0 flex-col gap-8">
            <NextUp topic={confirmed} onReadAll={() => setReadingAll(true)} />
            <Flags flags={confirmed.flags} />
            <LearnThese topic={confirmed} onOpen={setConcept} />
            <YourConcepts topic={confirmed} onOpen={setConcept} />
          </motion.div>
          <Sidebar topic={confirmed} from={from} />
        </Stage>
      )}
      <ConceptSheet conceptId={concept} onClose={closeConcept} />
      {confirmed && <ReadingList open={readingAll} topicId={confirmed.id} onClose={closeReadingAll} />}
    </Screen>
  );
}

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
const concepts = (t: TopicView) => t.outcomes.flatMap((o) => o.concepts);
const toLearn = (t: TopicView) => concepts(t).filter((c) => c.state === "seen");

// A concept's mark colour: to learn (seen) is warm, known is teal.
const MARK_TONE: Record<string, string> = { seen: "text-warn", recalled: "text-teal", retained: "text-teal" };

// The one thing to do now, and the page's only primary button.
function NextUp({ topic: t, onReadAll }: { topic: TopicView; onReadAll: () => void }) {
  const practise = `#/practice/${encodeURIComponent(t.id)}`;
  const unread = toLearn(t).filter((c) => !c.studied);
  const cards = Number(t.due_now) + Number(t.new_now);
  const minutes = Math.max(1, Math.round(cards * 0.6));

  let title: string;
  let line: string;
  let action: { label: string; href?: string; onClick?: () => void; icon: typeof Play } | null = null;
  if (!t.practised && !t.can_practise) {
    title = "Writing your first questions";
    line = "Your starting point is ready once every concept on outcome 1 has a question. You can close kizn meanwhile.";
  } else if (!t.practised) {
    title = "Find your starting point";
    line = `${plural(cards, "question")} on outcome ${t.new_outcome_ord ?? 1}. Misses are fine: they show where to begin.`;
    action = { label: "Start", href: practise, icon: Play };
  } else if (t.can_practise) {
    const due = Number(t.due_now);
    const fresh = Number(t.new_now);
    title =
      due > 0
        ? `${plural(due, "recall is", "recalls are")} due`
        : `Outcome ${t.new_outcome_ord ?? ""}'s starting point is ready`;
    line =
      due > 0 && fresh > 0
        ? `Then ${plural(fresh, "new concept")} from outcome ${t.new_outcome_ord}. About ${minutes} min.`
        : due > 0
          ? `Each on a different question from last time. About ${minutes} min.`
          : `${plural(fresh, "new concept")}. About ${minutes} min.`;
    action = { label: "Practise now", href: practise, icon: Play };
  } else {
    const days = t.next_due_in_days === null ? null : Number(t.next_due_in_days);
    title =
      days === null || days <= 0
        ? "Nothing due"
        : days === 1
          ? "Nothing due until tomorrow"
          : `Nothing due for ${days} days`;
    if (unread.length > 0) {
      line = `Meanwhile, read up on the ${plural(unread.length, "concept")} you missed.`;
      action = { label: "Start reading", onClick: onReadAll, icon: BookOpen };
    } else {
      line = "You're up to date. Your concepts are below if you want to read ahead.";
    }
  }

  const Icon = action?.icon;
  return (
    <motion.section
      variants={pop}
      aria-labelledby="next-title"
      className="flex flex-col gap-4 rounded-2xl bg-accent/[0.07] px-6 py-5 ring-1 ring-accent/15 sm:flex-row sm:items-center"
    >
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <h2 id="next-title" className="text-[11px] font-semibold uppercase tracking-wide text-accent">
          Next up
        </h2>
        <AnimatePresence mode="wait" initial={false}>
          <motion.div key={title} variants={rise} {...presence} className="flex flex-col gap-1">
            <p className="flex items-center gap-2 text-lg font-semibold tracking-tight text-ink">
              {!t.practised && !t.can_practise && (
                <LoaderCircle size={16} className="animate-spin text-accent" aria-hidden />
              )}
              {title}
            </p>
            <p className="text-sm text-ink-3">{line}</p>
          </motion.div>
        </AnimatePresence>
      </div>
      <AnimatePresence mode="wait" initial={false}>
        {action && Icon && (
          <motion.div key={action.label} variants={rise} {...presence} className="shrink-0">
            {action.href ? (
              <a
                href={action.href}
                className="flex items-center gap-1.5 rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-canvas transition-opacity hover:opacity-90"
              >
                <Icon size={14} aria-hidden /> {action.label}
              </a>
            ) : (
              <button
                type="button"
                onClick={action.onClick}
                className="flex items-center gap-1.5 rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-canvas transition-opacity hover:opacity-90"
              >
                <Icon size={14} aria-hidden /> {action.label}
              </button>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </motion.section>
  );
}

// The concepts you missed, to come back to their reading. Only there when
// there's something in it.
function LearnThese({ topic, onOpen }: { topic: TopicView; onOpen: (conceptId: string) => void }) {
  const learn = toLearn(topic);
  return (
    <AnimatePresence>
      {learn.length > 0 && (
        <motion.section
          key="learn"
          variants={stage(0.07, 0.12)}
          initial="hidden"
          animate="shown"
          exit="gone"
          aria-labelledby="learn-title"
          className="flex flex-col gap-1.5"
        >
          <motion.h2 variants={pop} id="learn-title" className="px-3 text-sm font-semibold text-ink">
            Learn these
          </motion.h2>
          <ul className="flex flex-col">
            <AnimatePresence>
              {learn.map((c) => (
                <motion.li key={c.id} layout variants={pop} exit="gone">
                  <ConceptRow concept={c} onOpen={onOpen} trailing />
                </motion.li>
              ))}
            </AnimatePresence>
          </ul>
        </motion.section>
      )}
    </AnimatePresence>
  );
}

// One concept: its mark and statement; opens its reading.
function ConceptRow({
  concept: c,
  onOpen,
  trailing = false,
  small = false,
}: {
  concept: ConceptView;
  onOpen: (conceptId: string) => void;
  /** Shows studied (✓) or a chevron at the end. */
  trailing?: boolean;
  small?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={() => onOpen(c.id)}
      className={`group flex w-full items-center gap-2.5 rounded-lg px-3 text-left transition-colors hover:bg-raised/50 ${small ? "py-1.5 text-[13px]" : "py-2 text-sm"}`}
    >
      <span className={`shrink-0 ${MARK_TONE[c.state] ?? "text-ink-4"}`}>
        <ConceptMark state={c.state} size={small ? 12 : 14} />
      </span>
      <span className={`flex-1 ${small ? "text-ink-2 group-hover:text-ink" : "text-ink"}`}>{c.statement}</span>
      {trailing &&
        (c.studied ? (
          <Check size={14} className="shrink-0 text-success" aria-label="Studied" />
        ) : (
          <ChevronRight size={14} className="shrink-0 text-ink-4 transition-colors group-hover:text-ink" aria-hidden />
        ))}
    </button>
  );
}

// Every outcome as one line: its number, statement and a strip of concept
// marks. Expanding shows its concepts, each opening its reading. The outcome
// being worked starts open.
function YourConcepts({ topic: t, onOpen }: { topic: TopicView; onOpen: (conceptId: string) => void }) {
  const working =
    t.outcomes.find((o) => o.concepts.some((c) => c.state === "seen"))?.id ??
    t.outcomes.find((o) => o.ord === t.new_outcome_ord)?.id ??
    t.outcomes[0]?.id;
  const [open, setOpen] = useState<Set<string>>(() => new Set(working ? [working] : []));
  const toggle = (id: string) =>
    setOpen((s) => {
      const next = new Set(s);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  return (
    <motion.section variants={stage(0.08, 0)} aria-labelledby="concepts-title" className="flex flex-col gap-1.5">
      <motion.h2 variants={pop} id="concepts-title" className="px-3 text-sm font-semibold text-ink">
        Your concepts
      </motion.h2>
      <motion.ol variants={stage(0.08, 0)} className="flex flex-col">
        {t.outcomes.map((o, i) => (
          <OutcomeLine
            key={o.id}
            outcome={o}
            index={i}
            open={open.has(o.id)}
            onToggle={() => toggle(o.id)}
            onOpen={onOpen}
          />
        ))}
      </motion.ol>
    </motion.section>
  );
}

function OutcomeLine({
  outcome: o,
  index,
  open,
  onToggle,
  onOpen,
}: {
  outcome: OutcomeView;
  index: number;
  open: boolean;
  onToggle: () => void;
  onOpen: (conceptId: string) => void;
}) {
  return (
    <motion.li variants={pop} className="flex flex-col">
      <button
        type="button"
        aria-expanded={open}
        onClick={onToggle}
        className="group flex w-full items-start gap-3 rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-raised/50"
      >
        <span className="w-4 shrink-0 pt-px text-sm tabular-nums text-ink-4">{o.ord}</span>
        <span className="flex min-w-0 flex-1 flex-col gap-1.5">
          <span className="text-sm text-ink">{o.statement}</span>
          <motion.span
            variants={stage(0.035, 0.18 + index * 0.05)}
            className="flex items-center gap-1"
            role="img"
            aria-label={`${o.concepts.length} concepts`}
          >
            {o.concepts.map((c) => (
              <motion.span key={c.id} variants={bead} className={`inline-flex ${MARK_TONE[c.state] ?? "text-ink-4"}`}>
                <ConceptMark state={c.state} size={12} />
              </motion.span>
            ))}
          </motion.span>
        </span>
        <motion.span
          animate={{ rotate: open ? 180 : 0 }}
          transition={{ duration: 0.18, ease: EASE }}
          className="mt-0.5 flex shrink-0 text-ink-4 group-hover:text-ink"
        >
          <ChevronDown size={14} aria-hidden />
        </motion.span>
      </button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.ul
            key="concepts"
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
            className="ml-7 overflow-hidden"
            aria-label={`Concepts for outcome ${o.ord}`}
          >
            {o.concepts.map((c) => (
              <li key={c.id}>
                <ConceptRow concept={c} onOpen={onOpen} small />
              </li>
            ))}
          </motion.ul>
        )}
      </AnimatePresence>
    </motion.li>
  );
}

// The quiet rest: the goal, the counts at a glance, and kizn's question
// writing (which only asks for attention when it's stuck).
function Sidebar({ topic: t, from }: { topic: TopicView; from?: "map" }) {
  const all = concepts(t);
  const count = (s: string) => all.filter((c) => c.state === s).length;
  const rows: [string, number, string][] = [
    ["retained", count("retained"), "stuck"],
    ["recalled", count("recalled"), "recalled"],
    ["seen", count("seen"), "seen"],
    ["unseen", count("unseen"), "not started"],
  ];
  const failed = t.outcomes.filter((o) => o.bank_state === "failed");
  const resting = t.outcomes.filter((o) => o.bank_state === "resting");
  const when = whenDue(t.next_due_in_days);
  const pct = t.expected > 0 ? Math.min(100, (Number(t.active) / Number(t.expected)) * 100) : 0;
  return (
    <motion.aside
      variants={stage(0.09, 0.25)}
      className="flex flex-col gap-7 text-sm lg:pt-1"
      aria-label="About this topic"
    >
      <motion.p variants={pop} className="text-ink-3">
        “{t.goal}”
      </motion.p>
      <motion.div variants={stage(0.06, 0)} className="flex flex-col gap-1.5">
        {rows
          .filter(([, n]) => n > 0)
          .map(([state, n, word]) => (
            <motion.p key={state} variants={pop} className="flex items-center gap-2 text-ink-3">
              <span className={MARK_TONE[state] ?? "text-ink-4"}>
                <ConceptMark state={state} size={12} />
              </span>
              <span className="w-6 tabular-nums text-ink-2">{n}</span>
              {word}
            </motion.p>
          ))}
        {when && t.practised && (
          <motion.p variants={pop} className="pt-1 text-xs text-ink-4">
            Next recall {when}
          </motion.p>
        )}
      </motion.div>
      <motion.div variants={pop} className="flex flex-col gap-1.5">
        <p className="text-xs text-ink-4">Questions ready</p>
        <div className="flex items-center gap-2">
          <span className="h-1 flex-1 overflow-hidden rounded-full bg-raised">
            <motion.span
              className="block h-full rounded-full bg-ink-4"
              initial={false}
              animate={{ width: `${pct}%` }}
              transition={{ duration: 0.4, ease: EASE }}
            />
          </span>
          <span className="text-xs tabular-nums text-ink-4">
            {t.active} of ~{t.expected}
          </span>
        </div>
        <AnimatePresence initial={false}>
          {resting.length > 0 && <Resting key="resting" outcomes={resting} />}
          {failed.map((o) => (
            <Stuck key={o.id} outcome={o} />
          ))}
        </AnimatePresence>
      </motion.div>
      <MapLinks topic={t} />
      <DeleteTopic topic={t} back={from === "map" ? "#/map" : "#/"} />
    </motion.aside>
  );
}

// "Outcomes 3, 4 and 5".
const outcomeList = (os: OutcomeView[]) => {
  const ords = os.map((o) => String(o.ord));
  const last = ords.pop();
  return `${os.length === 1 ? "Outcome" : "Outcomes"} ${ords.length ? `${ords.join(", ")} and ${last}` : last}`;
};

// When kizn tries again, in the learner's own clock ("10:37 PM").
const retryTime = (at: string | null) => {
  if (!at) return null;
  const when = new Date(`${at.replace(" ", "T")}Z`);
  if (Number.isNaN(when.getTime()) || when.getTime() <= Date.now()) return "any moment now";
  return `around ${when.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`;
};

// Try writing again now (kizn also does by itself), as a small icon.
function TryNow({ outcomes, onError }: { outcomes: OutcomeView[]; onError: (e: string | null) => void }) {
  const [busy, setBusy] = useState(false);
  const retry = async () => {
    setBusy(true);
    onError(null);
    try {
      for (const o of outcomes) await call({ type: "retry_bank", outcome_id: o.id });
    } catch (e) {
      onError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <button
      type="button"
      onClick={retry}
      disabled={busy}
      aria-label="Try now"
      title="Try now"
      className="rounded p-0.5 text-ink-4 transition-colors hover:text-ink-2 disabled:opacity-60"
    >
      <RotateCw size={11} className={busy ? "animate-spin" : ""} aria-hidden />
    </button>
  );
}

// Outcomes whose questions haven't passed the checks yet: kizn leaves them a
// while and tries again by itself, so this asks nothing of the learner.
function Resting({ outcomes }: { outcomes: OutcomeView[] }) {
  const [error, setError] = useState<string | null>(null);
  const next =
    outcomes
      .map((o) => o.retry_at)
      .filter((r): r is string => !!r)
      .sort()[0] ?? null;
  return (
    <motion.div variants={rise} {...presence} className="mt-1.5 flex flex-col gap-1 text-xs">
      <p className="flex items-center gap-1 text-ink-4">
        <span className="flex-1">{outcomeList(outcomes)}: more questions later</span>
        <Tip label="Why later">
          None of the author's questions for {outcomes.length === 1 ? "this outcome" : "these"} passed kizn's checks
          yet. kizn tries again by itself, {retryTime(next)}.
        </Tip>
        <TryNow outcomes={outcomes} onError={setError} />
      </p>
      <Reveal when={error}>
        <p className="text-danger">{error}</p>
      </Reveal>
    </motion.div>
  );
}

// Writing stopped on an outcome for a reason the learner may need to fix (an
// account, a connection): kept short, the reason in a tip.
function Stuck({ outcome: o }: { outcome: OutcomeView }) {
  const [error, setError] = useState<string | null>(null);
  const reason = o.bank_error ?? "something went wrong writing questions";
  return (
    <motion.div variants={rise} {...presence} className="mt-1.5 flex flex-col gap-1 text-xs">
      <p className="flex items-center gap-1 text-warn">
        <span className="flex-1">Outcome {o.ord}: paused</span>
        <Tip label="Why paused">
          {reason.charAt(0).toUpperCase() + reason.slice(1)}
          {/[.!?]$/.test(reason) ? "" : "."} kizn tries again by itself, {retryTime(o.retry_at)}.
        </Tip>
        <TryNow outcomes={[o]} onError={setError} />
        {settingsCanFix(reason) && (
          <a href="#/settings" className="rounded px-1 text-ink-3 transition-colors hover:text-ink">
            Settings
          </a>
        )}
      </p>
      <Reveal when={error}>
        <p className="text-danger">{error}</p>
      </Reveal>
    </motion.div>
  );
}

// Letting the topic go, quietly at the foot of the sidebar, asked first.
function DeleteTopic({ topic: t, back }: { topic: TopicView; back: string }) {
  const [asking, setAsking] = useState(false);
  const close = useCallback(() => setAsking(false), []);
  return (
    <motion.div variants={pop}>
      <button
        type="button"
        onClick={() => setAsking(true)}
        className="flex items-center gap-1.5 rounded px-1 py-0.5 text-xs text-ink-4 transition-colors hover:text-danger"
      >
        <Trash2 size={12} aria-hidden /> Delete topic
      </button>
      <Confirm
        open={asking}
        title={`Delete ${t.title}?`}
        action="Delete topic"
        onClose={close}
        onConfirm={async () => {
          await call({ type: "delete_topic", topic_id: t.id });
          window.location.hash = back;
        }}
      >
        Its questions, your practice, reading and notes go with it, and this can't be undone. Topics it's linked to stay
        on your map.
      </Confirm>
    </motion.div>
  );
}

// kizn is setting the topic up by itself (a foundation the check found
// shaky): how far it's got, and that nothing's needed from the learner.
function Preparing({ topic: t }: { topic: TopicView }) {
  const concepts = t.outcomes.reduce((n, o) => n + o.concepts.length, 0);
  return (
    <motion.section
      variants={rise}
      {...presence}
      aria-label="Being prepared"
      className="flex flex-col gap-2 rounded-2xl bg-subtle px-6 py-5 ring-1 ring-hairline"
    >
      <p className="text-[11px] font-semibold uppercase tracking-wider text-accent">Being prepared</p>
      <h2 className="text-lg font-semibold text-ink">kizn is setting up {t.title}</h2>
      <p className="flex items-center gap-2 text-sm text-ink-3">
        <LoaderCircle size={14} className="animate-spin text-accent" aria-hidden />
        {t.outcomes.length === 0
          ? "Drafting what to learn…"
          : `${plural(t.outcomes.length, "outcome")} and ${plural(concepts, "concept")} so far…`}
      </p>
      <p className="text-sm text-ink-3">
        It's a foundation for what you're learning, set up in its own terms. Its first questions follow; you can close
        kizn meanwhile.
      </p>
    </motion.section>
  );
}
