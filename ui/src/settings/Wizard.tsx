import {
  Activity,
  ArrowLeft,
  ArrowRight,
  ArrowUpRight,
  Boxes,
  ChevronRight,
  CircleAlert,
  CircleCheck,
  Cpu,
  Download,
  Globe,
  KeyRound,
  LoaderCircle,
  type LucideIcon,
  X,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { type FormEvent, type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import type { CheckView } from "../bindings/CheckView";
import type { ProviderView } from "../bindings/ProviderView";
import { openExternal } from "../external";
import { EASE } from "../motion";
import { call } from "../rpc";
import { PRESETS, type Preset, PresetIcon } from "./presets";

// Connecting a model, one card per step: provider → key / install / address
// → model → test → ready. A segmented progress line above the card marks
// each step. Whether the model works is decided by kizn-app's connection
// test; this walks the learner to it and says plainly what to fix.

type StepId = "provider" | "key" | "install" | "address" | "model" | "test" | "ready";

const STEP: Record<StepId, { label: string; icon: LucideIcon }> = {
  provider: { label: "Provider", icon: Boxes },
  key: { label: "API key", icon: KeyRound },
  install: { label: "Install", icon: Download },
  address: { label: "Address", icon: Globe },
  model: { label: "Model", icon: Cpu },
  test: { label: "Test", icon: Activity },
  ready: { label: "Ready", icon: CircleCheck },
};

const second = (p: Preset | null): StepId =>
  p?.kind === "local" ? "install" : p?.kind === "custom" ? "address" : "key";
const stepsFor = (p: Preset | null): StepId[] => ["provider", second(p), "model", "test", "ready"];

/** Seconds the Ready card waits before going to Today. */
const READY_SECONDS = 6;

function freeName(base: string, taken: string[]) {
  if (!taken.includes(base)) return base;
  let n = 2;
  while (taken.includes(`${base}-${n}`)) n++;
  return `${base}-${n}`;
}

export function Wizard({
  editing,
  editingPreset,
  taken,
  onClose,
  onFinished,
  onStay,
}: {
  /** A provider to change; starts at its key (or model) with earlier steps done. */
  editing: ProviderView | null;
  editingPreset: Preset | null;
  taken: string[];
  /** Absent on first run: there's nothing to go back to. */
  onClose?: () => void;
  /** After Ready: off to Today (the CTA, or when the countdown ends). */
  onFinished: () => void;
  /** "Stay here" on Ready: back to the Settings list. */
  onStay: () => void;
}) {
  const [preset, setPreset] = useState<Preset | null>(editingPreset);
  const [step, setStep] = useState<StepId>(editing ? (editingPreset?.kind === "cloud" ? "key" : "model") : "provider");
  const [dir, setDir] = useState(1);
  const [name, setName] = useState(editing?.name ?? "");
  const [baseUrl, setBaseUrl] = useState(editing?.base_url ?? "");
  const [key, setKey] = useState("");
  const [model, setModel] = useState(editing?.model ?? "");
  const [models, setModels] = useState<string[] | null>(null);
  const [check, setCheck] = useState<CheckView | null>(null);

  const steps = stepsFor(preset);
  const index = steps.indexOf(step);
  const go = (to: StepId) => {
    setDir(steps.indexOf(to) >= index ? 1 : -1);
    setStep(to);
  };
  const back = () => index > 0 && go(steps[index - 1] as StepId);

  const pick = (p: Preset) => {
    setPreset(p);
    setName(freeName(p.id, taken));
    setBaseUrl(p.baseUrl);
    setModels(null);
    setDir(1);
    setStep(second(p));
  };

  const listModels = useCallback(
    () =>
      call({
        type: "list_models",
        protocol: preset?.protocol ?? "openai",
        base_url: baseUrl.trim(),
        key: key.trim() || undefined,
        name: editing?.name,
      }),
    [preset, baseUrl, key, editing],
  );

  return (
    <section aria-label="Connect a model" className="flex flex-col gap-5">
      <Progress steps={steps} index={index} />
      <div className="relative">
        <AnimatePresence mode="wait" custom={dir} initial={false}>
          <motion.div
            key={step}
            custom={dir}
            variants={slide}
            initial="enter"
            animate="center"
            exit="exit"
            className="rounded-xl border border-hairline-strong bg-subtle p-5 shadow-xl shadow-black/20"
          >
            {step === "provider" && <ProviderStep onPick={pick} onClose={onClose} />}
            {step === "key" && preset && (
              <KeyStep
                preset={preset}
                hasSavedKey={editing?.key.kind === "keychain"}
                value={key}
                onChange={setKey}
                onBack={editing ? undefined : back}
                onClose={onClose}
                verify={async () => {
                  const list = await listModels();
                  setModels(list);
                  go("model");
                }}
              />
            )}
            {step === "install" && preset && (
              <InstallStep
                preset={preset}
                baseUrl={baseUrl}
                onBaseUrl={setBaseUrl}
                onContinue={() => go("model")}
                onBack={editing ? undefined : back}
                onClose={onClose}
                probe={listModels}
                onRunning={(list) => {
                  setModels(list);
                  go("model");
                }}
              />
            )}
            {step === "address" && preset && (
              <AddressStep
                baseUrl={baseUrl}
                keyValue={key}
                onBaseUrl={setBaseUrl}
                onKey={setKey}
                onBack={editing ? undefined : back}
                onClose={onClose}
                verify={async () => {
                  setModels(await listModels().catch(() => []));
                  go("model");
                }}
              />
            )}
            {step === "model" && preset && (
              <ModelStep
                preset={preset}
                models={models}
                loadModels={async () => setModels(await listModels())}
                value={model}
                onChange={setModel}
                baseUrl={baseUrl}
                onBaseUrl={setBaseUrl}
                onBack={back}
                onClose={onClose}
                save={async () => {
                  await call({
                    type: "save_provider",
                    name,
                    protocol: preset.protocol,
                    base_url: baseUrl.trim(),
                    model: model.trim(),
                    key: key.trim() || undefined,
                    preset: preset.id,
                  });
                  setCheck(null);
                  go("test");
                }}
              />
            )}
            {step === "test" && preset && (
              <TestStep
                preset={preset}
                name={name}
                model={model}
                check={check}
                onChecked={setCheck}
                fix={(to) => go(to)}
                second={second(preset)}
                onReady={() => go("ready")}
                onClose={onClose}
              />
            )}
            {step === "ready" && preset && check && (
              <ReadyStep preset={preset} model={model} check={check} onFinished={onFinished} onStay={onStay} />
            )}
          </motion.div>
        </AnimatePresence>
      </div>
    </section>
  );
}

const slide = {
  enter: (d: number) => ({ x: 48 * d, opacity: 0 }),
  center: { x: 0, opacity: 1, transition: { duration: 0.24, ease: EASE } },
  exit: (d: number) => ({ x: -48 * d, opacity: 0, transition: { duration: 0.16, ease: EASE } }),
};

// ---- progress ------------------------------------------------------------------

/** How long one segment takes to fill; the next waits for it. */
const FILL_SECONDS = 0.45;

function Progress({ steps, index }: { steps: StepId[]; index: number }) {
  const last = index === steps.length - 1;
  // Segments fill one at a time: when several become done at once (Test and
  // Ready on the last step), each waits for the one before it. Going back
  // empties them straight away.
  const target = last ? steps.length : index;
  const [filled, setFilled] = useState(target);
  // Where filling started for this move: the first new segment fills at
  // once, each after it waits for the one before.
  const from = useRef(filled);
  // biome-ignore lint/correctness/useExhaustiveDependencies: only when the target moves
  useEffect(() => {
    from.current = filled;
  }, [target]);
  useEffect(() => {
    if (filled > target) {
      setFilled(target);
      return;
    }
    if (filled === target) return;
    const t = setTimeout(() => setFilled((f) => f + 1), filled === from.current ? 0 : FILL_SECONDS * 1000);
    return () => clearTimeout(t);
  }, [filled, target]);
  return (
    <ol aria-label="Progress" className="grid gap-1.5" style={{ gridTemplateColumns: `repeat(${steps.length}, 1fr)` }}>
      {steps.map((s, i) => {
        const done = i < filled;
        const current = i === index && !last;
        const Icon = STEP[s].icon;
        return (
          // Keyed by position on purpose: the second step's label changes
          // (API key / Install / Address) while its segment stays put.
          // biome-ignore lint/suspicious/noArrayIndexKey: segments are positional
          <li key={i} aria-current={current ? "step" : undefined} className="flex flex-col gap-2">
            <span
              className={`flex items-center gap-1.5 text-xs transition-colors ${done || current ? "text-ink" : "text-ink-4"}`}
            >
              <motion.span
                animate={{ scale: done ? [1, 1.18, 1] : 1 }}
                transition={{ duration: 0.3, ease: EASE }}
                className={`flex size-5 items-center justify-center rounded-full border transition-colors ${
                  done
                    ? "border-accent bg-accent text-canvas"
                    : current
                      ? "border-accent text-accent"
                      : "border-hairline-strong text-ink-4"
                }`}
              >
                <Icon size={11} aria-hidden />
              </motion.span>
              <AnimatePresence mode="wait" initial={false}>
                <motion.span
                  key={STEP[s].label}
                  initial={{ opacity: 0, y: 3 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -3 }}
                  transition={{ duration: 0.15 }}
                  className="truncate"
                >
                  {STEP[s].label}
                </motion.span>
              </AnimatePresence>
            </span>
            <span className="h-1.5 overflow-hidden rounded-full bg-raised">
              <motion.span
                className="block h-full rounded-full bg-accent"
                initial={false}
                animate={{ width: done ? "100%" : "0%" }}
                transition={{ duration: FILL_SECONDS, ease: EASE }}
              />
            </span>
          </li>
        );
      })}
    </ol>
  );
}

// ---- pieces shared by the steps ------------------------------------------------

function CardHead({
  preset,
  title,
  sub,
  onClose,
}: {
  preset?: Preset;
  title: string;
  sub?: ReactNode;
  onClose?: () => void;
}) {
  return (
    <div className="flex items-start gap-3">
      {preset && (
        <span className="flex size-9 shrink-0 items-center justify-center rounded-md bg-raised/60 text-ink">
          <PresetIcon preset={preset} size={20} />
        </span>
      )}
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <h3 className="text-base font-semibold text-ink">{title}</h3>
        {sub && <p className="text-sm text-ink-3">{sub}</p>}
      </div>
      {onClose && (
        <button
          type="button"
          aria-label="Close"
          onClick={onClose}
          className="rounded p-1 text-ink-3 hover:bg-raised hover:text-ink"
        >
          <X size={15} aria-hidden />
        </button>
      )}
    </div>
  );
}

function Nav({
  onBack,
  next,
  busy,
  disabled,
}: {
  onBack?: () => void;
  next?: string;
  busy?: string | null;
  disabled?: boolean;
}) {
  return (
    <div className="mt-5 flex items-center gap-2">
      {onBack && (
        <button
          type="button"
          onClick={onBack}
          className="flex items-center gap-1 rounded-md px-2 py-1.5 text-sm text-ink-3 hover:bg-raised hover:text-ink"
        >
          <ArrowLeft size={14} aria-hidden /> Back
        </button>
      )}
      <span className="flex-1" />
      {next && (
        <button
          type="submit"
          disabled={!!busy || disabled}
          className="flex items-center gap-1.5 rounded-md bg-accent px-3.5 py-1.5 text-sm font-medium text-canvas transition-opacity hover:opacity-90 disabled:opacity-50"
        >
          {busy ? <LoaderCircle size={14} className="animate-spin" aria-hidden /> : null}
          {busy ?? next}
          {!busy && <ArrowRight size={14} aria-hidden />}
        </button>
      )}
    </div>
  );
}

function Problem({ children }: { children: ReactNode }) {
  return (
    <p role="alert" className="mt-3 flex items-start gap-2 text-sm text-danger">
      <CircleAlert size={14} className="mt-0.5 shrink-0" aria-hidden />
      <span>{children}</span>
    </p>
  );
}

function ExternalButton({ url, label }: { url: string; label: string }) {
  return (
    <button
      type="button"
      onClick={() => openExternal(url)}
      className="flex w-fit items-center gap-1 rounded-md border border-hairline-strong px-2.5 py-1 text-sm text-ink-2 transition-colors hover:border-accent/50 hover:text-ink"
    >
      {label}
      <ArrowUpRight size={13} aria-hidden />
    </button>
  );
}

const FIELD =
  "w-full rounded-md border border-hairline-strong bg-canvas px-3 py-2 text-sm text-ink outline-none placeholder:text-ink-4 focus:ring-2 focus:ring-accent/40";

function useAutofocus<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  useEffect(() => {
    const t = setTimeout(() => ref.current?.focus(), 250);
    return () => clearTimeout(t);
  }, []);
  return ref;
}

/** A form whose submit runs `action`, showing its error in plain words. */
function useAction(action: () => Promise<void>) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (err) {
      setError((err as Error).message);
      setBusy(false);
    }
  };
  return { busy, error, submit };
}

// ---- steps ---------------------------------------------------------------------

function ProviderStep({ onPick, onClose }: { onPick: (p: Preset) => void; onClose?: () => void }) {
  const group = (kinds: Preset["kind"][]) => PRESETS.filter((p) => kinds.includes(p.kind));
  return (
    <div className="flex flex-col gap-4">
      <CardHead
        title="Where should your model run?"
        sub="On your own machine, free and private; or with a provider, paid per use with an API key."
        onClose={onClose}
      />
      <TileGroup title="On your computer" presets={group(["local"])} onPick={onPick} />
      <TileGroup title="With a provider" presets={group(["cloud"])} onPick={onPick} />
      <TileGroup title="Something else" presets={group(["custom"])} onPick={onPick} />
    </div>
  );
}

function TileGroup({ title, presets, onPick }: { title: string; presets: Preset[]; onPick: (p: Preset) => void }) {
  return (
    <div className="flex flex-col gap-2">
      <h4 className="text-[11px] font-medium uppercase tracking-wide text-ink-4">{title}</h4>
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        {presets.map((p) => (
          <button
            key={p.id}
            type="button"
            onClick={() => onPick(p)}
            className="group flex items-start gap-3 rounded-lg border border-hairline bg-canvas px-3 py-2.5 text-left transition-colors hover:border-accent/50 hover:bg-raised/40"
          >
            <span className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-md bg-raised/60 text-ink-2 group-hover:text-ink">
              <PresetIcon preset={p} />
            </span>
            <span className="flex min-w-0 flex-1 flex-col">
              <span className="text-sm font-medium text-ink">{p.name}</span>
              <span className="text-xs text-ink-3">{p.blurb}</span>
            </span>
            <ChevronRight size={14} className="mt-1 shrink-0 text-ink-4 group-hover:text-accent" aria-hidden />
          </button>
        ))}
      </div>
    </div>
  );
}

function KeyStep({
  preset,
  hasSavedKey,
  value,
  onChange,
  onBack,
  onClose,
  verify,
}: {
  preset: Preset;
  hasSavedKey: boolean;
  value: string;
  onChange: (v: string) => void;
  onBack?: () => void;
  onClose?: () => void;
  verify: () => Promise<void>;
}) {
  const input = useAutofocus<HTMLInputElement>();
  const { busy, error, submit } = useAction(verify);
  return (
    <form onSubmit={submit} aria-label="API key step">
      <CardHead
        preset={preset}
        title={hasSavedKey ? `Your ${preset.name} key` : `Get an API key from ${preset.name}`}
        sub={
          hasSavedKey
            ? "A key is saved in your keychain. Paste a new one to replace it, or continue."
            : `Create a key on ${preset.name}'s site, copy it, and paste it here.`
        }
        onClose={onClose}
      />
      <div className="mt-4 flex flex-col gap-2">
        {!hasSavedKey && preset.keyUrl && (
          <ExternalButton url={preset.keyUrl} label={`Open ${new URL(preset.keyUrl).host}`} />
        )}
        <input
          ref={input}
          className={FIELD}
          type="password"
          autoComplete="off"
          aria-label="API key"
          placeholder={hasSavedKey ? "Leave empty to keep the saved key" : "Paste your key"}
          value={value}
          onChange={(e) => onChange(e.target.value)}
        />
        <span className="text-xs text-ink-4">Kept in your system keychain, never written to a file.</span>
      </div>
      {error && (
        <Problem>
          {error.includes("refused the key")
            ? `${preset.name} didn't accept that key. Check it was copied whole.`
            : error}
        </Problem>
      )}
      <Nav
        onBack={onBack}
        next="Continue"
        busy={busy ? "Checking the key…" : null}
        disabled={!hasSavedKey && !value.trim()}
      />
    </form>
  );
}

function InstallStep({
  preset,
  baseUrl,
  onBaseUrl,
  onContinue,
  onBack,
  onClose,
  probe,
  onRunning,
}: {
  preset: Preset;
  baseUrl: string;
  onBaseUrl: (v: string) => void;
  /** Skip waiting: the server may start later, or live where kizn can't see it yet. */
  onContinue: () => void;
  onBack?: () => void;
  onClose?: () => void;
  probe: () => Promise<string[]>;
  onRunning: (models: string[]) => void;
}) {
  const [found, setFound] = useState(false);
  const [patience, setPatience] = useState(false);
  useEffect(() => {
    const t = setTimeout(() => setPatience(true), 4000);
    return () => clearTimeout(t);
  }, []);
  const running = useRef(onRunning);
  running.current = onRunning;
  // Moves on by itself once the local server answers.
  useEffect(() => {
    let live = true;
    const tick = async () => {
      try {
        const list = await probe();
        if (!live) return;
        setFound(true);
        setTimeout(() => live && running.current(list), 700);
      } catch {
        if (live) timer = setTimeout(tick, 2000);
      }
    };
    let timer = setTimeout(tick, 300);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [probe]);
  return (
    <div>
      <CardHead preset={preset} title={`Install and start ${preset.name}`} sub={preset.blurb} onClose={onClose} />
      <ol className="mt-4 flex list-decimal flex-col gap-1 pl-5 text-sm text-ink-2">
        {preset.setup?.map((s) => (
          <li key={s}>{s}</li>
        ))}
      </ol>
      {preset.setupUrl && (
        <div className="mt-3">
          <ExternalButton url={preset.setupUrl} label={`Get ${preset.name}`} />
        </div>
      )}
      <p className="mt-4 flex items-center gap-2 text-sm" aria-live="polite">
        {found ? (
          <>
            <CircleCheck size={15} className="text-success" aria-hidden />{" "}
            <span className="text-ink">{preset.name} is running.</span>
          </>
        ) : (
          <>
            <LoaderCircle size={15} className="animate-spin text-ink-3" aria-hidden />
            <span className="text-ink-3">
              Waiting for {preset.name} at {baseUrl.replace(/\/v1\/?$/, "")}…
            </span>
          </>
        )}
      </p>
      <details className="mt-3 text-xs text-ink-3">
        <summary className="cursor-pointer select-none hover:text-ink">Running on another machine?</summary>
        <label className="mt-2 flex flex-col gap-1">
          Base URL
          <input className={FIELD} aria-label="Base URL" value={baseUrl} onChange={(e) => onBaseUrl(e.target.value)} />
        </label>
      </details>
      <div className="mt-5 flex items-center gap-2">
        {onBack && (
          <button
            type="button"
            onClick={onBack}
            className="flex items-center gap-1 rounded-md px-2 py-1.5 text-sm text-ink-3 hover:bg-raised hover:text-ink"
          >
            <ArrowLeft size={14} aria-hidden /> Back
          </button>
        )}
        <span className="flex-1" />
        {patience && !found && (
          <button
            type="button"
            onClick={onContinue}
            className="flex items-center gap-1 rounded-md px-3 py-1.5 text-sm text-ink-2 hover:bg-raised hover:text-ink"
          >
            Continue anyway <ArrowRight size={14} aria-hidden />
          </button>
        )}
      </div>
    </div>
  );
}

function AddressStep({
  baseUrl,
  keyValue,
  onBaseUrl,
  onKey,
  onBack,
  onClose,
  verify,
}: {
  baseUrl: string;
  keyValue: string;
  onBaseUrl: (v: string) => void;
  onKey: (v: string) => void;
  onBack?: () => void;
  onClose?: () => void;
  verify: () => Promise<void>;
}) {
  const input = useAutofocus<HTMLInputElement>();
  const { busy, error, submit } = useAction(verify);
  return (
    <form onSubmit={submit} aria-label="Address step">
      <CardHead
        title="Where is your server?"
        sub="Its OpenAI-compatible base URL, usually ending in /v1."
        onClose={onClose}
      />
      <div className="mt-4 flex flex-col gap-2">
        <input
          ref={input}
          className={FIELD}
          aria-label="Base URL"
          placeholder="http://my-server:8000/v1"
          value={baseUrl}
          onChange={(e) => onBaseUrl(e.target.value)}
        />
        <input
          className={FIELD}
          type="password"
          autoComplete="off"
          aria-label="API key"
          placeholder="API key, if it needs one"
          value={keyValue}
          onChange={(e) => onKey(e.target.value)}
        />
      </div>
      {error && <Problem>{error}</Problem>}
      <Nav
        onBack={onBack}
        next="Continue"
        busy={busy ? "Looking…" : null}
        disabled={!/^https?:\/\//.test(baseUrl.trim())}
      />
    </form>
  );
}

function ModelStep({
  preset,
  models,
  loadModels,
  value,
  onChange,
  baseUrl,
  onBaseUrl,
  onBack,
  onClose,
  save,
}: {
  preset: Preset;
  models: string[] | null;
  loadModels: () => Promise<void>;
  value: string;
  onChange: (v: string) => void;
  baseUrl: string;
  onBaseUrl: (v: string) => void;
  onBack: () => void;
  onClose?: () => void;
  save: () => Promise<void>;
}) {
  const input = useAutofocus<HTMLInputElement>();
  const { busy, error, submit } = useAction(save);
  const [listError, setListError] = useState<string | null>(null);
  useEffect(() => {
    if (models === null) loadModels().catch((e: Error) => setListError(e.message));
  }, [models, loadModels]);
  const count = models?.length ?? 0;
  return (
    <form onSubmit={submit} aria-label="Model step">
      <CardHead
        preset={preset}
        title="Pick a model"
        sub={
          count > 0
            ? `${preset.name} offers ${count} model${count === 1 ? "" : "s"}. Start typing to search.`
            : preset.id === "ollama"
              ? "Type the name of a model you've pulled (ollama pull <model>)."
              : "Type the model id."
        }
        onClose={onClose}
      />
      <div className="mt-4 flex flex-col gap-2">
        <input
          ref={input}
          className={FIELD}
          aria-label="Model"
          list="kizn-models"
          placeholder={preset.modelPlaceholder}
          value={value}
          onChange={(e) => onChange(e.target.value)}
        />
        <datalist id="kizn-models">
          {models?.map((m) => (
            <option key={m} value={m} />
          ))}
        </datalist>
        <p className="text-xs text-ink-4">
          A larger model writes better practice items; a smaller one is cheaper and faster. You can change it later.
        </p>
        {listError && <p className="text-xs text-warn">Couldn't list the models: {listError}</p>}
        {preset.kind !== "custom" && (
          <details className="text-xs text-ink-3">
            <summary className="cursor-pointer select-none hover:text-ink">Advanced</summary>
            <label className="mt-2 flex flex-col gap-1">
              Base URL
              <input
                className={FIELD}
                aria-label="Base URL"
                value={baseUrl}
                onChange={(e) => onBaseUrl(e.target.value)}
              />
            </label>
          </details>
        )}
      </div>
      {error && <Problem>{error}</Problem>}
      <Nav onBack={onBack} next="Test it" busy={busy ? "Saving…" : null} disabled={!value.trim()} />
    </form>
  );
}

function TestStep({
  preset,
  name,
  model,
  check,
  onChecked,
  fix,
  second,
  onReady,
  onClose,
}: {
  preset: Preset;
  name: string;
  model: string;
  check: CheckView | null;
  onChecked: (c: CheckView) => void;
  fix: (to: StepId) => void;
  second: StepId;
  onReady: () => void;
  onClose?: () => void;
}) {
  const [error, setError] = useState<string | null>(null);
  const [shown, setShown] = useState(check ? 3 : 0);
  const started = useRef(false);

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    call({ type: "test_provider", name })
      .then((v) => {
        const c = v.providers.find((p) => p.name === name)?.check;
        if (!c) throw new Error("the test didn't record a result");
        onChecked(c);
      })
      .catch((e: Error) => setError(e.message));
  }, [name, onChecked]);

  // Reveal the three results one by one once they're in.
  useEffect(() => {
    if (!check || shown >= 3) return;
    const t = setTimeout(() => setShown((n) => n + 1), 280);
    return () => clearTimeout(t);
  }, [check, shown]);

  const rows: { label: string; ok: boolean | null; detail: string }[] = [
    {
      label: "Reachable",
      ok: check ? check.reachable : null,
      detail: check?.reachable ? `${preset.name} answered` : (check?.summary ?? ""),
    },
    {
      label: "Structured output",
      ok: check ? check.structured !== "none" : null,
      detail: !check?.reachable
        ? ""
        : check.structured === "native"
          ? "follows a JSON schema"
          : check.structured === "prompted"
            ? "answers in JSON; kizn parses it"
            : "no JSON it could use",
    },
    {
      label: "Streaming",
      ok: check ? check.streaming : null,
      detail: !check?.reachable ? "" : check.streaming ? "streams replies" : "can't stream",
    },
  ];
  const settled = check !== null && shown >= 3;
  const canAuthor = check?.roles.includes("author") ?? false;

  // Passed: let the last tick land, then move on to Ready by itself.
  const ready = useRef(onReady);
  ready.current = onReady;
  useEffect(() => {
    if (!settled || !canAuthor) return;
    const t = setTimeout(() => ready.current(), 900);
    return () => clearTimeout(t);
  }, [settled, canAuthor]);
  const keyProblem = check && !check.reachable && /key/.test(check.summary);
  const fixTo: StepId | null = !check ? null : !check.reachable ? second : canAuthor ? null : "model";
  const fixLabel = !check
    ? ""
    : !check.reachable
      ? keyProblem
        ? "Fix the key"
        : preset.kind === "local"
          ? `Check ${preset.name}`
          : "Fix the address"
      : "Try another model";

  return (
    <div>
      <CardHead
        preset={preset}
        title={settled && canAuthor ? `${model} works` : `Checking ${model}`}
        onClose={onClose}
      />
      <ul className="mt-4 flex flex-col gap-2.5" aria-live="polite">
        {rows.map((r, i) => {
          const visible = settled || (check !== null && i < shown);
          const state = !visible ? "pending" : r.ok ? "ok" : "fail";
          return (
            <li key={r.label} className="flex items-center gap-3 text-sm">
              <span className="flex size-5 items-center justify-center">
                {state === "pending" && <LoaderCircle size={15} className="animate-spin text-ink-4" aria-hidden />}
                {state === "ok" && <CircleCheck size={16} className="text-success" aria-hidden />}
                {state === "fail" && <CircleAlert size={16} className="text-warn" aria-hidden />}
              </span>
              <span className="w-36 text-ink">{r.label}</span>
              <span className="text-ink-3">{visible ? r.detail : "…"}</span>
            </li>
          );
        })}
      </ul>
      {error && <Problem>{error}</Problem>}
      {settled && !canAuthor && (
        <p className="mt-4 text-sm text-ink-2">
          {check.reachable
            ? `kizn needs a model that can return structured output to write your practice items, and ${model} can't. Try another model.`
            : `${check.summary.charAt(0).toUpperCase()}${check.summary.slice(1)}.`}
        </p>
      )}
      <div className="mt-5 flex items-center gap-2">
        {settled && fixTo && (
          <button
            type="button"
            onClick={() => fix(fixTo)}
            className="flex items-center gap-1 rounded-md px-2 py-1.5 text-sm text-ink-3 hover:bg-raised hover:text-ink"
          >
            <ArrowLeft size={14} aria-hidden /> {fixLabel}
          </button>
        )}
        <span className="flex-1" />
      </div>
    </div>
  );
}

function ReadyStep({
  preset,
  model,
  check,
  onFinished,
  onStay,
}: {
  preset: Preset;
  model: string;
  check: CheckView;
  onFinished: () => void;
  onStay: () => void;
}) {
  const finish = useRef(onFinished);
  finish.current = onFinished;
  useEffect(() => {
    const t = setTimeout(() => finish.current(), READY_SECONDS * 1000);
    return () => clearTimeout(t);
  }, []);
  const tutors = check.roles.includes("tutor");
  return (
    <div className="flex flex-col items-center gap-4 py-4 text-center">
      <motion.span
        initial={{ scale: 0.6, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ type: "spring", stiffness: 420, damping: 22 }}
        className="flex size-12 items-center justify-center rounded-full bg-success/15 text-success"
      >
        <CircleCheck size={26} aria-hidden />
      </motion.span>
      <div className="flex flex-col gap-1">
        <h3 className="text-lg font-semibold text-ink">You're connected</h3>
        <p className="max-w-sm text-sm text-ink-3">
          kizn will use {model} on {preset.name} to write your practice items
          {tutors ? " and to answer your questions." : "."}
        </p>
      </div>
      <button
        type="button"
        onClick={onFinished}
        className="flex items-center gap-1.5 rounded-md bg-accent px-4 py-2 text-sm font-medium text-canvas transition-opacity hover:opacity-90"
      >
        Go to Today <ArrowRight size={14} aria-hidden />
      </button>
      <div className="flex w-48 flex-col items-center gap-1.5">
        <span className="h-1 w-full overflow-hidden rounded-full bg-raised">
          <motion.span
            className="block h-full rounded-full bg-accent/60"
            initial={{ width: "100%" }}
            animate={{ width: "0%" }}
            transition={{ duration: READY_SECONDS, ease: "linear" }}
          />
        </span>
        <button type="button" onClick={onStay} className="text-xs text-ink-4 hover:text-ink">
          Stay in Settings
        </button>
      </div>
    </div>
  );
}
