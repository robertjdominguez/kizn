import { ChevronRight, CircleAlert, CircleCheck, Info, KeyRound, LoaderCircle, Plus, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useState } from "react";
import type { CheckView } from "../bindings/CheckView";
import type { KeyView } from "../bindings/KeyView";
import type { ProviderView } from "../bindings/ProviderView";
import type { Role } from "../bindings/Role";
import { subscribe } from "../events";
import { presence, rise } from "../motion";
import { call, type SettingsView } from "../rpc";
import { PresetIcon, presetFor } from "./presets";
import { Wizard } from "./Wizard";

// Settings → Models. Everything that counts (whether a provider passed,
// which roles it can take) comes from kizn-app; this only shows it and
// sends what was tapped.

const ROLES: { role: Role; label: string; job: string }[] = [
  { role: "author", label: "Author", job: "writes practice items ahead of time" },
  { role: "checker", label: "Checker", job: "answers each item blind before it goes live" },
  { role: "tutor", label: "Tutor", job: "talks when you ask about something" },
];

export function Settings() {
  const [view, setView] = useState<SettingsView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [testing, setTesting] = useState<string | null>(null);
  // The connect wizard, in place of the provider list: adding (null) or
  // editing a provider. On first run (no providers) it's all there is.
  // `first`: opened because there were no providers. It stays open until
  // it's finished, even though saving the provider means there now is one.
  const [wizard, setWizard] = useState<{ provider: ProviderView | null; first?: boolean; started?: boolean } | null>(
    null,
  );

  const run = useCallback(async (p: Promise<SettingsView>) => {
    try {
      setView(await p);
      setError(null);
      return true;
    } catch (e) {
      setError((e as Error).message);
      return false;
    }
  }, []);

  useEffect(() => {
    run(call({ type: "get_settings" }));
    return subscribe((e) => {
      if (e.type === "config_changed") run(call({ type: "get_settings" }));
    });
  }, [run]);

  const test = async (name: string) => {
    setTesting(name);
    await run(call({ type: "test_provider", name }));
    setTesting(null);
  };

  const firstRun = view !== null && view.providers.length === 0 && !view.config_error;
  useEffect(() => {
    if (firstRun && wizard === null) setWizard({ provider: null, first: true });
    // A provider arrived some other way (a hand edit) before the learner
    // started: show it rather than the wizard.
    if (!firstRun && wizard?.first && !wizard.started) setWizard(null);
  }, [firstRun, wizard]);

  if (!view) {
    return error ? <p className="p-6 text-sm text-danger">Couldn't load Settings: {error}</p> : null;
  }

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-8 px-6 py-8">
      <section aria-labelledby="models-title" className="flex flex-col gap-4">
        <h2 id="models-title" className="sr-only">
          Models
        </h2>

        <AnimatePresence>
          {error && (
            <motion.div
              variants={rise}
              {...presence}
              role="alert"
              className="flex items-start gap-2 rounded-lg border border-danger/40 bg-danger/5 px-3 py-2 text-sm text-ink-2"
            >
              <CircleAlert size={15} className="mt-0.5 shrink-0 text-danger" aria-hidden />
              <p className="flex-1">{error}</p>
              <button
                type="button"
                aria-label="Dismiss"
                onClick={() => setError(null)}
                className="text-ink-3 hover:text-ink"
              >
                <X size={14} aria-hidden />
              </button>
            </motion.div>
          )}
        </AnimatePresence>

        {view.config_error && (
          <p className="rounded-lg border border-danger/40 bg-danger/5 px-3 py-2 text-sm text-ink-2">
            {view.config_error}
          </p>
        )}

        {wizard ? (
          <Wizard
            key={wizard.provider?.name ?? "new"}
            editing={wizard.provider}
            editingPreset={wizard.provider ? presetFor(wizard.provider) : null}
            taken={view.providers.map((p) => p.name)}
            onClose={wizard.first ? undefined : () => setWizard(null)}
            onStarted={() => setWizard((w) => (w ? { ...w, started: true } : w))}
            onStay={() => {
              setWizard(null);
              run(call({ type: "get_settings" }));
            }}
            onFinished={() => {
              setWizard(null);
              window.location.hash = "#/";
            }}
          />
        ) : (
          <>
            <ConfigNotice path={view.config_path} />
            <div className="flex flex-col gap-3">
              <AnimatePresence initial={false}>
                {view.providers.map((p) => (
                  <ProviderCard
                    key={p.name}
                    provider={p}
                    testing={testing === p.name}
                    onTest={() => test(p.name)}
                    onEdit={() => setWizard({ provider: p })}
                    onRemove={() => run(call({ type: "remove_provider", name: p.name }))}
                  />
                ))}
              </AnimatePresence>
            </div>
            <div>
              <button
                type="button"
                onClick={() => setWizard({ provider: null })}
                className="flex items-center gap-1.5 rounded-md bg-accent/15 px-3 py-1.5 text-sm font-medium text-accent transition-colors hover:bg-accent/25"
              >
                <Plus size={14} aria-hidden /> Add a provider
              </button>
            </div>
          </>
        )}
      </section>

      {!wizard && (
        <details className="group flex flex-col gap-3">
          <summary className="flex cursor-pointer select-none items-center gap-2 text-sm text-ink-3 hover:text-ink">
            <ChevronRight size={14} className="transition-transform group-open:rotate-90" aria-hidden />
            Roles (advanced)
          </summary>
          <section aria-labelledby="roles-title" className="mt-3 flex flex-col gap-3">
            <div className="flex flex-col gap-1">
              <h2 id="roles-title" className="sr-only">
                Roles
              </h2>
              <p className="max-w-prose text-sm text-ink-3">
                A provider can take a role once its test shows it can do the job: authoring and checking need structured
                output, tutoring needs streaming.
              </p>
            </div>
            <div className="flex flex-col divide-y divide-hairline rounded-lg border border-hairline bg-subtle">
              {ROLES.map(({ role, label, job }) => (
                <label key={role} className="flex items-center gap-4 px-4 py-3">
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="text-sm font-medium text-ink">{label}</span>
                    <span className="text-xs text-ink-3">{job}</span>
                  </span>
                  <select
                    aria-label={label}
                    value={view.roles[role] ?? ""}
                    onChange={(e) =>
                      run(
                        call({
                          type: "assign_role",
                          role,
                          provider: e.target.value === "" ? undefined : e.target.value,
                        }),
                      )
                    }
                    className="w-64 rounded-md border border-hairline-strong bg-canvas px-2 py-1.5 text-sm text-ink outline-none focus:ring-2 focus:ring-accent/40"
                  >
                    <option value="">Not set</option>
                    {view.providers.map((p) => {
                      const why = roleBlocker(p, role);
                      return (
                        <option key={p.name} value={p.name} disabled={why !== null}>
                          {why ? `${p.name} (${why})` : p.name}
                        </option>
                      );
                    })}
                  </select>
                </label>
              ))}
            </div>
            {view.notes.map((n) => (
              <p key={n} className="flex items-start gap-2 text-sm text-ink-3">
                <CircleAlert size={14} className="mt-0.5 shrink-0 text-warn" aria-hidden />
                {n}
              </p>
            ))}
          </section>
        </details>
      )}
    </div>
  );
}

function roleBlocker(p: ProviderView, role: Role): string | null {
  if (!p.check) return "not tested";
  if (p.check.roles.includes(role)) return null;
  return role === "tutor" ? "can't stream" : "no structured output";
}

function ProviderCard({
  provider: p,
  testing,
  onTest,
  onEdit,
  onRemove,
}: {
  provider: ProviderView;
  testing: boolean;
  onTest: () => void;
  onEdit: () => void;
  onRemove: () => void;
}) {
  return (
    <motion.article
      variants={rise}
      {...presence}
      layout
      aria-label={`Provider ${p.name}`}
      className="flex flex-col gap-3 rounded-lg border border-hairline bg-subtle px-4 py-3"
    >
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <span className="flex size-7 items-center justify-center rounded-md bg-raised/60 text-ink-2">
          <PresetIcon preset={presetFor(p)} size={16} />
        </span>
        <span className="text-sm font-semibold text-ink">{p.name}</span>
        <span className="rounded bg-raised/60 px-1.5 py-0.5 text-[11px] text-ink-3">{presetFor(p).name}</span>
        <code className="text-xs text-ink-2">{p.model}</code>
        <span className="flex-1" />
        <button
          type="button"
          onClick={onTest}
          disabled={testing}
          className="flex items-center gap-1.5 rounded-md bg-teal/15 px-2.5 py-1 text-xs font-medium text-teal transition-colors hover:bg-teal/25 disabled:opacity-60"
        >
          {testing && <LoaderCircle size={12} className="animate-spin" aria-hidden />}
          {testing ? "Testing…" : "Test"}
        </button>
        <button
          type="button"
          onClick={onEdit}
          className="rounded-md px-2 py-1 text-xs text-ink-3 transition-colors hover:bg-raised hover:text-ink"
        >
          Edit
        </button>
        <button
          type="button"
          onClick={onRemove}
          className="rounded-md px-2 py-1 text-xs text-ink-3 transition-colors hover:bg-raised hover:text-danger"
        >
          Remove
        </button>
      </div>
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-ink-3">
        <code>{p.base_url}</code>
        <KeyLine keyView={p.key} />
      </div>
      <AnimatePresence mode="wait" initial={false}>
        <motion.div key={p.check?.tested_at ?? "untested"} variants={rise} {...presence}>
          <CheckLine check={p.check} />
        </motion.div>
      </AnimatePresence>
    </motion.article>
  );
}

function KeyLine({ keyView: k }: { keyView: KeyView }) {
  switch (k.kind) {
    case "none":
      return null;
    case "keychain":
      return (
        <span className={`flex items-center gap-1 ${k.present ? "" : "text-danger"}`}>
          <KeyRound size={12} aria-hidden />
          {k.present ? "key in your keychain" : "key missing from your keychain"}
        </span>
      );
    case "env":
      return (
        <span className={`flex items-center gap-1 ${k.present ? "" : "text-danger"}`}>
          <KeyRound size={12} aria-hidden />
          key from ${k.var}
          {k.present ? "" : " (not set)"}
        </span>
      );
    case "refused":
      return <span className="text-danger">{k.reason}</span>;
  }
}

function CheckLine({ check }: { check: CheckView | null }) {
  if (!check) return <p className="text-sm text-ink-4">Not tested yet.</p>;
  const ok = check.roles.length === 3;
  const Icon = ok ? CircleCheck : CircleAlert;
  const tested = new Date(`${check.tested_at.replace(" ", "T")}Z`);
  const structured =
    check.structured === "native"
      ? "follows a JSON schema"
      : check.structured === "prompted"
        ? "answers in JSON (kizn parses it)"
        : "no structured output";
  return (
    <div className="flex flex-col gap-1.5">
      <p className="flex items-start gap-2 text-sm text-ink" data-testid="check-summary">
        <Icon size={15} className={`mt-0.5 shrink-0 ${ok ? "text-success" : "text-warn"}`} aria-hidden />
        <span>{check.summary}</span>
      </p>
      {check.reachable && (
        <ul className="flex flex-wrap gap-1.5 pl-6 text-[11px]">
          <Chip ok={check.reachable}>reachable</Chip>
          <Chip ok={check.structured !== "none"}>{structured}</Chip>
          <Chip ok={check.streaming}>{check.streaming ? "streams" : "can't stream"}</Chip>
        </ul>
      )}
      {check.problems.length > 0 && check.reachable && (
        <ul className="pl-6 text-xs text-ink-3">
          {check.problems.map((p) => (
            <li key={p}>{p}</li>
          ))}
        </ul>
      )}
      <p className="pl-6 text-[11px] text-ink-4">Tested {tested.toLocaleString()}</p>
    </div>
  );
}

function Chip({ ok, children }: { ok: boolean; children: string }) {
  return (
    <li
      className={`rounded border px-1.5 py-0.5 ${ok ? "border-success/40 bg-success/10 text-success" : "border-warn/40 bg-warn/10 text-warn"}`}
    >
      {children}
    </li>
  );
}

const NOTICE_KEY = "kizn.notice.config-file";

// Where settings live, once, until dismissed. Remembered per browser: a
// viewer convenience, so storage failures just mean it shows again.
function ConfigNotice({ path }: { path: string }) {
  const [hidden, setHidden] = useState(() => {
    try {
      return localStorage.getItem(NOTICE_KEY) === "dismissed";
    } catch {
      return false;
    }
  });
  const dismiss = () => {
    setHidden(true);
    try {
      localStorage.setItem(NOTICE_KEY, "dismissed");
    } catch {
      // Shows again next time; harmless.
    }
  };
  return (
    <AnimatePresence initial={false}>
      {!hidden && (
        <motion.aside
          variants={rise}
          {...presence}
          aria-label="Where settings live"
          className="flex items-start gap-2.5 rounded-lg border border-accent/30 bg-accent/5 px-3 py-2.5 text-sm"
        >
          <Info size={15} className="mt-0.5 shrink-0 text-accent" aria-hidden />
          <p className="flex-1 text-ink-3">
            Settings live in <code className="break-all text-xs text-ink-2">{path}</code>. You can edit it by hand and
            kizn picks up the change while it's running. Keys stay in your system keychain.
          </p>
          <button
            type="button"
            aria-label="Dismiss"
            onClick={dismiss}
            className="rounded p-0.5 text-ink-3 transition-colors hover:bg-raised hover:text-ink"
          >
            <X size={14} aria-hidden />
          </button>
        </motion.aside>
      )}
    </AnimatePresence>
  );
}
