import type { Transition, Variants } from "motion/react";

// Shared motion, as in booboo: one easing curve, short durations.
// <MotionConfig reducedMotion="user"> in main.tsx honours the OS setting.

export const EASE = [0.2, 0, 0, 1] as const;
export const FAST: Transition = { duration: 0.12, ease: EASE };
export const BASE: Transition = { duration: 0.18, ease: EASE };

export const fade: Variants = {
  hidden: { opacity: 0 },
  shown: { opacity: 1, transition: BASE },
  gone: { opacity: 0, transition: FAST },
};

// Content that arrives in place: cards, rows, notices.
export const rise: Variants = {
  hidden: { opacity: 0, y: 6 },
  shown: { opacity: 1, y: 0, transition: BASE },
  gone: { opacity: 0, y: 4, transition: FAST },
};

export const presence = { initial: "hidden", animate: "shown", exit: "gone" } as const;

// ---- Orchestration --------------------------------------------------------
// Screens assemble themselves in reading order: a Stage staggers its
// children in on a spring that lands with a touch of overshoot, and sends
// them out quickly in reverse. Children take `variants={pop}` (or bead, for
// marks and dots) and nothing else, so they follow their Stage.

/** Lands with a little overshoot. */
export const SPRING: Transition = { type: "spring", stiffness: 340, damping: 26, mass: 0.9 };
/** Small things that pop: marks, dots, a correct answer. */
export const BOUNCY: Transition = { type: "spring", stiffness: 520, damping: 17, mass: 0.7 };
/** Large surfaces: sheets, the hero. */
export const GLIDE: Transition = { type: "spring", stiffness: 220, damping: 28 };

/** A parent that staggers its children in, and out in reverse. */
export function stage(stagger = 0.07, delay = 0.04): Variants {
  return {
    hidden: {},
    shown: { transition: { staggerChildren: stagger, delayChildren: delay } },
    gone: { transition: { staggerChildren: 0.025, staggerDirection: -1 } },
  };
}

/** A piece of a screen arriving: up, unblurring, settling. */
const POP_HIDDEN = { opacity: 0, y: 14, scale: 0.97, filter: "blur(6px)" };
const POP_SHOWN = { opacity: 1, y: 0, scale: 1, filter: "blur(0px)" };
const POP_GONE = { opacity: 0, y: -6, scale: 0.98, filter: "blur(3px)", transition: FAST };
export const pop: Variants = {
  hidden: POP_HIDDEN,
  shown: { ...POP_SHOWN, transition: SPRING },
  gone: POP_GONE,
};

/** A pop that also staggers its own children (a card and its contents). */
export function popStage(stagger = 0.06, delay = 0.1): Variants {
  return {
    hidden: POP_HIDDEN,
    shown: { ...POP_SHOWN, transition: { ...SPRING, staggerChildren: stagger, delayChildren: delay } },
    gone: POP_GONE,
  };
}

/** Beads: marks and dots popping into place one after another. */
export const bead: Variants = {
  hidden: { opacity: 0, scale: 0 },
  shown: { opacity: 1, scale: 1, transition: BOUNCY },
  gone: { opacity: 0, scale: 0.4, transition: FAST },
};

/** The primary button: arrives last, with a little bounce. */
export const button: Variants = {
  hidden: { opacity: 0, scale: 0.85, y: 6 },
  shown: { opacity: 1, scale: 1, y: 0, transition: BOUNCY },
  gone: { opacity: 0, scale: 0.95, transition: FAST },
};

/** Practice cards, dealt: in from the right with a slight tilt, out to the left. */
export const deal: Variants = {
  hidden: { opacity: 0, x: 48, rotate: 1.5, scale: 0.98 },
  shown: {
    opacity: 1,
    x: 0,
    rotate: 0,
    scale: 1,
    transition: { ...SPRING, staggerChildren: 0.05, delayChildren: 0.08 },
  },
  gone: { opacity: 0, x: -56, rotate: -2, scale: 0.97, transition: { duration: 0.2, ease: EASE } },
};

/** A gentle no (a missed answer). */
export const shake = { x: [0, -5, 5, -3, 3, 0], transition: { duration: 0.42, ease: EASE } };
/** A small yes (a right answer). */
export const nod = { scale: [1, 1.025, 1], transition: { duration: 0.36, ease: EASE } };

/** A pop that waits: for pieces that may mount after their Stage has run. */
export function popAfter(delay: number): Variants {
  return { hidden: POP_HIDDEN, shown: { ...POP_SHOWN, transition: { ...SPRING, delay } }, gone: POP_GONE };
}
