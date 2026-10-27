import { X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { type ReactNode, useEffect } from "react";
import { GLIDE } from "../motion";

// A short message at the bottom of the screen that leaves by itself (or when
// closed): what just happened, and where to go from here.
export function Toast({
  children,
  onClose,
  after = 9000,
}: {
  children: ReactNode;
  onClose: () => void;
  after?: number;
}) {
  const open = children !== null && children !== undefined && children !== false;
  useEffect(() => {
    if (!open) return;
    const t = window.setTimeout(onClose, after);
    return () => window.clearTimeout(t);
  }, [open, onClose, after]);
  return (
    <AnimatePresence>
      {open && (
        <motion.div
          role="status"
          initial={{ opacity: 0, y: 24, scale: 0.96 }}
          animate={{ opacity: 1, y: 0, scale: 1, transition: GLIDE }}
          exit={{ opacity: 0, y: 12, transition: { duration: 0.18 } }}
          className="fixed bottom-6 left-1/2 z-40 flex w-[min(28rem,calc(100vw-2rem))] -translate-x-1/2 items-start gap-3 rounded-2xl bg-raised px-4 py-3 text-sm text-ink-2 shadow-2xl shadow-black/40 ring-1 ring-hairline-strong"
        >
          <div className="flex-1">{children}</div>
          <button type="button" aria-label="Close" onClick={onClose} className="pt-0.5 text-ink-4 hover:text-ink">
            <X size={14} aria-hidden />
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
