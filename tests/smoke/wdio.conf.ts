import { type ChildProcess, spawn } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

// The @smoke layer: a handful of scenarios in the real Tauri webview, through
// tauri-driver (WebKitWebDriver on Linux, msedgedriver on Windows). macOS has
// no WKWebView driver; it gets a manual check per release. `just smoke`
// builds the debug app first. Never a real model.

const root = resolve(import.meta.dirname, "../..");
const exe = process.platform === "win32" ? ".exe" : "";
const application = join(root, `target/debug/kizn${exe}`);
const dataDir = mkdtempSync(join(tmpdir(), "kizn-smoke-"));
let driver: ChildProcess | undefined;

// The app inherits these through tauri-driver and the native driver.
process.env.KIZN_DATA_DIR = dataDir;
process.env.KIZN_CONFIG_DIR = dataDir;
process.env.KIZN_SMOKE_DATA_DIR = dataDir;

export const config: WebdriverIO.Config = {
  runner: "local",
  hostname: "127.0.0.1",
  port: 4444,
  specs: [join(root, "features/**/*.feature")],
  maxInstances: 1,
  capabilities: [
    {
      // @ts-expect-error tauri-driver's own capability
      "tauri:options": { application },
    },
  ],
  logLevel: "warn",
  framework: "cucumber",
  reporters: ["spec"],
  cucumberOpts: {
    require: [join(import.meta.dirname, "steps/**/*.ts")],
    tags: process.env.KIZN_BDD_TAG ? `@smoke and @${process.env.KIZN_BDD_TAG.replace(/^@/, "")}` : "@smoke",
    timeout: 30_000,
    strict: true,
  },
  onPrepare: () => {
    const bin =
      process.env.TAURI_DRIVER ?? join(process.env.HOME ?? process.env.USERPROFILE ?? "", ".cargo/bin/tauri-driver");
    driver = spawn(bin, [], { stdio: ["ignore", "inherit", "inherit"] });
  },
  onComplete: () => {
    driver?.kill();
  },
};
