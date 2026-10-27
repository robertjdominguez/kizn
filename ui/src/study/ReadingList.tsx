import { Check, ChevronRight, FileText, LoaderCircle, PencilLine, Play, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { ReadingItem } from "../bindings/ReadingItem";
import type { ReadingListView } from "../bindings/ReadingListView";
import { ConceptMark } from "../components/ConceptMark";
import { subscribe } from "../events";
import { EASE, fade, GLIDE, pop, presence, stage } from "../motion";
import { call } from "../rpc";
import { ConceptSheet } from "./ConceptSheet";

// Everything to study: the concepts missed and not yet recalled, what there
// is to learn from for each, and how far the studying has got. A concept
// opens its sheet on top; closing it brings the list up to date.
export function ReadingList({
  open,
  topicId,
  onClose,
}: {
  open: boolean;
  /** One topic's concepts; across topics when absent. */
  topicId?: string;
  onClose: () => void;
}) {
  return (
    <AnimatePresence>
      {open && (
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
            aria-label="Your reading"
            initial={{ x: 64, opacity: 0 }}
            animate={{ x: 0, opacity: 1, transition: GLIDE }}
            exit={{ x: 48, opacity: 0, transition: { duration: 0.16, ease: EASE } }}
            className="flex h-full w-full max-w-lg flex-col border-l border-hairline-strong bg-subtle shadow-2xl shadow-black/40"
            onClick={(e) => e.stopPropagation()}
          >
            <List topicId={topicId} onClose={onClose} />
          </motion.aside>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function List({ topicId, onClose }: { topicId?: string; onClose: () => void }) {
  const [list, setList] = useState<ReadingListView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [concept, setConcept] = useState<string | null>(null);
  const conceptOpen = useRef(false);
  conceptOpen.current = concept !== null;

  const load = useCallback(() => {
    call({ type: "get_reading_list", topic_id: topicId })
      .then(setList)
      .catch((e: Error) => setError(e.message));
  }, [topicId]);

  useEffect(() => {
    load();
    // Resources found in the background, concepts marked studied.
    const off = subscribe((e) => {
      if (e.type === "topic_changed") load();
    });
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !conceptOpen.current && onClose();
    window.addEventListener("keydown", onKey);
    return () => {
      off();
      window.removeEventListener("keydown", onKey);
    };
  }, [load, onClose]);

  const closeConcept = useCallback(() => {
    setConcept(null);
    load();
  }, [load]);

  const total = list?.concepts.length ?? 0;
  const studied = Number(list?.studied ?? 0);
  const groups: [string, ReadingItem[]][] = [];
  for (const c of list?.concepts ?? []) {
    const g = groups.find(([t]) => t === c.topic_title);
    if (g) g[1].push(c);
    else groups.push([c.topic_title, [c]]);
  }
  return (
    <>
      <div className="flex flex-col gap-3 border-b border-hairline px-5 py-4">
        <div className="flex items-center gap-2">
          <h2 className="text-[11px] font-semibold uppercase tracking-wide text-warn">Your reading</h2>
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
        {list && total > 0 && (
          <div className="flex items-center gap-3">
            <span
              role="progressbar"
              aria-label="Studied so far"
              aria-valuemin={0}
              aria-valuemax={total}
              aria-valuenow={studied}
              className="h-1.5 flex-1 overflow-hidden rounded-full bg-raised"
            >
              <motion.span
                className="block h-full rounded-full bg-success"
                initial={{ width: 0 }}
                animate={{ width: `${(studied / total) * 100}%` }}
                transition={{ duration: 0.5, ease: EASE }}
              />
            </span>
            <span className="text-xs tabular-nums text-ink-3">
              {studied} of {total} studied
            </span>
          </div>
        )}
      </div>
      <motion.div
        variants={stage(0.08, 0.1)}
        initial="hidden"
        animate={list ? "shown" : "hidden"}
        className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto px-3 py-4"
      >
        {error && <p className="px-2 text-sm text-danger">{error}</p>}
        {list && total === 0 && (
          <motion.p variants={pop} className="px-2 text-sm text-ink-3">
            Nothing to study right now: every concept you've met has been recalled.
          </motion.p>
        )}
        {groups.map(([title, items]) => (
          <motion.section key={title} variants={stage(0.05, 0)} aria-label={title} className="flex flex-col gap-1">
            {groups.length > 1 || !topicId ? (
              <motion.h3 variants={pop} className="px-2 text-xs font-medium text-ink-3">
                {title}
              </motion.h3>
            ) : null}
            {items.map((c) => (
              <motion.button
                key={c.concept_id}
                variants={pop}
                type="button"
                onClick={() => setConcept(c.concept_id)}
                className="group flex w-full items-start gap-2.5 rounded-lg px-2 py-2 text-left transition-colors hover:bg-raised/50"
              >
                <span className="mt-0.5 shrink-0 text-warn">
                  <ConceptMark state="seen" size={14} />
                </span>
                <span className="line-clamp-2 flex-1 text-sm text-ink">{c.statement}</span>
                <Materials item={c} />
                {c.studied ? (
                  <Check size={14} className="mt-0.5 shrink-0 text-success" aria-label="Studied" />
                ) : (
                  <ChevronRight size={14} className="mt-0.5 shrink-0 text-ink-4 group-hover:text-ink" aria-hidden />
                )}
              </motion.button>
            ))}
          </motion.section>
        ))}
      </motion.div>
      <ConceptSheet conceptId={concept} onClose={closeConcept} />
    </>
  );
}

// What there is to learn from: an icon per page and video, "finding…"
// while kizn checks, and a pencil when there's a note.
function Materials({ item: c }: { item: ReadingItem }) {
  const pages = Number(c.pages);
  const videos = Number(c.videos);
  const label = [
    pages && `${pages} ${pages === 1 ? "page" : "pages"}`,
    videos && `${videos} ${videos === 1 ? "video" : "videos"}`,
  ]
    .filter(Boolean)
    .join(", ");
  return (
    <span
      className="mt-0.5 flex shrink-0 items-center gap-1 text-ink-4"
      role="img"
      aria-label={label || (c.finding ? "finding resources" : "no resources yet")}
    >
      {c.finding && pages + videos === 0 ? (
        <span className="flex items-center gap-1 text-[11px]">
          <LoaderCircle size={11} className="animate-spin" aria-hidden /> finding…
        </span>
      ) : (
        <>
          {Array.from({ length: pages }, (_, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: icons are positional
            <FileText key={`p${i}`} size={12} aria-hidden />
          ))}
          {Array.from({ length: videos }, (_, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: icons are positional
            <Play key={`v${i}`} size={12} aria-hidden />
          ))}
        </>
      )}
      {c.has_note && <PencilLine size={12} className="ml-1 text-ink-3" aria-label="Has a note" />}
    </span>
  );
}
