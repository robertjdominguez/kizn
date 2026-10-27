import { ArrowRight, Flag, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { AnswerView } from "../bindings/AnswerView";
import type { CardView } from "../bindings/CardView";
import type { DeckView } from "../bindings/DeckView";
import type { DisputeView } from "../bindings/DisputeView";
import { ConceptMark, STATE_WORD } from "../components/ConceptMark";
import { Reveal } from "../components/Reveal";
import { Screen } from "../components/Screen";
import { BOUNCY, bead, deal, button as motionButton, nod, pop, presence, rise, SPRING, shake, stage } from "../motion";

// "Right." lands with a little bounce.
const verdictYes = {
  hidden: { opacity: 0, scale: 0.6 },
  shown: { opacity: 1, scale: 1, transition: BOUNCY },
  gone: { opacity: 0 },
};

import { call } from "../rpc";
import { Onboarding } from "./Onboarding";
import { Summary } from "./Summary";

const LETTERS = ["A", "B", "C", "D", "E"];

// One practice set: a card at a time, feedback in place on a tap, then the
// end of the set. Every answer is recorded as it's given, so Stop loses
// nothing. Built in small reviewed steps: the card and its feedback first.
export function Practice({ topicId }: { topicId?: string }) {
  const [deck, setDeck] = useState<DeckView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [at, setAt] = useState(0);
  // A topic's first set opens on two cards saying how it works.
  const [intro, setIntro] = useState<0 | 1 | null>(null);
  const [answers, setAnswers] = useState<AnswerView[]>([]);
  const back = topicId ? `#/topic/${encodeURIComponent(topicId)}` : "#/";

  useEffect(() => {
    call({ type: "start_practice", topic_id: topicId })
      .then((d) => {
        setDeck(d);
        if (d.first_set) setIntro(0);
      })
      .catch((e: Error) => setError(e.message));
  }, [topicId]);
  const introNext = useCallback(() => setIntro((i) => (i === 0 ? 1 : null)), []);
  const introSkip = useCallback(() => setIntro(null), []);

  const card = deck?.cards[at];
  const answered = answers.length > at ? answers[at] : undefined;
  const done = deck !== null && at >= deck.cards.length;
  const next = useCallback(() => setAt((n) => n + 1), []);
  const attemptIds = useMemo(() => answers.map((a) => a.attempt_id), [answers]);

  return (
    <Screen
      width="max-w-2xl"
      header={
        <motion.header variants={rise} {...presence} className="flex items-center gap-3">
          <span className="text-sm font-medium text-ink">Practice</span>
          {card?.topic_title && <span className="truncate text-sm text-ink-3">{card.topic_title}</span>}
          <span className="flex-1" />
          <AnimatePresence mode="wait" initial={false}>
            {deck && !done && intro === null && (
              <motion.span
                key={at}
                variants={rise}
                {...presence}
                className="text-xs tabular-nums text-ink-3"
                aria-live="polite"
              >
                {at + 1} of {deck.cards.length}
              </motion.span>
            )}
          </AnimatePresence>
          <a
            href={back}
            className="flex items-center gap-1 rounded-md px-2 py-1 text-sm text-ink-3 transition-colors hover:bg-raised hover:text-ink"
            title="Stop: what you've answered counts"
          >
            <X size={14} aria-hidden /> Stop
          </a>
        </motion.header>
      }
    >
      <AnimatePresence mode="wait">
        {error && (
          <motion.p key="error" variants={rise} {...presence} className="text-sm text-danger">
            Couldn't build a practice set: {error}
          </motion.p>
        )}
        {deck && deck.cards.length === 0 && (
          <motion.p key="empty" variants={rise} {...presence} className="text-sm text-ink-2">
            Nothing to practise right now.
          </motion.p>
        )}
        {deck && intro !== null && (
          <Onboarding
            key={`intro-${intro}`}
            step={intro}
            count={deck.cards.length}
            onNext={introNext}
            onSkip={introSkip}
          />
        )}
        {card && intro === null && (
          <Card
            key={card.item_id}
            card={card}
            answer={answered}
            onAnswer={(a) => setAnswers((all) => [...all, a])}
            onNext={next}
            last={at === (deck?.cards.length ?? 0) - 1}
          />
        )}
        {done && deck.cards.length > 0 && <Summary key="summary" attemptIds={attemptIds} />}
      </AnimatePresence>
    </Screen>
  );
}

function Card({
  card,
  answer,
  onAnswer,
  onNext,
  last,
}: {
  card: CardView;
  answer?: AnswerView;
  onAnswer: (a: AnswerView) => void;
  onNext: () => void;
  last: boolean;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // "This key is wrong": looking again first, then set aside.
  const [confirming, setConfirming] = useState(false);
  const [disputed, setDisputed] = useState<DisputeView | null>(null);

  const choose = useCallback(
    (i: number) => {
      if (answer || busy) return;
      setBusy(true);
      call({ type: "answer", item_id: card.item_id, order: card.order, chosen: i })
        .then(onAnswer)
        .catch((e: Error) => setError(e.message))
        .finally(() => setBusy(false));
    },
    [answer, busy, card, onAnswer],
  );

  // 1–5 or A–E answer; Enter moves on once answered.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (answer) {
        if (e.key === "Enter") {
          e.preventDefault();
          onNext();
        }
        return;
      }
      const n = /^[1-5]$/.test(e.key) ? Number(e.key) - 1 : LETTERS.indexOf(e.key.toUpperCase());
      if (n >= 0 && n < card.options.length) choose(n);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [answer, card.options.length, choose, onNext]);

  return (
    <motion.section
      variants={deal}
      initial="hidden"
      animate="shown"
      exit="gone"
      className="flex flex-col gap-4"
      aria-label="Question"
    >
      {/* Not the concept: it's written as a statement, so naming it before
          the answer would give the answer away. Feedback names it after. */}
      <motion.p variants={pop} className="text-[11px] font-semibold uppercase tracking-wide text-teal">
        {card.reason === "new" ? "Starting point" : "Recall"}
      </motion.p>
      <motion.p variants={pop} className="text-[15px] leading-relaxed text-ink">
        {card.stem}
      </motion.p>
      <motion.ol variants={stage(0.06, 0.04)} className="flex flex-col gap-2">
        {card.options.map((text, i) => {
          const fb = answer?.options[i];
          const isKey = !!answer && answer.key === i;
          const isPick = !!answer && answer.picked === i;
          const tone = !answer
            ? "border-hairline enabled:hover:border-teal/50 enabled:hover:bg-teal/5"
            : isKey
              ? "border-teal/60 bg-teal/10"
              : isPick
                ? "border-warn/50 bg-warn/10"
                : "border-hairline opacity-80";
          return (
            <motion.li key={text} variants={pop}>
              <motion.button
                // A small yes on the right answer, a gentle no on a missed pick.
                animate={isPick && !isKey ? shake : isKey ? nod : undefined}
                whileTap={answer ? undefined : { scale: 0.985 }}
                type="button"
                disabled={!!answer || busy}
                onClick={() => choose(i)}
                className={`flex w-full items-start gap-2.5 rounded-lg border px-3 py-2.5 text-left text-sm transition-colors disabled:cursor-default ${tone}`}
              >
                <span className="shrink-0 pt-0.5 font-mono text-xs text-ink-3">{LETTERS[i]}</span>
                <span className="min-w-0 flex-1">
                  <span className="text-ink">{text}</span>
                  <AnimatePresence>
                    {fb && (
                      <motion.span
                        key="why"
                        initial={{ opacity: 0, height: 0, filter: "blur(4px)" }}
                        animate={{
                          opacity: 1,
                          height: "auto",
                          filter: "blur(0px)",
                          transition: { ...SPRING, delay: 0.15 + i * 0.07 },
                        }}
                        exit={{ opacity: 0, height: 0 }}
                        className="block overflow-hidden"
                      >
                        <span className="block pt-1 text-xs text-ink-3">{fb.why}</span>
                      </motion.span>
                    )}
                  </AnimatePresence>
                </span>
                <AnimatePresence>
                  {isKey && (
                    <motion.span
                      key="key"
                      variants={bead}
                      initial="hidden"
                      animate="shown"
                      exit="gone"
                      className="shrink-0 text-xs text-teal"
                    >
                      right
                    </motion.span>
                  )}
                  {isPick && !isKey && (
                    <motion.span
                      key="pick"
                      variants={bead}
                      initial="hidden"
                      animate="shown"
                      exit="gone"
                      className="shrink-0 text-xs text-warn"
                    >
                      your pick
                    </motion.span>
                  )}
                </AnimatePresence>
              </motion.button>
            </motion.li>
          );
        })}
      </motion.ol>
      <Reveal when={error}>
        <p className="text-xs text-danger">{error}</p>
      </Reveal>
      <AnimatePresence>
        {answer && (
          <motion.div
            key="feedback"
            variants={stage(0.07, 0.1 + card.options.length * 0.07)}
            initial="hidden"
            animate="shown"
            exit="gone"
            className="flex flex-col gap-2 text-sm"
          >
            <AnimatePresence mode="wait" initial={false}>
              {disputed ? (
                <SetAside key="aside" disputed={disputed} />
              ) : (
                <motion.div key="feedback" variants={stage(0.07, 0)} className="flex flex-col gap-2">
                  <Feedback answer={answer} />
                </motion.div>
              )}
            </AnimatePresence>
            <AnimatePresence>
              {confirming && !disputed && (
                <LookAgain
                  key="look"
                  answer={answer}
                  onKeep={() => setConfirming(false)}
                  onDispute={() =>
                    call({ type: "dispute", attempt_id: answer.attempt_id })
                      .then((d) => {
                        setDisputed(d);
                        setConfirming(false);
                      })
                      .catch((e: Error) => setError(e.message))
                  }
                />
              )}
            </AnimatePresence>
            <motion.div variants={motionButton} className="flex items-center gap-3 pt-1">
              <button
                type="button"
                onClick={onNext}
                className="flex items-center gap-1.5 rounded-md border border-accent/40 bg-accent/15 px-3 py-1.5 text-sm font-medium text-accent outline-none transition-colors hover:bg-accent/25 focus-visible:ring-2 focus-visible:ring-accent/50"
              >
                {last ? "Finish" : "Next"} <ArrowRight size={14} aria-hidden />
              </button>
              <span className="text-xs text-ink-4">or press Enter</span>
              <span className="flex-1" />
              {!disputed && !confirming && (
                <button
                  type="button"
                  onClick={() => setConfirming(true)}
                  className="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-ink-4 transition-colors hover:bg-raised hover:text-ink"
                >
                  <Flag size={12} aria-hidden /> This key is wrong
                </button>
              )}
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.section>
  );
}

// A moment to look again before disputing: kizn's key and its reason. Right
// after a miss it's natural to blame the question; this catches that
// without stopping a real dispute.
function LookAgain({ answer, onKeep, onDispute }: { answer: AnswerView; onKeep: () => void; onDispute: () => void }) {
  const key = answer.options[answer.key];
  return (
    <motion.div
      variants={stage(0.06, 0)}
      initial="hidden"
      animate="shown"
      exit="gone"
      role="group"
      aria-label="Look again"
      className="flex flex-col gap-2 rounded-xl bg-warn/[0.06] px-4 py-3 ring-1 ring-warn/25"
    >
      <motion.p variants={pop} className="text-xs text-ink-3">
        kizn's key is <span className="font-medium text-ink">{LETTERS[answer.key]}</span>: {key?.why}
      </motion.p>
      <motion.div variants={pop} className="flex flex-wrap items-center gap-2">
        <span className="text-sm text-ink-2">Still think the key is wrong?</span>
        <span className="flex-1" />
        <button
          type="button"
          onClick={onKeep}
          className="rounded-md px-2.5 py-1 text-xs text-ink-3 transition-colors hover:bg-raised hover:text-ink"
        >
          Keep it
        </button>
        <button
          type="button"
          onClick={onDispute}
          className="rounded-md bg-warn/15 px-2.5 py-1 text-xs font-medium text-warn transition-colors hover:bg-warn/25"
        >
          Yes, set it aside
        </button>
      </motion.div>
    </motion.div>
  );
}

// After a dispute: the answer doesn't count, the question is gone, and a
// replacement is on its way.
function SetAside({ disputed }: { disputed: DisputeView }) {
  return (
    <motion.div variants={stage(0.07, 0)} initial="hidden" animate="shown" exit="gone" className="flex flex-col gap-2">
      <motion.p variants={pop} className="text-ink-2">
        Set aside. This answer doesn't count, and the question won't come back; kizn will write another.
      </motion.p>
      {disputed.concepts.map((c) => (
        <motion.p key={c.concept} variants={pop} className="flex items-center gap-1.5 text-xs text-ink-3">
          <span className="text-ink-4">
            <ConceptMark state={c.after} size={12} />
          </span>
          {c.concept}: {STATE_WORD[c.after] ?? c.after}
        </motion.p>
      ))}
    </motion.div>
  );
}

function Feedback({ answer }: { answer: AnswerView }) {
  const pick = answer.options[answer.picked];
  return (
    <>
      <motion.p
        variants={answer.correct ? verdictYes : pop}
        className={`origin-left ${answer.correct ? "text-teal" : "text-warn"}`}
      >
        {answer.correct ? "Right." : "Not this time."}
      </motion.p>
      {!answer.correct && pick?.misconception && (
        <motion.p variants={pop} className="text-xs text-ink-2">
          Your pick is a common mix-up: {pick.misconception.toLowerCase()}.
        </motion.p>
      )}
      {answer.concepts.length > 0 && (
        <motion.p variants={pop} className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-ink-3">
          {answer.concepts.map((c) => (
            <span key={c.concept} className="inline-flex items-center gap-1.5">
              <span className={c.after === "seen" ? "text-warn" : "text-teal"}>
                <ConceptMark state={c.after} size={12} />
              </span>
              {c.concept}: {STATE_WORD[c.after] ?? c.after}
            </span>
          ))}
        </motion.p>
      )}
      <motion.p variants={pop} className="text-xs text-ink-4">
        {answer.source ? `Source: ${answer.source}` : "From the model's own knowledge, not a cited source"}
      </motion.p>
    </>
  );
}
