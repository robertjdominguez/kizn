import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AppError } from "./bindings/AppError";
import type { AppInfo } from "./bindings/AppInfo";
import type { Command } from "./bindings/Command";
import type { Reply } from "./bindings/Reply";
import type { SettingsView } from "./bindings/SettingsView";
import type { TodayView } from "./bindings/TodayView";
import type { TopicView } from "./bindings/TopicView";

// The one way the UI talks to kizn-app: a Command in, its Reply out. In the
// Tauri window that's IPC (the `rpc` command); in the browser loop it's
// POST /rpc on the dev bridge. Same JSON both ways.

// Each command's reply. `satisfies` makes tsc fail when a Command variant
// is added in Rust without a line here.
const REPLY = {
  get_today: "today",
  get_app_info: "app_info",
  get_settings: "settings",
  save_provider: "settings",
  list_models: "models",
  remove_provider: "settings",
  test_provider: "settings",
  assign_role: "settings",
  start_topic: "topic",
  stop_drafting: "topic",
  save_draft: "topic",
  confirm_topic: "topic",
  get_topic: "topic",
  retry_bank: "topic",
} as const satisfies Record<Command["type"], Reply["type"]>;

type ReplyData<T extends Reply["type"]> = Extract<Reply, { type: T }>["data"];
export type ReplyFor<C extends Command> = ReplyData<(typeof REPLY)[C["type"]]>;

export class RpcError extends Error {
  constructor(readonly error: AppError) {
    super(error.message);
  }
}

async function send(cmd: Command): Promise<Reply> {
  if (isTauri()) {
    try {
      return await invoke<Reply>("rpc", { cmd });
    } catch (e) {
      throw new RpcError(e as AppError);
    }
  }
  const res = await fetch("/rpc", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(cmd),
  });
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    throw new RpcError(body ?? { kind: "io", message: `kizn didn't answer (HTTP ${res.status})` });
  }
  return (await res.json()) as Reply;
}

export async function call<C extends Command>(cmd: C): Promise<ReplyFor<C>> {
  const reply = await send(cmd);
  if (reply.type !== REPLY[cmd.type]) throw new Error(`${cmd.type} got a ${reply.type} reply`);
  return reply.data as ReplyFor<C>;
}

export type { AppInfo, SettingsView, TodayView, TopicView };
