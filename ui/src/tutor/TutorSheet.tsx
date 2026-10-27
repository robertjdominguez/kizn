import { ArrowUp, ChevronRight, LoaderCircle, MessageCircle, Plus, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import type { Anchor } from "../bindings/Anchor";
import type { ThreadSummary } from "../bindings/ThreadSummary";
import type { ThreadView } from "../bindings/ThreadView";
import { subscribe } from "../events";
import { EASE, fade, GLIDE, pop, presence, stage } from "../motion";
import { call } from "../rpc";
import { Markdown } from "./Markdown";

/** A starter: sent as is, or (prefill) put in the box for the learner to finish. */
export type Starter = { label: string; prefill?: boolean };

// The anchored tutor: opened from a thing (here, a card), with that thing on
// top, starter questions, the reply streaming in, and earlier conversations
// about the same thing to reopen. The tutor talks; it can't move anything.
export function TutorSheet({
  anchor,
  context,
  starters,
  passage,
  onClose,
}: {
  anchor: Anchor | null;
  /** What the conversation is about, shown on top. */
  context: ReactNode;
  starters: Starter[];
  /** A passage (or a video's moment) the first question is about. */
  passage?: string | null;
  onClose: () => void;
}) {
  return (
    <AnimatePresence>
      {anchor && (
        <motion.div
          key="scrim"
          variants={fade}
          {...presence}
          className="fixed inset-0 z-[60] flex justify-end bg-crust/50"
          onClick={onClose}
        >
          <motion.aside
            role="dialog"
            aria-modal="true"
            aria-label="Ask about this"
            initial={{ x: 64, opacity: 0 }}
            animate={{ x: 0, opacity: 1, transition: GLIDE }}
            exit={{ x: 48, opacity: 0, transition: { duration: 0.16, ease: EASE } }}
            className="flex h-full w-full max-w-lg flex-col border-l border-hairline-strong bg-subtle shadow-2xl shadow-black/40"
            onClick={(e) => e.stopPropagation()}
          >
            <Conversation anchor={anchor} context={context} starters={starters} passage={passage} onClose={onClose} />
          </motion.aside>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function Conversation({
  anchor,
  context,
  starters,
  passage,
  onClose,
}: {
  anchor: Anchor;
  context: ReactNode;
  starters: Starter[];
  passage?: string | null;
  onClose: () => void;
}) {
  const [threads, setThreads] = useState<ThreadSummary[]>([]);
  const [thread, setThread] = useState<ThreadView | null>(null);
  const [pending, setPending] = useState("");
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const box = useRef<HTMLTextAreaElement>(null);
  const end = useRef<HTMLDivElement>(null);
  const threadId = thread?.id;

  const loadThreads = useCallback(() => {
    call({ type: "get_threads", anchor })
      .then(setThreads)
      .catch((e: Error) => setError(e.message));
  }, [anchor]);
  useEffect(loadThreads, [loadThreads]);

  // The reply, word by word, kept per conversation from the moment the sheet
  // opens (the first words can arrive before "ask" has answered); when it's
  // done, the kept version replaces it.
  const words = useRef(new Map<string, string>());
  const current = useRef<string | undefined>(undefined);
  current.current = threadId;
  useEffect(() => {
    return subscribe((e) => {
      if (e.type === "tutor_delta") {
        const text = (words.current.get(e.thread_id) ?? "") + e.text;
        words.current.set(e.thread_id, text);
        if (e.thread_id === current.current) setPending(text);
      }
      if (e.type === "tutor_done") {
        words.current.delete(e.thread_id);
        if (e.thread_id !== current.current) return;
        call({ type: "get_thread", thread_id: e.thread_id })
          .then((t) => {
            // Done means done, whatever a read in the same instant says.
            setThread({ ...t, streaming: false });
            setPending("");
            if (e.error) setError(`The tutor couldn't answer: ${e.error}`);
          })
          .catch((err: Error) => setError(err.message));
        loadThreads();
      }
    });
  }, [loadThreads]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: scroll as the reply grows
  useEffect(() => {
    end.current?.scrollIntoView({ block: "end", behavior: "smooth" });
  }, [pending, thread?.messages.length]);

  const send = (text: string) => {
    const t = text.trim();
    if (!t || thread?.streaming) return;
    setError(null);
    setDraft("");
    setPending("");
    // The passage goes with a new conversation's first question.
    call({ type: "ask", anchor, thread_id: thread?.id, text: t, passage: thread ? undefined : (passage ?? undefined) })
      .then((th) => {
        setThread(th);
        current.current = th.id;
        setPending(words.current.get(th.id) ?? "");
      })
      .catch((e: Error) => setError(e.message));
  };

  const startNew = () => {
    setThread(null);
    setPending("");
    setDraft("");
    box.current?.focus();
  };

  const pickStarter = (s: Starter) => {
    if (s.prefill) {
      setDraft(`${s.label} `);
      requestAnimationFrame(() => {
        box.current?.focus();
        box.current?.setSelectionRange(box.current.value.length, box.current.value.length);
      });
    } else send(s.label);
  };

  const streaming = !!thread?.streaming;
  const earlier = threads.filter((t) => t.id !== thread?.id);
  return (
    <>
      <div className="flex items-center gap-2 border-b border-hairline px-5 py-3">
        <MessageCircle size={14} className="text-accent" aria-hidden />
        <span className="text-[11px] font-semibold uppercase tracking-wide text-accent">Ask about this</span>
        <span className="flex-1" />
        {thread && (
          <button
            type="button"
            onClick={startNew}
            className="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-ink-3 transition-colors hover:bg-raised hover:text-ink"
          >
            <Plus size={12} aria-hidden /> New conversation
          </button>
        )}
        <button
          type="button"
          aria-label="Close"
          onClick={onClose}
          className="rounded p-1 text-ink-3 transition-colors hover:bg-raised hover:text-ink"
        >
          <X size={14} aria-hidden />
        </button>
      </div>
      <motion.div
        variants={stage(0.08, 0.12)}
        initial="hidden"
        animate="shown"
        className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-5 py-5"
      >
        <motion.div
          variants={pop}
          className="rounded-xl bg-canvas/60 px-4 py-3 text-xs text-ink-3 ring-1 ring-hairline"
        >
          {context}
          {passage && !thread && (
            <blockquote className="mt-2 border-l-2 border-accent/50 pl-3 text-sm text-ink-2">{passage}</blockquote>
          )}
        </motion.div>
        {thread ? (
          <div className="flex flex-col gap-4" aria-live="polite">
            {thread.messages.map((m, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: a conversation only grows
              <Message key={i} who={m.role} text={m.text} />
            ))}
            {streaming && <Message who="tutor" text={pending} typing={!pending} />}
          </div>
        ) : (
          <motion.div variants={stage(0.05, 0)} className="flex flex-col gap-2">
            <motion.p variants={pop} className="text-xs text-ink-4">
              Start with
            </motion.p>
            <div className="flex flex-wrap gap-2">
              {starters.map((s) => (
                <motion.button
                  key={s.label}
                  variants={pop}
                  type="button"
                  onClick={() => pickStarter(s)}
                  className="rounded-full bg-raised/60 px-3 py-1.5 text-left text-xs text-ink-2 ring-1 ring-hairline transition-colors hover:bg-raised hover:text-ink"
                >
                  {s.prefill ? `${s.label}…` : s.label}
                </motion.button>
              ))}
            </div>
          </motion.div>
        )}
        <AnimatePresence>
          {error && (
            <motion.p
              key="error"
              variants={pop}
              initial="hidden"
              animate="shown"
              exit="gone"
              className="text-xs text-danger"
            >
              {error}
            </motion.p>
          )}
        </AnimatePresence>
        {!thread && earlier.length > 0 && (
          <motion.section variants={pop} aria-label="Earlier conversations" className="mt-2 flex flex-col gap-1">
            <p className="text-[11px] font-semibold uppercase tracking-wide text-ink-4">
              Earlier · {earlier.length} {earlier.length === 1 ? "conversation" : "conversations"}
            </p>
            {earlier.map((t) => (
              <button
                key={t.id}
                type="button"
                onClick={() =>
                  call({ type: "get_thread", thread_id: t.id })
                    .then(setThread)
                    .catch((e: Error) => setError(e.message))
                }
                className="group flex items-center gap-2 rounded-lg px-2 py-1.5 text-left text-sm text-ink-2 transition-colors hover:bg-raised/50 hover:text-ink"
              >
                <span className="flex-1 truncate">{t.title}</span>
                <ChevronRight size={13} className="shrink-0 text-ink-4 group-hover:text-ink" aria-hidden />
              </button>
            ))}
          </motion.section>
        )}
        <div ref={end} />
      </motion.div>
      <form
        className="flex items-end gap-2 border-t border-hairline px-4 py-3"
        onSubmit={(e) => {
          e.preventDefault();
          send(draft);
        }}
      >
        <textarea
          ref={box}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send(draft);
            }
          }}
          rows={1}
          placeholder={thread ? "Ask a follow-up…" : "Ask about this…"}
          aria-label="Your question"
          className="max-h-40 min-h-9 flex-1 resize-none rounded-lg border border-hairline bg-canvas px-3 py-2 text-sm text-ink outline-none placeholder:text-ink-4 focus:border-accent/50"
        />
        <button
          type="submit"
          aria-label="Ask"
          disabled={!draft.trim() || streaming}
          className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-accent text-canvas transition-opacity disabled:opacity-40"
        >
          <ArrowUp size={16} aria-hidden />
        </button>
      </form>
    </>
  );
}

function Message({ who, text, typing = false }: { who: string; text: string; typing?: boolean }) {
  if (who === "learner") {
    // A question about a passage is kept as "> passage" then the question.
    const quoted = text.startsWith("> ") ? text.slice(2).split("\n\n") : null;
    return (
      <motion.div
        initial={{ opacity: 0, y: 6 }}
        animate={{ opacity: 1, y: 0, transition: { duration: 0.2, ease: EASE } }}
        className="ml-10 flex flex-col gap-1.5 self-end rounded-2xl rounded-br-md bg-accent/12 px-3.5 py-2 text-sm text-ink"
      >
        {quoted ? (
          <>
            <blockquote className="border-l-2 border-accent/50 pl-2 text-xs text-ink-3">{quoted[0]}</blockquote>
            <span>{quoted.slice(1).join("\n\n")}</span>
          </>
        ) : (
          text
        )}
      </motion.div>
    );
  }
  return (
    <motion.div
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0, transition: { duration: 0.2, ease: EASE } }}
      className="flex flex-col gap-2 text-sm leading-relaxed text-ink-2"
      aria-label="The tutor's reply"
    >
      {typing ? (
        <span className="flex items-center gap-2 text-xs text-ink-4">
          <LoaderCircle size={13} className="animate-spin text-accent" aria-hidden /> Thinking…
        </span>
      ) : (
        <Markdown text={text} />
      )}
    </motion.div>
  );
}
