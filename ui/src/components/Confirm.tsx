import { AnimatePresence, motion } from "motion/react";
import { type ReactNode, useEffect, useState } from "react";
import { fade, presence, rise } from "../motion";

// Asks before something that can't be undone: what will happen, and a
// button that says what it does.
export function Confirm({
  open,
  title,
  children,
  action,
  onConfirm,
  onClose,
}: {
  open: boolean;
  title: string;
  children: ReactNode;
  /** The button's words ("Delete topic"). */
  action: string;
  onConfirm: () => Promise<void>;
  onClose: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!open) return;
    setError(null);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);
  const confirm = async () => {
    setBusy(true);
    setError(null);
    try {
      await onConfirm();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <AnimatePresence>
      {open && (
        <motion.div
          variants={fade}
          {...presence}
          className="fixed inset-0 z-40 flex items-start justify-center bg-crust/60 px-4 pt-24"
          onClick={onClose}
        >
          <motion.div
            variants={rise}
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="confirm-title"
            className="flex w-full max-w-md flex-col gap-3 rounded-lg border border-hairline-strong bg-subtle p-5 shadow-2xl shadow-black/40"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 id="confirm-title" className="text-base font-semibold text-ink">
              {title}
            </h2>
            <div className="text-sm text-ink-3">{children}</div>
            {error && <p className="text-sm text-danger">{error}</p>}
            <div className="flex items-center justify-end gap-2 pt-1">
              <button
                type="button"
                onClick={onClose}
                className="rounded-lg px-3 py-1.5 text-sm text-ink-3 transition-colors hover:bg-raised hover:text-ink"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={confirm}
                disabled={busy}
                className="rounded-lg bg-danger px-3 py-1.5 text-sm font-semibold text-canvas transition-opacity hover:opacity-90 disabled:opacity-60"
              >
                {action}
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
