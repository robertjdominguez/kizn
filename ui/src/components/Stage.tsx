import { type HTMLMotionProps, motion } from "motion/react";
import { stage } from "../motion";

// A staggered group: its `variants={pop}` children arrive one after another
// in reading order, and leave quickly in reverse. Runs on its own mount, so a
// screen that loads its data first still assembles itself.
export function Stage({ stagger, delay, ...props }: HTMLMotionProps<"div"> & { stagger?: number; delay?: number }) {
  return <motion.div variants={stage(stagger, delay)} initial="hidden" animate="shown" exit="gone" {...props} />;
}
