import { BookOpen, LoaderCircle, PencilLine, Play, Plug, Plus, Sparkles } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { type ReactNode, useCallback, useEffect, useState } from "react";
import type { NextStep } from "../bindings/NextStep";
import type { TopicSummary } from "../bindings/TopicSummary";
import { ConceptMark } from "../components/ConceptMark";
import { Screen } from "../components/Screen";
import { Stage } from "../components/Stage";
import { subscribe } from "../events";
import { bead, EASE, button as landing, pop, popStage, SPRING, stage } from "../motion";
import { whenDue } from "../practice/Summary";
import { call, type TodayView } from "../rpc";
import { ReadingList } from "../study/ReadingList";
import { Flags } from "../topic/Flags";

// Home. Calm first, then one invitation: a greeting, the one next step
// (warm, with how small it is), the week so far, and your topics, quieter.
// Never scolds: no streaks, no overdue counts. When nothing's due it says so
// and lets you rest.
//
// It assembles itself in reading order: the greeting word by word, the date,
// the hero and then its words and button, the week's dots like beads, and the
// topic cards dealing their marks.
export function Today() {
  const [today, setToday] = useState<TodayView | null>(null);
  const [error, setError] = useState<string | null>(null);
  // "Start reading" opens everything to study.
  const [reading, setReading] = useState(false);
  const closeReading = useCallback(() => setReading(false), []);

  const load = useCallback(() => {
    call({ type: "get_today" })
      .then(setToday)
      .catch((e: Error) => setError(e.message));
  }, []);

  // Follows topics as they're drafted, written and practised.
  useEffect(() => {
    load();
    return subscribe((e) => {
      if (e.type === "topic_changed" || e.type === "config_changed") load();
    });
  }, [load]);

  if (error) {
    return (
      <Screen width="max-w-3xl">
        <p className="text-sm text-danger">Couldn't load Today: {error}</p>
      </Screen>
    );
  }
  if (!today) return null;

  const practisedEver =
    today.week.some((d) => d.practised) || today.counts.recalled + today.counts.retained + today.counts.to_learn > 0;
  return (
    <Screen width="max-w-3xl">
      <Stage className="flex flex-col gap-6" stagger={0.11}>
        <motion.header variants={stage(0.08, 0)} className="flex flex-col gap-1 pt-4">
          <h1 className="flex flex-wrap gap-x-[0.28em] text-3xl font-semibold tracking-tight text-ink">
            {greeting()
              .split(" ")
              .map((w) => (
                <motion.span key={w} variants={pop} className="inline-block">
                  {w}
                </motion.span>
              ))}
          </h1>
          <motion.p variants={pop} className="text-sm text-ink-4">
            {longDate(today.date)}
          </motion.p>
        </motion.header>

        <AnimatePresence>
          {!today.model_configured && (
            <motion.section
              key="model"
              variants={pop}
              aria-label="Model"
              className="flex items-start gap-3 rounded-xl bg-warn/[0.06] px-4 py-3 text-sm ring-1 ring-warn/25"
            >
              <Plug size={16} className="mt-0.5 shrink-0 text-warn" aria-hidden />
              <p className="text-ink-3">
                <span className="font-medium text-ink">No model connected</span>. kizn writes your practice and finds
                your reading with a model you choose: a local one, or a provider you have a key for.{" "}
                <a href="#/settings" className="text-accent hover:underline">
                  Connect a model
                </a>
              </p>
            </motion.section>
          )}
        </AnimatePresence>

        <Hero step={today.next} onRead={() => setReading(true)} />

        <Flags flags={today.flags} showTopic />

        <AnimatePresence>{practisedEver && <Week key="week" today={today} />}</AnimatePresence>

        <AnimatePresence>{today.topics.length > 0 && <Topics key="topics" topics={today.topics} />}</AnimatePresence>
      </Stage>

      <ReadingList open={reading} onClose={closeReading} />
    </Screen>
  );
}

const plural = (n: number | bigint, one: string, many = `${one}s`) => `${n} ${Number(n) === 1 ? one : many}`;
const minutes = (cards: number) => `about ${Math.max(1, Math.round(cards * 0.6))} min`;
const list = (xs: string[]) => (xs.length <= 1 ? (xs[0] ?? "") : `${xs.slice(0, -1).join(", ")} and ${xs.at(-1)}`);

function greeting() {
  const h = new Date().getHours();
  return h < 5 ? "Good evening." : h < 12 ? "Good morning." : h < 18 ? "Good afternoon." : "Good evening.";
}

// The learner's date (the app's, so the dev clock shift shows), in words.
function longDate(date: string) {
  const d = new Date(`${date}T12:00:00Z`);
  return d.toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long", timeZone: "UTC" });
}

type HeroContent = {
  eyebrow: string;
  title: string;
  line: string;
  note?: string;
  busy?: boolean;
  action?: { label: string; icon: typeof Play; href?: string; onClick?: () => void };
};

function heroFor(step: NextStep, onRead: (conceptId: string) => void): HeroContent {
  switch (step.kind) {
    case "start_topic":
      return {
        eyebrow: "Welcome",
        title: "What do you want to be able to do?",
        line: "Say it in your own words. kizn drafts what to learn, you shape it, and your first practice is ready in minutes.",
        action: { label: "Start a topic", icon: Plus, href: "#/new-topic" },
      };
    case "finish_setup":
      return {
        eyebrow: "Pick up where you left off",
        title: `Finish setting up ${step.title}`,
        line: "Shape what kizn drafted, then confirm, and it starts writing your practice.",
        action: { label: "Continue", icon: PencilLine, href: `#/setup/${encodeURIComponent(step.topic_id)}` },
      };
    case "preparing":
      return {
        eyebrow: "Almost there",
        title: `Writing your first questions for ${step.title}`,
        line: "Your starting point is ready in a few minutes. You can close kizn: it carries on next time.",
        busy: true,
      };
    case "practise": {
      const due = Number(step.due);
      const fresh = Number(step.new);
      const cards = due + fresh;
      if (step.first_set && due === 0) {
        return {
          eyebrow: "Ready when you are",
          title: `Find your starting point in ${step.topics[0] ?? "your topic"}`,
          line: `${plural(fresh, "question")}, ${minutes(cards)}. Misses are fine: they show where to begin.`,
          action: { label: "Begin", icon: Play, href: "#/practice" },
        };
      }
      if (due > 0) {
        return {
          eyebrow: "Ready when you are",
          title: `${plural(due, "recall is", "recalls are")} waiting`,
          line: `${list(step.topics)} · ${minutes(cards)}${fresh > 0 ? `, with ${plural(fresh, "new concept")}` : ""}`,
          note: "These are the ones that could stick today.",
          action: { label: "Begin", icon: Play, href: "#/practice" },
        };
      }
      return {
        eyebrow: "Ready when you are",
        title: "New concepts are ready",
        line: `${plural(fresh, "concept")} from ${list(step.topics)} · ${minutes(cards)}`,
        action: { label: "Begin", icon: Play, href: "#/practice" },
      };
    }
    case "learn": {
      const when = whenDue(step.next_due_in_days);
      return {
        eyebrow: "Something to read",
        title: "Read up on what you missed",
        line: `${plural(step.count, "concept")} still to read${when ? `; they come back ${when}` : ""}. Reading first makes that recall count.`,
        action: { label: "Start reading", icon: BookOpen, onClick: () => onRead(step.concept_id) },
        note: "See everything to study, and what there is for each.",
      };
    }
    case "nothing_due": {
      const when = whenDue(step.next_due_in_days);
      return {
        eyebrow: "All caught up",
        title: "You're all caught up",
        line: when
          ? `Your next recall is ${when}. Read ahead in any topic if you like, or start something new.`
          : "Read ahead in any topic if you like, or start something new.",
      };
    }
  }
}

// The one next step: warm, roomy, the screen's only primary button. The card
// lands first; its words follow, and the button arrives last with a bounce.
// When the step changes, the words swap in place.
function Hero({ step, onRead }: { step: NextStep; onRead: (conceptId: string) => void }) {
  const h = heroFor(step, onRead);
  const Icon = h.action?.icon;
  const cta =
    "flex items-center gap-2 rounded-xl bg-peach px-5 py-2.5 text-sm font-semibold text-crust shadow-sm shadow-peach/20 transition-opacity hover:opacity-95";
  return (
    <motion.section
      variants={pop}
      aria-labelledby="hero-title"
      className="drift relative overflow-hidden rounded-3xl bg-gradient-to-br from-peach/[0.14] via-rosewater/[0.06] to-mauve/[0.05] px-7 py-7 ring-1 ring-peach/20"
    >
      <AnimatePresence mode="wait">
        <motion.div
          key={`${step.kind}-${h.title}`}
          variants={stage(0.08, 0.18)}
          initial="hidden"
          animate="shown"
          exit="gone"
          className="flex flex-col gap-5 sm:flex-row sm:items-end"
        >
          <div className="flex min-w-0 flex-1 flex-col gap-2">
            <motion.p
              variants={pop}
              className="flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-peach"
            >
              <motion.span
                initial={{ rotate: -30, scale: 0 }}
                animate={{
                  rotate: [-30, 20, -8, 0],
                  scale: [0, 1.25, 1],
                  transition: { duration: 0.9, delay: 0.3, ease: EASE },
                }}
                className="inline-flex"
              >
                <Sparkles size={12} aria-hidden />
              </motion.span>
              {h.eyebrow}
            </motion.p>
            <motion.h2
              variants={pop}
              id="hero-title"
              className="flex items-center gap-2.5 text-2xl font-semibold tracking-tight text-ink"
            >
              {h.busy && <LoaderCircle size={20} className="shrink-0 animate-spin text-peach" aria-hidden />}
              {h.title}
            </motion.h2>
            <motion.p variants={pop} className="text-sm text-ink-2">
              {h.line}
            </motion.p>
            {h.note && (
              <motion.p variants={pop} className="text-sm text-ink-4">
                {h.note}
              </motion.p>
            )}
          </div>
          {h.action && Icon && (
            <motion.div variants={landing} whileHover={{ y: -2 }} whileTap={{ scale: 0.96 }} className="shrink-0">
              {h.action.href ? (
                <a href={h.action.href} className={cta}>
                  <Icon size={15} aria-hidden />
                  <span>{h.action.label}</span>
                </a>
              ) : (
                <button type="button" onClick={h.action.onClick} className={cta}>
                  <Icon size={15} aria-hidden />
                  <span>{h.action.label}</span>
                </button>
              )}
            </motion.div>
          )}
        </motion.div>
      </AnimatePresence>
    </motion.section>
  );
}

const DAY_LETTERS = ["M", "T", "W", "T", "F", "S", "S"];

// The week so far: days you practised as soft dots popping in one by one
// (never a streak that can break), and your concepts by state across topics.
function Week({ today }: { today: TodayView }) {
  const c = today.counts;
  const parts: [string, number, string][] = [
    ["retained", Number(c.retained), "stuck"],
    ["recalled", Number(c.recalled), "recalled"],
    ["seen", Number(c.to_learn), "seen"],
  ];
  return (
    <motion.section
      variants={popStage(0.05, 0.12)}
      aria-label="This week"
      className="flex flex-col gap-4 px-1 sm:flex-row sm:items-center sm:gap-10"
    >
      <div className="flex items-center gap-4">
        <motion.span variants={pop} className="text-sm font-medium text-ink-2">
          This week
        </motion.span>
        <motion.ol variants={stage(0.05, 0)} className="flex items-end gap-2">
          {today.week.map((d, i) => (
            <li key={d.date} className="flex flex-col items-center gap-1" title={d.date}>
              <motion.span
                variants={bead}
                className={`block size-2.5 rounded-full ${
                  d.practised ? "bg-peach" : d.future ? "bg-raised/50" : "ring-1 ring-inset ring-overlay"
                } ${d.today ? "outline-2 outline-offset-2 outline-peach/40" : ""}`}
                aria-label={`${d.date}${d.practised ? ": practised" : ""}`}
              />
              <span className={`text-[10px] ${d.today ? "text-ink-2" : "text-ink-4"}`}>{DAY_LETTERS[i]}</span>
            </li>
          ))}
        </motion.ol>
      </div>
      <p className="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm text-ink-3">
        {parts
          .filter(([, n]) => n > 0)
          .map(([state, n, word]) => (
            <motion.span key={state} variants={pop} className="flex items-center gap-1.5">
              <span className={state === "seen" ? "text-warn" : "text-teal"}>
                <ConceptMark state={state} size={12} />
              </span>
              <span className="tabular-nums text-ink-2">{n}</span> {word}
            </motion.span>
          ))}
      </p>
    </motion.section>
  );
}

// Your topics, quiet: a card each with its marks and one plain status; a
// dashed card to start another. Cards deal in, then each strings its marks.
function Topics({ topics }: { topics: TopicSummary[] }) {
  return (
    <motion.section variants={stage(0.09, 0)} aria-labelledby="topics-title" className="flex flex-col gap-3">
      <motion.h2 variants={pop} id="topics-title" className="px-1 text-sm font-medium text-ink-2">
        Your topics
      </motion.h2>
      <motion.ul variants={stage(0.09, 0)} className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <AnimatePresence>
          {topics.map((t) => (
            <motion.li key={t.id} layout variants={pop} whileHover={{ y: -2, transition: SPRING }}>
              <TopicCard topic={t} />
            </motion.li>
          ))}
          <motion.li key="new" layout variants={pop}>
            <a
              href="#/new-topic"
              className="flex h-full min-h-24 items-center justify-center gap-1.5 rounded-2xl border border-dashed border-hairline-strong text-sm text-ink-3 transition-colors hover:border-peach/50 hover:text-ink"
            >
              <Plus size={14} aria-hidden /> Start a topic
            </a>
          </motion.li>
        </AnimatePresence>
      </motion.ul>
    </motion.section>
  );
}

const MARK_TONE: Record<string, string> = { seen: "text-warn", recalled: "text-teal", retained: "text-teal" };
const MAX_MARKS = 20;

function topicStatus(t: TopicSummary): ReactNode {
  if (t.status === "draft" && t.origin === "kizn") return "Being prepared by kizn";
  if (t.status === "draft") return "Draft · finish setting it up";
  if (t.states.length === 0) return "Nothing to practise yet";
  const due = Number(t.due_now);
  const fresh = Number(t.new_now);
  if (!t.practised) return due + fresh > 0 ? <span className="text-peach">Ready to start</span> : "Writing questions…";
  if (due > 0) return <span className="text-peach">{plural(due, "recall")} due</span>;
  if (fresh > 0) return <span className="text-peach">New concepts ready</span>;
  const when = whenDue(t.next_due_in_days);
  return when ? `Next recall ${when}` : "Nothing due";
}

function TopicCard({ topic: t }: { topic: TopicSummary }) {
  // A draft of the learner's is theirs to finish; one kizn is preparing
  // opens on its page.
  const href =
    t.status === "draft" && t.origin !== "kizn"
      ? `#/setup/${encodeURIComponent(t.id)}`
      : `#/topic/${encodeURIComponent(t.id)}`;
  const shown = t.states.slice(0, MAX_MARKS);
  const pct = Number(t.expected) > 0 ? Math.min(100, (Number(t.active) / Number(t.expected)) * 100) : 0;
  return (
    <a
      href={href}
      className="flex h-full flex-col gap-3 rounded-2xl bg-subtle px-5 py-4 ring-1 ring-hairline transition-[box-shadow,background-color] hover:bg-raised/30 hover:ring-hairline-strong"
    >
      <span className="text-[15px] font-medium text-ink">{t.title}</span>
      {t.status !== "draft" && shown.length > 0 && (
        <motion.span
          variants={stage(0.02, 0.22)}
          className="flex flex-wrap items-center gap-1"
          role="img"
          aria-label={`${t.states.length} concepts`}
        >
          {shown.map((s, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: marks are positional
            <motion.span key={i} variants={bead} className={`inline-flex ${MARK_TONE[s] ?? "text-ink-4"}`}>
              <ConceptMark state={s} size={11} />
            </motion.span>
          ))}
          {t.states.length > MAX_MARKS && (
            <span className="pl-1 text-[11px] text-ink-4">+{t.states.length - MAX_MARKS}</span>
          )}
        </motion.span>
      )}
      <span className="mt-auto flex flex-col gap-2 text-xs text-ink-3">
        {topicStatus(t)}
        <AnimatePresence initial={false}>
          {t.status !== "draft" && t.preparing && (
            <motion.span
              key="bar"
              variants={pop}
              initial="hidden"
              animate="shown"
              exit="gone"
              role="progressbar"
              aria-label="Questions being written"
              aria-valuenow={Math.round(pct)}
              className="block h-0.5 overflow-hidden rounded-full bg-raised"
            >
              <motion.span
                className="block h-full rounded-full bg-ink-4"
                initial={{ width: 0 }}
                animate={{ width: `${pct}%` }}
                transition={{ duration: 0.8, ease: EASE, delay: 0.4 }}
              />
            </motion.span>
          )}
        </AnimatePresence>
      </span>
    </a>
  );
}
