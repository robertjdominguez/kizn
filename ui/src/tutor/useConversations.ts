import { useCallback, useEffect, useState } from "react";
import type { Anchor } from "../bindings/Anchor";
import { call } from "../rpc";

/** How many conversations an anchor has, and a way to recount (after the
 *  tutor sheet closes). */
export function useConversations(anchor: Anchor | null): [number, () => void] {
  const [n, setN] = useState(0);
  const key = anchor ? JSON.stringify(anchor) : null;
  const recount = useCallback(() => {
    if (!key) return;
    call({ type: "get_threads", anchor: JSON.parse(key) as Anchor })
      .then((ts) => setN(ts.length))
      .catch(() => undefined);
  }, [key]);
  useEffect(recount, [recount]);
  return [n, recount];
}

export const conversations = (n: number) => (n === 0 ? "" : ` · ${n} ${n === 1 ? "conversation" : "conversations"}`);
