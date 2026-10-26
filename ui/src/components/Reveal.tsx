import { AnimatePresence, motion } from "motion/react";
import type { ReactNode } from "react";
import { presence, rise } from "../motion";

// Shows its children while `when` holds, rising in and falling away, so
// nothing on screen pops in or vanishes (everything animates on mount and
// unmount). `initial` animates the first render too: for things that arrive
// after the screen is already up.
export function Reveal({
  when,
  children,
  className,
  initial = false,
}: {
  when: unknown;
  children: ReactNode;
  className?: string;
  initial?: boolean;
}) {
  return (
    <AnimatePresence initial={initial}>
      {when ? (
        <motion.div key="reveal" variants={rise} {...presence} className={className}>
          {children}
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}
