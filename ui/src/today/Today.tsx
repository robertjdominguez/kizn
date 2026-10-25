import { Plug, Sprout } from "lucide-react";
import { motion } from "motion/react";
import { useEffect, useState } from "react";
import { rise } from "../motion";
import { call, type TodayView } from "../rpc";

// Home. Phase 0 has only the empty state: no topics yet, and whether a model
// is connected. Due recalls, fringe outcomes, flags and the Practice button
// arrive with the phases that make them.
export function Today() {
  const [today, setToday] = useState<TodayView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    call({ type: "get_today" })
      .then(setToday)
      .catch((e: Error) => setError(e.message));
  }, []);

  if (error) {
    return <p className="p-6 text-sm text-danger">Couldn't load Today: {error}</p>;
  }
  if (!today) return null;

  return (
    <div className="mx-auto w-full max-w-2xl px-6 py-10 flex flex-col gap-6">
      {!today.model_configured && (
        <motion.section
          variants={rise}
          initial="hidden"
          animate="shown"
          aria-label="Model"
          className="flex items-start gap-3 rounded-lg border border-warn/40 bg-warn/5 px-4 py-3 text-sm"
        >
          <Plug size={16} className="mt-0.5 shrink-0 text-warn" aria-hidden />
          <div className="flex flex-col gap-0.5">
            <p className="font-medium text-ink">No model connected</p>
            <p className="text-ink-3">
              kizn writes practice items and tutors with a model you choose: a local one or a provider you have a key
              for.{" "}
              <a href="#/settings" className="text-accent hover:underline">
                Connect a model
              </a>
            </p>
          </div>
        </motion.section>
      )}

      {today.topics.length === 0 && (
        <motion.section
          variants={rise}
          initial="hidden"
          animate="shown"
          aria-label="Topics"
          className="flex flex-col items-center gap-4 rounded-lg border border-hairline bg-subtle px-6 py-12 text-center"
        >
          <Sprout size={28} className="text-teal" aria-hidden />
          <div className="flex flex-col gap-1">
            <h2 className="text-base font-medium text-ink">Nothing to practise yet</h2>
            <p className="max-w-sm text-sm text-ink-3">
              Say what you want to be able to do and why. kizn drafts outcomes and concepts for you to edit, then writes
              practice for them.
            </p>
          </div>
          <button
            type="button"
            disabled
            title="Topic setup isn't built yet"
            className="rounded-md bg-teal/15 px-3 py-1.5 text-sm font-medium text-teal transition-colors hover:bg-teal/25 disabled:cursor-not-allowed disabled:opacity-60"
          >
            Start a topic
          </button>
        </motion.section>
      )}
    </div>
  );
}
