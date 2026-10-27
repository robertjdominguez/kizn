import { Check, ExternalLink, FileText, LoaderCircle, Play, Plus, RotateCw, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { ConceptPageView } from "../bindings/ConceptPageView";
import { ConceptMark } from "../components/ConceptMark";
import { Reveal } from "../components/Reveal";
import { subscribe } from "../events";
import { openExternal } from "../external";
import { bead, EASE, fade, GLIDE, pop, presence, stage } from "../motion";
import { whenDue } from "../practice/Summary";
import { call } from "../rpc";
import { Reader } from "./Reader";

// A concept to learn, sliding in over whatever opened it (the summary): the
// concept, the pages kizn checked and could read, and "Mark studied".
// Studying is a suggestion: the concept comes back as recall either way.
export function ConceptSheet({ conceptId, onClose }: { conceptId: string | null; onClose: () => void }) {
  return (
    <AnimatePresence>
      {conceptId && (
        <motion.div
          key="scrim"
          variants={fade}
          {...presence}
          className="fixed inset-0 z-40 flex justify-end bg-crust/50"
          onClick={onClose}
        >
          <motion.aside
            role="dialog"
            aria-modal="true"
            aria-labelledby="concept-title"
            initial={{ x: 64, opacity: 0 }}
            animate={{ x: 0, opacity: 1, transition: GLIDE }}
            exit={{ x: 48, opacity: 0, transition: { duration: 0.16, ease: EASE } }}
            className="flex h-full w-full max-w-md flex-col border-l border-hairline-strong bg-subtle shadow-2xl shadow-black/40"
            onClick={(e) => e.stopPropagation()}
          >
            <Sheet conceptId={conceptId} onClose={onClose} />
          </motion.aside>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function Sheet({ conceptId, onClose }: { conceptId: string; onClose: () => void }) {
  const [c, setC] = useState<ConceptPageView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // A resource open in the reader, over the sheet.
  const [reading, setReading] = useState<string | null>(null);
  const readingNow = useRef<string | null>(null);
  readingNow.current = reading;

  const load = useCallback(() => {
    call({ type: "get_concept", concept_id: conceptId })
      .then(setC)
      .catch((e: Error) => setError(e.message));
  }, [conceptId]);
  const closeReader = useCallback(() => {
    setReading(null);
    load();
  }, [load]);

  useEffect(() => {
    load();
    // Escape closes the reader first, when it's open.
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !readingNow.current && onClose();
    window.addEventListener("keydown", onKey);
    const off = subscribe((e) => {
      if (e.type === "topic_changed") load();
    });
    return () => {
      window.removeEventListener("keydown", onKey);
      off();
    };
  }, [load, onClose]);

  const act = (type: "mark_studied" | "find_sources") => {
    setBusy(true);
    call({ type, concept_id: conceptId })
      .then(setC)
      .catch((e: Error) => setError(e.message))
      .finally(() => setBusy(false));
  };

  const when = c ? whenDue(c.next_due_in_days) : null;
  return (
    <>
      <div className="flex items-center gap-2 border-b border-hairline px-5 py-3">
        <span className="text-[11px] font-semibold uppercase tracking-wide text-warn">Learn this</span>
        <span className="flex-1" />
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
        variants={stage(0.1, 0.14)}
        initial="hidden"
        animate={c ? "shown" : "hidden"}
        className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto px-5 py-5"
      >
        <Reveal when={error} initial>
          <p className="text-sm text-danger">{error}</p>
        </Reveal>
        {c && (
          <>
            <motion.div variants={pop} className="flex items-start gap-2.5">
              <motion.span variants={bead} className="mt-1 inline-flex text-warn">
                <ConceptMark state={c.state} />
              </motion.span>
              <h2 id="concept-title" className="text-base font-semibold leading-snug text-ink">
                {c.statement}
              </h2>
            </motion.div>
            <motion.section variants={pop} aria-labelledby="sources-title" className="flex flex-col gap-2">
              <h3 id="sources-title" className="text-[11px] font-semibold uppercase tracking-wide text-ink-3">
                Learn from
              </h3>
              <AnimatePresence mode="wait" initial={false}>
                {c.finding === "finding" || c.finding === "not_started" ? (
                  <motion.p
                    key="finding"
                    variants={fade}
                    {...presence}
                    className="flex items-center gap-2 text-sm text-ink-3"
                  >
                    <LoaderCircle size={14} className="animate-spin text-accent" aria-hidden />
                    Finding pages kizn can check…
                  </motion.p>
                ) : c.sources.length > 0 ? (
                  <motion.ul
                    key="sources"
                    variants={stage(0.08, 0.1)}
                    initial="hidden"
                    animate="shown"
                    exit="gone"
                    className="flex flex-col divide-y divide-hairline overflow-hidden rounded-lg border border-hairline bg-canvas"
                  >
                    {c.sources.map((s) => (
                      <motion.li key={s.url} variants={pop} className="group flex items-center">
                        <button
                          type="button"
                          onClick={() => setReading(s.id)}
                          className="flex min-w-0 flex-1 items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-raised/40"
                        >
                          {s.kind === "video" ? (
                            <Play size={13} className="shrink-0 text-ink-4" aria-label="Video" />
                          ) : (
                            <FileText size={13} className="shrink-0 text-ink-4" aria-label="Page" />
                          )}
                          <span className="min-w-0 flex-1 truncate text-sm text-ink">{s.title}</span>
                          <span className="shrink-0 text-xs text-ink-4">{s.host}</span>
                        </button>
                        <button
                          type="button"
                          onClick={() => openExternal(s.url)}
                          aria-label={s.kind === "video" ? "Watch on YouTube" : "Open in browser"}
                          title={s.kind === "video" ? "Watch on YouTube" : "Open in browser"}
                          className="px-3 py-2.5 text-ink-4 transition-colors hover:text-accent"
                        >
                          <ExternalLink size={13} aria-hidden />
                        </button>
                      </motion.li>
                    ))}
                  </motion.ul>
                ) : (
                  <motion.div key="none" variants={fade} {...presence} className="flex flex-col gap-2">
                    <p className="text-sm text-ink-3">
                      kizn couldn't find anything it could check for this one
                      {c.checked > 0 && ` (it tried ${c.checked})`}.
                    </p>
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => act("find_sources")}
                      className="flex w-fit items-center gap-1.5 rounded-md px-2 py-1 text-xs text-ink-3 transition-colors hover:bg-raised hover:text-ink disabled:opacity-50"
                    >
                      <RotateCw size={12} aria-hidden /> Try again
                    </button>
                  </motion.div>
                )}
              </AnimatePresence>
              <p className="text-xs text-ink-4">kizn opened each of these and checked it's about this concept.</p>
              <AddOwn conceptId={c.concept_id} onAdded={setC} />
            </motion.section>
            {c.note && (
              <motion.section variants={pop} aria-label="Your note" className="flex flex-col gap-1.5">
                <h3 className="text-[11px] font-semibold uppercase tracking-wide text-ink-3">Your note</h3>
                <p className="whitespace-pre-wrap rounded-lg bg-canvas/60 px-3 py-2 text-sm text-ink-2 ring-1 ring-hairline">
                  {c.note}
                </p>
              </motion.section>
            )}
          </>
        )}
      </motion.div>
      {c && (
        <motion.div
          variants={pop}
          initial="hidden"
          animate="shown"
          transition={{ delay: 0.35 }}
          className="flex items-center gap-3 border-t border-hairline px-5 py-3"
        >
          <AnimatePresence mode="wait" initial={false}>
            {c.studied ? (
              <motion.span
                key="studied"
                variants={fade}
                {...presence}
                className="flex items-center gap-1.5 text-sm text-success"
              >
                <Check size={14} aria-hidden /> Studied
              </motion.span>
            ) : (
              <motion.button
                key="mark"
                variants={fade}
                {...presence}
                type="button"
                disabled={busy}
                onClick={() => act("mark_studied")}
                className="rounded-md border border-accent/40 bg-accent/15 px-3 py-1.5 text-sm font-medium text-accent transition-colors hover:bg-accent/25 disabled:opacity-50"
              >
                Mark studied
              </motion.button>
            )}
          </AnimatePresence>
          {when && <span className="text-xs text-ink-3">Comes back {when} either way</span>}
        </motion.div>
      )}
      <Reader sourceId={reading} onClose={closeReader} />
    </>
  );
}

// The learner's own resource: a URL, fetched and checked like kizn's.
function AddOwn({ conceptId, onAdded }: { conceptId: string; onAdded: (c: ConceptPageView) => void }) {
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const add = () => {
    if (!url.trim()) return;
    setBusy(true);
    setError(null);
    call({ type: "add_source", concept_id: conceptId, url: url.trim() })
      .then((c) => {
        onAdded(c);
        setUrl("");
      })
      .catch((e: Error) => setError(e.message))
      .finally(() => setBusy(false));
  };
  return (
    <form
      className="mt-1 flex flex-col gap-1.5"
      onSubmit={(e) => {
        e.preventDefault();
        add();
      }}
    >
      <div className="flex items-center gap-2">
        <input
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="Add your own: https://…"
          aria-label="Add your own resource"
          className="min-w-0 flex-1 rounded-lg border border-hairline bg-canvas px-3 py-1.5 text-sm text-ink outline-none placeholder:text-ink-4 focus:border-accent/50"
        />
        <button
          type="submit"
          disabled={busy || !url.trim()}
          className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs text-ink-3 ring-1 ring-hairline transition-colors hover:bg-raised hover:text-ink disabled:opacity-50"
        >
          {busy ? <LoaderCircle size={12} className="animate-spin" aria-hidden /> : <Plus size={12} aria-hidden />}
          {busy ? "Checking…" : "Add"}
        </button>
      </div>
      <AnimatePresence>
        {error && (
          <motion.p key="e" variants={pop} initial="hidden" animate="shown" exit="gone" className="text-xs text-warn">
            {error}
          </motion.p>
        )}
      </AnimatePresence>
    </form>
  );
}
