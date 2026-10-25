import { X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { fade, presence, rise } from "../motion";
import { type AppInfo, call } from "../rpc";

// Which kizn this is and where its data lives: for bug reports and backups.
export function About({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    call({ type: "get_app_info" })
      .then(setInfo)
      .catch((e: Error) => setError(e.message));
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

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
            role="dialog"
            aria-modal="true"
            aria-labelledby="about-title"
            className="w-full max-w-md rounded-lg border border-hairline-strong bg-subtle p-5 shadow-2xl shadow-black/40"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center gap-2">
              <h2 id="about-title" className="text-base font-semibold text-ink">
                About
              </h2>
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
            {error && <p className="mt-3 text-sm text-danger">Couldn't read kizn's details: {error}</p>}
            {info && (
              <dl className="mt-3 grid grid-cols-[max-content_1fr] gap-x-4 gap-y-2 text-sm">
                <dt className="text-ink-3">Version</dt>
                <dd className="text-ink">kizn {info.version}</dd>
                <dt className="text-ink-3">Data</dt>
                <dd className="break-all font-mono text-xs text-ink-2">{info.data_dir}</dd>
                <dt className="text-ink-3">Config</dt>
                <dd className="break-all font-mono text-xs text-ink-2">{info.config_dir}</dd>
              </dl>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
