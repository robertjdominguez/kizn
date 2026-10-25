import { ChevronLeft, Target } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useState } from "react";
import { subscribe } from "../events";
import { presence, rise } from "../motion";
import { call, type TopicView } from "../rpc";

// A topic: what you'll be able to do, and how its practice is coming along.
// Built in small reviewed steps; this is the header.
export function TopicPage({ topicId }: { topicId: string }) {
  const [topic, setTopic] = useState<TopicView | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    call({ type: "get_topic", topic_id: topicId })
      .then((t) => {
        setTopic(t);
        setError(null);
      })
      .catch((e: Error) => setError(e.message));
  }, [topicId]);

  useEffect(() => {
    load();
    return subscribe((e) => {
      if (e.type === "topic_changed" && e.topic_id === topicId) load();
    });
  }, [load, topicId]);

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-6 px-6 py-8">
      <motion.a
        variants={rise}
        {...presence}
        href="#/"
        className="flex w-fit items-center gap-0.5 rounded-md py-1 pr-2 text-sm text-ink-3 transition-colors hover:text-ink"
      >
        <ChevronLeft size={15} aria-hidden /> Today
      </motion.a>
      <AnimatePresence mode="wait">
        {error && (
          <motion.p key="error" variants={rise} {...presence} className="text-sm text-danger">
            Couldn't load this topic: {error}
          </motion.p>
        )}
        {topic && (
          <motion.header key="header" variants={rise} {...presence} className="flex flex-col gap-1.5">
            <h1 className="text-2xl font-semibold tracking-tight text-ink">{topic.title}</h1>
            <p className="flex items-start gap-1.5 text-sm text-ink-3">
              <Target size={15} className="mt-0.5 shrink-0 text-ink-4" aria-label="Your goal" />
              <span>“{topic.goal}”</span>
            </p>
          </motion.header>
        )}
      </AnimatePresence>
    </div>
  );
}
