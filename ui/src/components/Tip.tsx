import { Info } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { type ReactNode, useId, useState } from "react";
import { EASE } from "../motion";

// A small info mark that explains on hover or focus: details that matter
// sometimes, kept out of the way the rest of the time.
export function Tip({ label, children }: { label: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const id = useId();
  return (
    <span className="relative inline-flex">
      <button
        type="button"
        aria-label={label}
        aria-describedby={open ? id : undefined}
        onMouseEnter={() => setOpen(true)}
        onMouseLeave={() => setOpen(false)}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        onKeyDown={(e) => e.key === "Escape" && setOpen(false)}
        className="rounded p-0.5 text-ink-4 transition-colors hover:text-ink-2 focus-visible:text-ink-2"
      >
        <Info size={12} aria-hidden />
      </button>
      <AnimatePresence>
        {open && (
          <motion.span
            id={id}
            role="tooltip"
            initial={{ opacity: 0, y: 4, scale: 0.97 }}
            animate={{ opacity: 1, y: 0, scale: 1, transition: { duration: 0.16, ease: EASE } }}
            exit={{ opacity: 0, y: 2, transition: { duration: 0.1 } }}
            className="absolute bottom-full right-0 z-40 mb-1.5 w-60 rounded-lg bg-raised px-3 py-2 text-xs leading-relaxed text-ink-2 shadow-xl shadow-black/30 ring-1 ring-hairline-strong"
          >
            {children}
          </motion.span>
        )}
      </AnimatePresence>
    </span>
  );
}
