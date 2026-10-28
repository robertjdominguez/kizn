import { ArrowLeft, Check, ExternalLink, FileText, MessageCircle, Play, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { Fragment, useEffect, useRef, useState } from "react";
import type { LectureView } from "../bindings/LectureView";
import { openExternal } from "../external";
import { EASE, GLIDE, pop, stage } from "../motion";
import { call } from "../rpc";
import { TutorSheet } from "../tutor/TutorSheet";
import { conversations, useConversations } from "../tutor/useConversations";

// A concept's lecture, read inside kizn: key terms, the explanation, where
// people go wrong, and the sources every part is checked against (they open
// in the learner's browser). Text selected in it can be asked about.
export function LectureReader({
  conceptId,
  concept,
  lecture,
  note,
  onClose,
}: {
  conceptId: string;
  concept: string;
  lecture: LectureView | null;
  /** The learner's note on the concept, kept beside the lecture. */
  note: string | null;
  onClose: () => void;
}) {
  return (
    <AnimatePresence>
      {lecture && (
        <motion.div
          key="lecture"
          role="dialog"
          aria-modal="true"
          aria-label="Lecture"
          initial={{ opacity: 0, y: 40 }}
          animate={{ opacity: 1, y: 0, transition: GLIDE }}
          exit={{ opacity: 0, y: 24, transition: { duration: 0.16, ease: EASE } }}
          className="fixed inset-0 z-50 flex flex-col bg-canvas"
        >
          <Read conceptId={conceptId} concept={concept} lecture={lecture} note={note} onClose={onClose} />
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function Read({
  conceptId,
  concept,
  lecture: l,
  note,
  onClose,
}: {
  conceptId: string;
  concept: string;
  lecture: LectureView;
  note: string | null;
  onClose: () => void;
}) {
  const [asking, setAsking] = useState<{ passage: string | null } | null>(null);
  const askingNow = useRef(false);
  askingNow.current = asking !== null;
  const [talks, recount] = useConversations({ kind: "concept", concept_id: conceptId });
  const [picked, setPicked] = useState<{ text: string; x: number; y: number } | null>(null);
  // The footnote just jumped to, briefly lit.
  const [lit, setLit] = useState<number | null>(null);
  const article = useRef<HTMLElement>(null);

  // Follows the selection (mouse, keyboard or touch) a moment after it
  // settles; only text selected in the lecture counts.
  useEffect(() => {
    let t: ReturnType<typeof setTimeout> | undefined;
    const pick = () => {
      const sel = window.getSelection();
      const text = sel?.toString().trim() ?? "";
      const inside = !!sel?.anchorNode && !!article.current?.contains(sel.anchorNode);
      if (!sel || !inside || text.split(/\s+/).length < 3 || sel.rangeCount === 0) return setPicked(null);
      const rect = sel.getRangeAt(0).getBoundingClientRect();
      setPicked({ text: text.slice(0, 600), x: rect.left + rect.width / 2, y: Math.max(rect.top, 56) });
    };
    const onChange = () => {
      clearTimeout(t);
      t = setTimeout(pick, 150);
    };
    document.addEventListener("selectionchange", onChange);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !askingNow.current && !(e.target instanceof HTMLTextAreaElement)) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      clearTimeout(t);
      document.removeEventListener("selectionchange", onChange);
      window.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  const jump = (n: number) => {
    document.getElementById(`note-${n}`)?.scrollIntoView({ behavior: "smooth", block: "center" });
    setLit(n);
    setTimeout(() => setLit(null), 1600);
  };
  const notes = (ns: number[]) => (
    <>
      {ns.map((n) => (
        <button
          key={n}
          type="button"
          onClick={() => jump(n)}
          aria-label={`Source ${n}`}
          className="ml-0.5 align-super text-[10px] font-semibold text-accent hover:underline"
        >
          {n}
        </button>
      ))}
    </>
  );

  return (
    <>
      <header className="flex h-11 shrink-0 items-center gap-3 border-b border-hairline px-4">
        <button
          type="button"
          onClick={onClose}
          className="flex items-center gap-1 rounded-md px-2 py-1 text-sm text-ink-3 transition-colors hover:bg-raised hover:text-ink"
        >
          <ArrowLeft size={14} aria-hidden /> Back
        </button>
        <span className="flex-1" />
        <button
          type="button"
          onClick={() => setAsking({ passage: null })}
          className="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs text-ink-3 transition-colors hover:bg-raised hover:text-ink"
        >
          <MessageCircle size={12} aria-hidden /> Ask about this
          <span className="text-ink-4">{conversations(talks)}</span>
        </button>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">
        <motion.div
          variants={stage(0.1, 0.1)}
          initial="hidden"
          animate="shown"
          className="mx-auto grid w-full max-w-5xl grid-cols-1 gap-10 px-6 py-10 lg:grid-cols-[minmax(0,1fr)_16rem]"
        >
          <article ref={article} className="flex min-w-0 flex-col gap-9">
            <motion.div variants={pop} className="flex flex-col gap-2 border-b border-hairline pb-6">
              <p className="text-[11px] font-semibold uppercase tracking-wider text-accent">Reading for</p>
              <h1 className="text-xl font-semibold leading-snug text-ink">{concept}</h1>
              <p className="text-xs text-ink-4">
                From {l.footnotes.length} sources kizn read · every part checked against what it cites
              </p>
            </motion.div>

            {l.key_terms.length > 0 && (
              <motion.section variants={pop} aria-labelledby="terms" className="flex flex-col gap-3">
                <h2 id="terms" className="text-[11px] font-semibold uppercase tracking-wide text-ink-3">
                  Key terms
                </h2>
                <motion.dl variants={stage(0.06, 0)} className="grid gap-2 sm:grid-cols-2">
                  {l.key_terms.map((t) => (
                    <motion.div
                      key={t.term}
                      variants={pop}
                      className="flex flex-col gap-0.5 rounded-lg bg-subtle px-3.5 py-2.5 ring-1 ring-hairline"
                    >
                      <dt className="text-sm font-semibold text-ink">
                        {t.term}
                        {notes(t.notes)}
                      </dt>
                      <dd className="text-sm text-ink-3">{t.meaning}</dd>
                    </motion.div>
                  ))}
                </motion.dl>
              </motion.section>
            )}

            {l.sections.map((s) => (
              <motion.section key={s.heading} variants={pop} className="flex flex-col gap-3">
                <h2 className="text-base font-semibold text-ink">{s.heading}</h2>
                {s.paragraphs.map((p) => (
                  <p key={p.text} className="text-[15px] leading-relaxed text-ink-2">
                    {p.text}
                    {notes(p.notes)}
                  </p>
                ))}
              </motion.section>
            ))}

            {l.misconceptions.length > 0 && (
              <motion.section variants={pop} aria-labelledby="mixups" className="flex flex-col gap-3">
                <h2 id="mixups" className="text-base font-semibold text-ink">
                  Where people go wrong
                </h2>
                <ul className="flex flex-col gap-3">
                  {l.misconceptions.map((m) => (
                    <li
                      key={m.belief}
                      className="flex flex-col gap-1.5 rounded-lg bg-subtle px-4 py-3 ring-1 ring-hairline"
                    >
                      <p className="flex items-start gap-2 text-sm text-ink-3">
                        <X size={14} className="mt-0.5 shrink-0 text-warn" aria-label="Mistaken" />
                        <span>{m.belief}</span>
                      </p>
                      <p className="flex items-start gap-2 text-sm text-ink-2">
                        <Check size={14} className="mt-0.5 shrink-0 text-teal" aria-label="Instead" />
                        <span>
                          {m.correction}
                          {notes(m.notes)}
                        </span>
                      </p>
                    </li>
                  ))}
                </ul>
              </motion.section>
            )}

            <motion.section
              variants={pop}
              aria-labelledby="sources"
              className="flex flex-col gap-2 border-t border-hairline pt-6"
            >
              <h2 id="sources" className="text-[11px] font-semibold uppercase tracking-wide text-ink-3">
                Sources
              </h2>
              <ol className="flex flex-col gap-1">
                {l.footnotes.map((f) => (
                  <Fragment key={f.n}>
                    <li id={`note-${f.n}`}>
                      <button
                        type="button"
                        onClick={() => openExternal(f.source.url)}
                        className={`group flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition-colors hover:bg-raised/50 ${lit === f.n ? "bg-accent/10" : ""}`}
                      >
                        <span className="w-4 shrink-0 text-xs font-semibold tabular-nums text-accent">{f.n}</span>
                        {f.source.kind === "video" ? (
                          <Play size={13} className="shrink-0 text-ink-4" aria-label="Video" />
                        ) : (
                          <FileText size={13} className="shrink-0 text-ink-4" aria-label="Page" />
                        )}
                        <span className="min-w-0 flex-1 truncate text-sm text-ink">{f.source.title}</span>
                        <span className="shrink-0 text-xs text-ink-4">{f.source.host}</span>
                        <ExternalLink size={12} className="shrink-0 text-ink-4 group-hover:text-accent" aria-hidden />
                      </button>
                    </li>
                  </Fragment>
                ))}
              </ol>
              <p className="text-xs text-ink-4">They open in your browser, to read further.</p>
            </motion.section>
          </article>
          <motion.aside
            variants={pop}
            aria-label="Notes"
            className="flex flex-col gap-3 lg:sticky lg:top-8 lg:self-start"
          >
            <Notes conceptId={conceptId} initial={note ?? ""} />
          </motion.aside>
        </motion.div>
      </div>
      <AnimatePresence>
        {picked && !asking && (
          <motion.button
            key="ask-picked"
            type="button"
            initial={{ opacity: 0, y: 6, scale: 0.9 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, scale: 0.9 }}
            style={{ left: picked.x, top: picked.y }}
            aria-label="Ask about the selection"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => {
              setAsking({ passage: picked.text });
              setPicked(null);
            }}
            className="fixed z-[55] flex -translate-x-1/2 -translate-y-[calc(100%+8px)] items-center gap-1.5 rounded-full bg-accent px-3 py-1.5 text-xs font-semibold text-canvas shadow-lg shadow-black/30"
          >
            <MessageCircle size={12} aria-hidden /> Ask about this
          </motion.button>
        )}
      </AnimatePresence>
      <TutorSheet
        anchor={asking ? { kind: "concept", concept_id: conceptId } : null}
        passage={asking?.passage}
        context={<p className="text-sm text-ink">{concept}</p>}
        starters={
          asking?.passage
            ? [{ label: "Explain this part" }, { label: "Give me an example" }, { label: "Why does this matter?" }]
            : [{ label: "Explain it simply" }, { label: "Give me an example" }, { label: "Quiz me on it" }]
        }
        onClose={() => {
          setAsking(null);
          recount();
        }}
      />
    </>
  );
}

// The concept's note, saved as you type (a moment after you stop).
function Notes({ conceptId, initial }: { conceptId: string; initial: string }) {
  const [text, setText] = useState(initial);
  const [state, setState] = useState<"idle" | "saving" | "saved" | string>("idle");
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const change = (v: string) => {
    setText(v);
    setState("saving");
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      call({ type: "save_note", concept_id: conceptId, text: v })
        .then(() => setState("saved"))
        .catch((e: Error) => setState(e.message));
    }, 600);
  };
  return (
    <>
      <label htmlFor="note" className="flex items-center text-sm font-medium text-ink-2">
        <span className="flex-1">Your note</span>
        <span className="text-xs font-normal text-ink-4">
          {state === "saving" ? "Saving…" : state === "saved" ? "Saved" : state === "idle" ? "" : state}
        </span>
      </label>
      <textarea
        id="note"
        value={text}
        onChange={(e) => change(e.target.value)}
        rows={10}
        placeholder="In your own words…"
        className="w-full resize-y rounded-xl border border-hairline bg-subtle px-3 py-2.5 text-sm leading-relaxed text-ink outline-none placeholder:text-ink-4 focus:border-accent/50"
      />
      <p className="text-xs text-ink-4">One note per concept, kept with it.</p>
    </>
  );
}
