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
