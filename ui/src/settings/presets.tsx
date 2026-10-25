import { Server, Sparkles } from "lucide-react";
import { siAnthropic, siGooglegemini, siLmstudio, siOllama, siOpenrouter } from "simple-icons";
import type { ProviderView } from "../bindings/ProviderView";

// The providers Settings offers up front. A preset only fills in the form
// (protocol, URL, where to get a key); whether a provider works is still
// decided by its connection test in kizn-app.
//
// Logos are Simple Icons (CC0), drawn in one colour so they sit in the
// theme. OpenAI asked Simple Icons to remove its mark, so it gets a neutral
// icon with its name.

export type Preset = {
  id: string;
  name: string;
  protocol: "openai" | "anthropic";
  baseUrl: string;
  kind: "local" | "cloud" | "custom";
  blurb: string;
  /** Where to get an API key (cloud). */
  keyUrl?: string;
  /** Where to get the software, and what to do first (local). */
  setupUrl?: string;
  setup?: string[];
  modelPlaceholder: string;
  icon: { path: string } | "openai" | "custom";
};

export const PRESETS: Preset[] = [
  {
    id: "ollama",
    name: "Ollama",
    protocol: "openai",
    baseUrl: "http://localhost:11434/v1",
    kind: "local",
    blurb: "Free. Runs open models on your own computer or a machine on your network.",
    setupUrl: "https://ollama.com/download",
    setup: ["Install Ollama and start it.", "Download a model in a terminal: ollama pull <model>."],
    modelPlaceholder: "a model you've pulled",
    icon: siOllama,
  },
  {
    id: "lmstudio",
    name: "LM Studio",
    protocol: "openai",
    baseUrl: "http://localhost:1234/v1",
    kind: "local",
    blurb: "Free. A desktop app for downloading and running open models.",
    setupUrl: "https://lmstudio.ai",
    setup: ["Install LM Studio and download a model.", "Start its local server (the Developer tab)."],
    modelPlaceholder: "a model you've downloaded",
    icon: siLmstudio,
  },
  {
    id: "openai",
    name: "OpenAI",
    protocol: "openai",
    baseUrl: "https://api.openai.com/v1",
    kind: "cloud",
    blurb: "OpenAI's models, paid per use with an API key.",
    keyUrl: "https://platform.openai.com/api-keys",
    modelPlaceholder: "pick one of your models",
    icon: "openai",
  },
  {
    id: "anthropic",
    name: "Anthropic",
    protocol: "anthropic",
    baseUrl: "https://api.anthropic.com",
    kind: "cloud",
    blurb: "Claude models, paid per use with an API key.",
    keyUrl: "https://console.anthropic.com/settings/keys",
    modelPlaceholder: "pick one of your models",
    icon: siAnthropic,
  },
  {
    id: "gemini",
    name: "Google Gemini",
    protocol: "openai",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai",
    kind: "cloud",
    blurb: "Google's Gemini models, with an API key from Google AI Studio.",
    keyUrl: "https://aistudio.google.com/apikey",
    modelPlaceholder: "pick one of your models",
    icon: siGooglegemini,
  },
  {
    id: "openrouter",
    name: "OpenRouter",
    protocol: "openai",
    baseUrl: "https://openrouter.ai/api/v1",
    kind: "cloud",
    blurb: "One key for models from many labs, paid per use.",
    keyUrl: "https://openrouter.ai/keys",
    modelPlaceholder: "search hundreds of models",
    icon: siOpenrouter,
  },
  {
    id: "custom",
    name: "Other server",
    protocol: "openai",
    baseUrl: "",
    kind: "custom",
    blurb: "Anything OpenAI-compatible: vLLM, llama.cpp, a gateway, a box on your network.",
    modelPlaceholder: "the model id the server expects",
    icon: "custom",
  },
];

export const presetFor = (p: ProviderView): Preset =>
  PRESETS.find((x) => x.id === p.preset) ??
  PRESETS.find((x) => x.kind !== "custom" && x.baseUrl === p.base_url) ??
  (p.protocol === "anthropic" ? (PRESETS.find((x) => x.id === "anthropic") as Preset) : (PRESETS.at(-1) as Preset));

export function PresetIcon({
  preset,
  size = 18,
  className = "",
}: {
  preset: Preset;
  size?: number;
  className?: string;
}) {
  if (preset.icon === "openai") return <Sparkles size={size} className={className} aria-hidden />;
  if (preset.icon === "custom") return <Server size={size} className={className} aria-hidden />;
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} fill="currentColor" className={className} aria-hidden>
      <path d={preset.icon.path} />
    </svg>
  );
}
