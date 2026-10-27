import { ArrowDown, ArrowRight, ArrowUp, Layers, Play, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { MapEdge } from "../bindings/MapEdge";
import type { MapNode } from "../bindings/MapNode";
import type { MapView as MapData } from "../bindings/MapView";
import { Screen } from "../components/Screen";
import { Toast } from "../components/Toast";
import { subscribe } from "../events";
import { BOUNCY, EASE, GLIDE, pop } from "../motion";
import { call } from "../rpc";

// Every topic and what it rests on, as a map: foundations at the bottom,
// broader topics above, suggestions dashed. Selecting a topic lights what it
// rests on and what rests on it; everything else dims.
const W = 208;
const H = 76;
const GAP_X = 28;
const GAP_Y = 76;

type Placed = MapNode & { x: number; y: number };

/** Each topic's row from the top: one below the lowest topic resting on it,
 *  so a foundation sits just under what it supports and lines stay short. */
function depths(nodes: MapNode[], edges: MapEdge[]): Map<string, number> {
  const depth = new Map<string, number>();
  const visit = (id: string, seen: Set<string>): number => {
    const known = depth.get(id);
    if (known !== undefined) return known;
    if (seen.has(id)) return 0;
    seen.add(id);
    const uppers = edges.filter((e) => e.lower === id).map((e) => e.upper);
    const d = uppers.length ? Math.max(...uppers.map((u) => visit(u, seen) + 1)) : 0;
    depth.set(id, d);
    return d;
  };
  for (const n of nodes) visit(n.id, new Set());
  return depth;
}

/** Positions: rows from the top, ordered within a row to keep lines short (a
 *  few passes of averaging neighbours' positions). */
function layout(nodes: MapNode[], edges: MapEdge[]): { placed: Placed[]; width: number; height: number } {
  const depth = depths(nodes, edges);
  const maxDepth = Math.max(0, ...depth.values());
  const rows: MapNode[][] = Array.from({ length: maxDepth + 1 }, () => []);
  for (const n of [...nodes].sort((a, b) => a.title.localeCompare(b.title))) rows[depth.get(n.id) ?? 0]?.push(n);
  const index = new Map<string, number>();
  const reindex = () => {
    for (const row of rows) {
      row.forEach((n, i) => {
        index.set(n.id, i / Math.max(1, row.length - 1));
      });
    }
  };
  reindex();
  // A node's neighbours' average position; with none, it keeps its own.
  const avg = (ids: string[], self: string) =>
    ids.length ? ids.reduce((sum, id) => sum + (index.get(id) ?? 0.5), 0) / ids.length : (index.get(self) ?? 0.5);
  // Sweep down then up, ordering each row by where the topics above it sit
  // (going down) or below it (going up); a topic with none on that side goes
  // by the other side. The last sweep is downward, so a topic ends up under
  // what it supports.
  const uppers = new Map<string, string[]>();
  const lowers = new Map<string, string[]>();
  for (const e of edges) {
    uppers.set(e.lower, [...(uppers.get(e.lower) ?? []), e.upper]);
    lowers.set(e.upper, [...(lowers.get(e.upper) ?? []), e.lower]);
  }
  const order = (row: MapNode[] | undefined, down: boolean) => {
    if (!row) return;
    const near = (id: string) => {
      const [first, second] = down ? [uppers, lowers] : [lowers, uppers];
      return first.get(id)?.length ? (first.get(id) ?? []) : (second.get(id) ?? []);
    };
    const key = new Map(row.map((n) => [n.id, avg(near(n.id), n.id)]));
    row.sort((a, b) => (key.get(a.id) ?? 0) - (key.get(b.id) ?? 0));
    reindex();
  };
  for (let pass = 0; pass < 4; pass++) {
    for (let r = 1; r < rows.length; r++) order(rows[r], true);
    for (let r = rows.length - 2; r >= 0; r--) order(rows[r], false);
  }
  for (let r = 1; r < rows.length; r++) order(rows[r], true);
  const widest = Math.max(1, ...rows.map((r) => r.length));
  const width = widest * W + (widest - 1) * GAP_X;
  const placed: Placed[] = [];
  rows.forEach((row, r) => {
    const rowWidth = row.length * W + (row.length - 1) * GAP_X;
    const start = (width - rowWidth) / 2;
    row.forEach((n, i) => {
      placed.push({ ...n, x: start + i * (W + GAP_X), y: r * (H + GAP_Y) });
    });
  });
  return { placed, width, height: rows.length * H + (rows.length - 1) * GAP_Y };
}

/** Everything above and below a topic, following the links. */
function connected(id: string, edges: MapEdge[]): Set<string> {
  const out = new Set([id]);
  for (const dir of ["down", "up"] as const) {
    const stack = [id];
    while (stack.length) {
      const t = stack.pop() as string;
      for (const e of edges) {
        const next = dir === "down" ? (e.upper === t ? e.lower : null) : e.lower === t ? e.upper : null;
        if (next && !out.has(next)) {
          out.add(next);
          stack.push(next);
        }
      }
    }
  }
  return out;
}

const dashOf = (e: MapEdge) => (e.suggested ? "5 5" : e.kind === "part_of" ? "1.5 4" : undefined);
const edgeKey = (e: MapEdge) => `${e.upper}-${e.lower}`;

// The entrance, from the ground up: the bottom row first, each row a beat
// after the one below, topics in a row one after another; a link draws once
// the topic above it has landed.
const FIRST = 0.35;
const ROW = 0.5;
const IN_ROW = 0.1;
const LINE = 0.6;

/** What was just added, to hold back until the rest of the map has arrived:
 *  new suggestions (with their links), or only the link to a topic you have.
 *  Named by node id, or (from a flag) by title; all of them under `under`.
 *  From a check, they're where to start. */
export type Added = { under: string; nodes?: string[]; title?: string; from?: "check" | "flag" };

type Held = { nodes: Set<string>; edges: Set<string>; order: Map<string, number> };

function heldBack(map: MapData, added: Added): Held | null {
  const title = added.title?.toLowerCase();
  const held: Held = { nodes: new Set(), edges: new Set(), order: new Map() };
  for (const edge of map.edges) {
    if (edge.upper !== added.under) continue;
    const lower = map.nodes.find((n) => n.id === edge.lower);
    const named = added.nodes?.includes(edge.lower) || (!!title && lower?.title.toLowerCase() === title);
    if (!named || !lower) continue;
    const i = added.nodes?.indexOf(edge.lower) ?? 0;
    const alone = map.edges.filter((e) => e.lower === lower.id || e.upper === lower.id).length === 1;
    if (lower.kind === "suggestion" && alone) held.nodes.add(lower.id);
    held.edges.add(edgeKey(edge));
    // Each arrival lands a beat after the one before.
    held.order.set(lower.id, Math.max(0, i));
    held.order.set(edgeKey(edge), Math.max(0, i));
  }
  return held.edges.size ? held : null;
}

/** What the toast says about what just arrived: from a check, where to start;
 *  from a flag, where it went. */
function arrivalMessage(map: MapData, added: Added | undefined, ids: string[]): ReactNode {
  if (!added || ids.length === 0) return null;
  const titles = ids.map((id) => map.nodes.find((n) => n.id === id)?.title).filter((t): t is string => !!t);
  const upper = map.nodes.find((n) => n.id === added.under)?.title ?? "your topic";
  const list = titles.length > 1 ? `${titles.slice(0, -1).join(", ")} and ${titles.at(-1)}` : titles[0];
  if (added.from === "check") {
    return (
      <>
        <span className="font-semibold text-ink">Start here.</span> Learning {list} first makes {upper} easier.{" "}
        {titles.length > 1 ? "Select one to start it." : "Select it to start."}
      </>
    );
  }
  return (
    <>
      <span className="font-medium text-ink">{list}</span> is on your map, under {upper}.
    </>
  );
}

export function MapView({ added }: { added?: Added }) {
  const [map, setMap] = useState<MapData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  // Once the entrance has played, changes glide instead of replaying it.
  const [settled, setSettled] = useState(false);
  const [held, setHeld] = useState<Held | null>(null);
  // Just arrived, each with its place in the order they land.
  const [fresh, setFresh] = useState<Map<string, number>>(new Map());
  const [toast, setToast] = useState<ReactNode>(null);
  const closeToast = useCallback(() => setToast(null), []);
  // On the first load, find what was just added (if anything) and hold it
  // back, in the same update, so it's never laid out with the rest.
  const [first, setFirst] = useState(false);
  const addedRef = useRef(added);
  const loaded = useRef(false);
  const load = useCallback(() => {
    call({ type: "get_map" })
      .then((m) => {
        if (!loaded.current) {
          loaded.current = true;
          if (addedRef.current) {
            setHeld(heldBack(m, addedRef.current));
            // Arriving once is enough: a reload shouldn't replay it.
            window.history.replaceState(null, "", "#/map");
          }
          setFirst(true);
        }
        setMap(m);
      })
      .catch((e: Error) => setError(e.message));
  }, []);
  useEffect(() => {
    load();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setSelected(null);
    window.addEventListener("keydown", onKey);
    const off = subscribe((e) => {
      if (e.type === "topic_changed") load();
    });
    return () => {
      off();
      window.removeEventListener("keydown", onKey);
    };
  }, [load]);

  const shown = useMemo(() => {
    if (!map) return null;
    if (!held) return map;
    return {
      ...map,
      nodes: map.nodes.filter((n) => !held.nodes.has(n.id)),
      edges: map.edges.filter((e) => !held.edges.has(edgeKey(e))),
    };
  }, [map, held]);

  const { placed, width, height } = useMemo(
    () => (shown ? layout(shown.nodes, shown.edges) : { placed: [], width: 0, height: 0 }),
    [shown],
  );
  const rows = Math.max(0, ...placed.map((p) => Math.round(p.y / (H + GAP_Y))));
  // Entrance delays, kept from the first layout so later changes don't wait.
  const delays = useMemo(() => {
    const out = new Map<string, number>();
    const byRow = new Map<number, Placed[]>();
    for (const p of placed) {
      const r = Math.round(p.y / (H + GAP_Y));
      byRow.set(r, [...(byRow.get(r) ?? []), p]);
    }
    for (const [r, row] of byRow) {
      row.sort((a, b) => a.x - b.x);
      row.forEach((p, i) => {
        out.set(p.id, FIRST + (rows - r) * ROW + i * IN_ROW);
      });
    }
    return out;
  }, [placed, rows]);
  const entranceEnds = Math.max(0, ...delays.values()) + 0.3 + LINE;

  useEffect(() => {
    if (!first || settled) return;
    const t = window.setTimeout(() => setSettled(true), entranceEnds * 1000);
    return () => window.clearTimeout(t);
  }, [first, settled, entranceEnds]);

  // After the entrance, let the addition in: the map makes room, it lands,
  // and it's selected so its links light up.
  useEffect(() => {
    if (!settled || !held) return;
    const t = window.setTimeout(() => {
      setFresh(held.order);
      setHeld(null);
      // The first arrival is selected, so its links light up.
      const arrived = [...held.order.entries()]
        .filter(([k]) => !held.edges.has(k))
        .sort((a, b) => a[1] - b[1])
        .map(([k]) => k);
      if (arrived[0]) setSelected(arrived[0]);
      if (map) setToast(arrivalMessage(map, addedRef.current, arrived));
    }, 350);
    return () => window.clearTimeout(t);
  }, [settled, held, map]);

  const lit = useMemo(() => (selected && shown ? connected(selected, shown.edges) : null), [selected, shown]);
  const at = new Map(placed.map((p) => [p.id, p]));
  const chosen = selected ? at.get(selected) : undefined;

  return (
    <Screen
      width="max-w-6xl"
      header={
        <motion.div
          variants={pop}
          initial="hidden"
          animate="shown"
          className="flex flex-wrap items-end gap-x-6 gap-y-2"
        >
          <div className="flex flex-col gap-0.5">
            <h1 className="flex items-center gap-2 text-2xl font-semibold tracking-tight text-ink">
              <Layers size={20} className="text-ink-3" aria-hidden /> Your map
            </h1>
            <p className="text-sm text-ink-3">What rests on what: foundations at the bottom.</p>
          </div>
          <span className="flex-1" />
          <Legend />
        </motion.div>
      }
    >
      {error && <p className="text-sm text-danger">{error}</p>}
      {map && map.nodes.length === 0 && <p className="text-sm text-ink-3">Your map fills in as you add topics.</p>}
      {shown && shown.nodes.length > 0 && (
        // Scrolling sideways clips both ways: room around the map for a
        // selected topic's lift and an arrival's glow.
        <div className="relative -mx-2 overflow-x-auto px-2 pt-3 pb-6">
          <motion.div
            className="relative mx-auto"
            initial={false}
            animate={{ width, height: height + 8 }}
            transition={GLIDE}
          >
            <svg width={width} height={height} className="absolute inset-0 overflow-visible" aria-hidden>
              {shown.edges.map((e) => {
                const a = at.get(e.upper);
                const b = at.get(e.lower);
                if (!a || !b) return null;
                const x1 = a.x + W / 2;
                const y1 = a.y + H;
                const x2 = b.x + W / 2;
                const y2 = b.y;
                const mid = (y1 + y2) / 2;
                const d = `M${x1},${y1} C${x1},${mid} ${x2},${mid} ${x2},${y2}`;
                const on = !lit || (lit.has(e.upper) && lit.has(e.lower));
                const dash = dashOf(e);
                // Drawn once the topic above has landed; an added link waits
                // for its new topic to pop in.
                const delay = !settled
                  ? (delays.get(e.upper) ?? FIRST) + 0.3
                  : fresh.has(edgeKey(e))
                    ? 0.45 + (fresh.get(edgeKey(e)) ?? 0) * 0.2
                    : 0;
                return (
                  // Solid links draw themselves; dashed and dotted ones fade in
                  // (drawing would overwrite their dashes).
                  <motion.path
                    key={edgeKey(e)}
                    fill="none"
                    stroke={e.kind === "part_of" ? "var(--color-sapphire)" : "var(--color-overlay-2)"}
                    strokeWidth={on && lit ? 2 : 1.5}
                    strokeDasharray={dash}
                    strokeLinecap="round"
                    initial={dash ? { opacity: 0, d } : { pathLength: 0, opacity: 0, d }}
                    animate={{
                      ...(dash ? {} : { pathLength: 1 }),
                      d,
                      opacity: on ? 0.9 : 0.15,
                      transition: {
                        default: { duration: LINE, ease: EASE, delay },
                        d: { ...GLIDE, delay: 0 },
                        opacity: { duration: settled ? 0.3 : LINE, ease: EASE, delay },
                      },
                    }}
                  />
                );
              })}
            </svg>
            {placed.map((n) => (
              <Node
                key={n.id}
                n={n}
                dim={!!lit && !lit.has(n.id)}
                selected={selected === n.id}
                delay={
                  settled ? (fresh.has(n.id) ? 0.2 + (fresh.get(n.id) ?? 0) * 0.2 : 0) : (delays.get(n.id) ?? FIRST)
                }
                fresh={fresh.has(n.id)}
                onSelect={() => setSelected(n.id === selected ? null : n.id)}
              />
            ))}
          </motion.div>
        </div>
      )}
      <Toast onClose={closeToast}>{toast}</Toast>
      <AnimatePresence>
        {chosen && <Selected key={chosen.id} n={chosen} map={shown} onClose={() => setSelected(null)} />}
      </AnimatePresence>
    </Screen>
  );
}

function Node({
  n,
  dim,
  selected,
  delay,
  fresh,
  onSelect,
}: {
  n: Placed;
  dim: boolean;
  selected: boolean;
  /** When it arrives; position changes later glide without waiting. */
  delay: number;
  /** Just added: it lands with a bounce and a brief glow. */
  fresh: boolean;
  onSelect: () => void;
}) {
  const suggestion = n.kind === "suggestion";
  const total = Number(n.concepts);
  const part = (x: number | bigint) => (total ? (Number(x) / total) * 100 : 0);
  return (
    <motion.button
      type="button"
      aria-pressed={selected}
      aria-label={`${n.title}${suggestion ? " (suggested)" : ""}`}
      onClick={onSelect}
      initial={
        fresh ? { opacity: 0, scale: 0.5, x: n.x, y: n.y + 24 } : { opacity: 0, scale: 0.9, x: n.x, y: n.y + 18 }
      }
      animate={{
        opacity: dim ? 0.3 : 1,
        scale: selected ? 1.03 : 1,
        x: n.x,
        y: n.y,
        transition: {
          default: { ...(fresh ? BOUNCY : GLIDE), delay },
          opacity: { duration: 0.35, ease: EASE, delay },
        },
      }}
      whileHover={{ scale: selected ? 1.03 : 1.015 }}
      style={{ left: 0, top: 0, width: W, height: H }}
      className={`absolute flex flex-col justify-between rounded-xl px-3 py-2.5 text-left transition-shadow ${
        suggestion
          ? `border border-dashed bg-canvas ${selected ? "border-mauve/70" : "border-ink-4/60"}`
          : `bg-subtle ring-1 ${selected ? "ring-accent shadow-lg shadow-accent/10" : "ring-hairline hover:ring-hairline-strong"}`
      }`}
    >
      {fresh && (
        <motion.span
          aria-hidden
          className="pointer-events-none absolute -inset-1 rounded-[14px] ring-2 ring-mauve"
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: [0, 0.9, 0], scale: [0.9, 1.04, 1.1] }}
          transition={{ duration: 1.1, ease: EASE, delay: delay + 0.15 }}
        />
      )}
      <span className={`line-clamp-2 text-[13px] font-medium leading-snug ${suggestion ? "text-ink-3" : "text-ink"}`}>
        {n.title}
      </span>
      {suggestion ? (
        <span className="text-[11px] text-ink-4">suggested</span>
      ) : n.status === "draft" ? (
        <span className="text-[11px] text-ink-4">being set up</span>
      ) : (
        <span
          className="flex h-1.5 w-full overflow-hidden rounded-full bg-raised"
          role="img"
          aria-label={`${n.retained} of ${total} stuck`}
        >
          <span className="h-full bg-teal" style={{ width: `${part(n.retained)}%` }} />
          <span className="h-full bg-teal/45" style={{ width: `${part(n.recalled)}%` }} />
          <span className="h-full bg-warn/60" style={{ width: `${part(n.seen)}%` }} />
        </span>
      )}
    </motion.button>
  );
}

function Selected({ n, map, onClose }: { n: Placed; map: MapData | null; onClose: () => void }) {
  const edges = map?.edges ?? [];
  const title = (id: string) => map?.nodes.find((x) => x.id === id)?.title ?? "";
  const restsOn = edges.filter((e) => e.upper === n.id).map((e) => title(e.lower));
  const supports = edges.filter((e) => e.lower === n.id).map((e) => title(e.upper));
  // Opens setup, prefilled, for the learner to make it theirs.
  const start = () => {
    window.location.hash = `#/new-topic?foundation=${encodeURIComponent(n.id)}`;
  };
  // Below it (what it rests on) and above it (what it supports), as arrows.
  const linked = (Icon: typeof ArrowDown, label: string, titles: string[]) =>
    titles.length > 0 && (
      <ul aria-label={label} className="flex flex-col gap-1">
        {titles.map((t) => (
          <li key={t} className="flex items-center gap-1.5 text-xs text-ink-3">
            <Icon size={12} className="shrink-0 text-ink-4" aria-label={label} />
            {t}
          </li>
        ))}
      </ul>
    );
  return (
    <motion.aside
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0, transition: GLIDE }}
      exit={{ opacity: 0, y: 10, transition: { duration: 0.15 } }}
      aria-label="Selected topic"
      className="fixed right-6 bottom-6 z-30 flex w-80 flex-col gap-2 rounded-2xl bg-subtle px-4 py-3.5 shadow-2xl shadow-black/40 ring-1 ring-hairline-strong"
    >
      <div className="flex items-start gap-2">
        <p className="flex-1 text-sm font-semibold text-ink">{n.title}</p>
        <button type="button" aria-label="Close" onClick={onClose} className="text-ink-4 hover:text-ink">
          <X size={14} aria-hidden />
        </button>
      </div>
      {n.why && <p className="text-xs text-ink-3">{n.why}</p>}
      {linked(ArrowUp, "Supports", supports)}
      {linked(ArrowDown, "Rests on", restsOn)}
      {n.kind === "suggestion" ? (
        <button
          type="button"
          onClick={start}
          className="flex w-fit items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-xs font-semibold text-canvas"
        >
          <Play size={12} aria-hidden /> Start it
        </button>
      ) : (
        <a
          href={
            n.status === "draft"
              ? `#/setup/${encodeURIComponent(n.id)}`
              : `#/topic/${encodeURIComponent(n.id)}?from=map`
          }
          className="flex w-fit items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-xs font-semibold text-canvas"
        >
          Open <ArrowRight size={12} aria-hidden />
        </a>
      )}
    </motion.aside>
  );
}

function Legend() {
  const item = (dash: string | undefined, color: string, label: string) => (
    <span className="flex items-center gap-1.5">
      <svg width="22" height="6" aria-hidden>
        <line
          x1="1"
          y1="3"
          x2="21"
          y2="3"
          stroke={color}
          strokeWidth="1.5"
          strokeDasharray={dash}
          strokeLinecap="round"
        />
      </svg>
      {label}
    </span>
  );
  return (
    <p className="flex flex-wrap items-center gap-4 text-xs text-ink-4">
      {item(undefined, "var(--color-overlay-2)", "rests on")}
      {item("1.5 4", "var(--color-sapphire)", "part of")}
      {item("5 5", "var(--color-overlay-2)", "suggested")}
    </p>
  );
}
