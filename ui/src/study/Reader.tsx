import { ArrowLeft, ExternalLink, FileText, Play } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { Fragment, useEffect, useRef, useState } from "react";
import type { BlockView } from "../bindings/BlockView";
import type { ResourceView } from "../bindings/ResourceView";
import { openExternal } from "../external";
import { EASE, GLIDE, pop, stage } from "../motion";
import { call } from "../rpc";

// Reading (or watching) a resource inside kizn: the concept on top, the
// page's text or the video's transcript, and the concept's note beside it.
// Rises over whatever opened it; closing returns there. Videos play in the
// learner's browser, at the moment clicked.
export function Reader({ sourceId, onClose }: { sourceId: string | null; onClose: () => void }) {
  return (
    <AnimatePresence>
      {sourceId && (
        <motion.div
          key={sourceId}
          role="dialog"
          aria-modal="true"
          aria-label="Reader"
          initial={{ opacity: 0, y: 40 }}
          animate={{ opacity: 1, y: 0, transition: GLIDE }}
          exit={{ opacity: 0, y: 24, transition: { duration: 0.16, ease: EASE } }}
          className="fixed inset-0 z-50 flex flex-col bg-canvas"
        >
          <Read sourceId={sourceId} onClose={onClose} />
        </motion.div>
      )}
    </AnimatePresence>
  );
}

const time = (s: number) => {
  const m = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  return `${m}:${String(sec).padStart(2, "0")}`;
};

const at = (url: string, s: number) => `${url}${url.includes("?") ? "&" : "?"}t=${Math.floor(s)}s`;

function Read({ sourceId, onClose }: { sourceId: string; onClose: () => void }) {
  const [r, setR] = useState<ResourceView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    call({ type: "get_resource", source_id: sourceId })
      .then(setR)
      .catch((e: Error) => setError(e.message));
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !(e.target instanceof HTMLTextAreaElement)) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [sourceId, onClose]);

  const video = r?.source.kind === "video";
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
        {r && (
          <button
            type="button"
            onClick={() => openExternal(r.source.url)}
            className="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs text-ink-3 transition-colors hover:bg-raised hover:text-ink"
          >
            {video ? <Play size={12} aria-hidden /> : <ExternalLink size={12} aria-hidden />}
            {video ? "Watch on YouTube" : "Open in browser"}
          </button>
        )}
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {error && <p className="mx-auto max-w-3xl px-6 py-8 text-sm text-danger">{error}</p>}
        {r && (
          <motion.div
            variants={stage(0.1, 0.12)}
            initial="hidden"
            animate="shown"
            className="mx-auto grid w-full max-w-5xl grid-cols-1 gap-10 px-6 py-8 lg:grid-cols-[minmax(0,1fr)_16rem]"
          >
            <article className="flex min-w-0 flex-col gap-6">
              <motion.div variants={pop} className="flex flex-col gap-2 border-b border-hairline pb-5">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-accent">
                  {video ? "Watching for" : "Reading for"}
                </p>
                <p className="text-base font-medium text-ink">{r.concept}</p>
                <p className="flex items-center gap-1.5 text-xs text-ink-4">
                  {video ? <Play size={12} aria-hidden /> : <FileText size={12} aria-hidden />}
                  <span className="text-ink-3">{r.source.title}</span> · {r.source.host}
                </p>
              </motion.div>
              <motion.div variants={pop} className="flex flex-col gap-4 text-[15px] leading-relaxed text-ink-2">
                {video ? <Transcript resource={r} /> : <Page blocks={r.blocks} />}
              </motion.div>
            </article>
            <motion.aside
              variants={pop}
              className="flex flex-col gap-3 lg:sticky lg:top-8 lg:self-start"
              aria-label="Notes"
            >
              <Notes conceptId={r.concept_id} initial={r.note ?? ""} />
              {video && r.source.automatic && (
                <p className="text-xs text-ink-4">
                  These captions were generated automatically, so expect a few mistakes.
                </p>
              )}
            </motion.aside>
          </motion.div>
        )}
      </div>
    </>
  );
}

// A page's text as blocks: headings, paragraphs, lists, code. Consecutive
// list items share a list.
function Page({ blocks }: { blocks: BlockView[] }) {
  const groups: BlockView[][] = [];
  for (const b of blocks) {
    const last = groups.at(-1);
    if (b.kind === "li" && last?.[0]?.kind === "li") last.push(b);
    else groups.push([b]);
  }
  return (
    <>
      {groups.map((g, i) => {
        const first = g[0];
        if (!first) return null;
        if (first.kind === "li") {
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: blocks are positional
            <ul key={i} className="ml-5 list-disc space-y-1.5">
              {g.map((b, j) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: blocks are positional
                <li key={j}>{b.text}</li>
              ))}
            </ul>
          );
        }
        if (first.kind === "h") {
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: blocks are positional
            <h2 key={i} className="pt-2 text-lg font-semibold tracking-tight text-ink">
              {first.text}
            </h2>
          );
        }
        if (first.kind === "pre") {
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: blocks are positional
            <pre key={i} className="overflow-x-auto rounded-lg bg-subtle px-4 py-3 font-mono text-[13px] text-ink">
              {first.text}
            </pre>
          );
        }
        // biome-ignore lint/suspicious/noArrayIndexKey: blocks are positional
        return <p key={i}>{first.text}</p>;
      })}
    </>
  );
}

// A video's transcript: each line with its moment, which opens the video
// there in the browser.
function Transcript({ resource: r }: { resource: ResourceView }) {
  return (
    <ol className="flex flex-col">
      {r.cues.map((c, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: cues are positional
        <Fragment key={i}>
          <li className="group flex items-baseline gap-3 rounded-lg px-2 py-1 hover:bg-raised/40">
            <button
              type="button"
              onClick={() => openExternal(at(r.source.url, c.start))}
              className="w-12 shrink-0 text-right font-mono text-xs tabular-nums text-ink-4 transition-colors group-hover:text-accent"
              title="Watch from here"
            >
              {time(c.start)}
            </button>
            <span>{c.text}</span>
          </li>
        </Fragment>
      ))}
    </ol>
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
      <p className="text-xs text-ink-4">One note per concept, beside every resource for it.</p>
    </>
  );
}
