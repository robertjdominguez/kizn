import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { defineConfig, devices } from "@playwright/test";
import { defineBddConfig } from "playwright-bdd";

// The @ui layer: the same feature files as @domain, driven through the UI in
// Chromium against the dev bridge (built with its `test` feature) and Vite.
// Ports differ from `just dev` so both can run at once. Never a real model.

const root = resolve(import.meta.dirname, "../..");
const bridgePort = process.env.KIZN_TEST_BRIDGE_PORT ?? "7879";
const vitePort = process.env.KIZN_TEST_VITE_PORT ?? "1421";
const dataDir = mkdtempSync(join(tmpdir(), "kizn-ui-"));
const exe = process.platform === "win32" ? ".exe" : "";
const tag = process.env.KIZN_BDD_TAG ? ` and @${process.env.KIZN_BDD_TAG.replace(/^@/, "")}` : "";

process.env.KIZN_TEST_BRIDGE = `http://127.0.0.1:${bridgePort}`;

const testDir = defineBddConfig({
  features: join(root, "features/**/*.feature"),
  steps: "steps/**/*.ts",
  featuresRoot: join(root, "features"),
  tags: `@ui${tag}`,
});

export default defineConfig({
  testDir,
  fullyParallel: false,
  workers: 1,
  forbidOnly: !!process.env.CI,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: { baseURL: `http://127.0.0.1:${vitePort}`, trace: "retain-on-failure" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: [
    {
      command: join(root, `target/debug/kizn-bridge${exe}`),
      env: { KIZN_DATA_DIR: dataDir, KIZN_CONFIG_DIR: dataDir, KIZN_BRIDGE_PORT: bridgePort },
      // /rpc only takes POST (a GET is a 405, which Playwright doesn't count
      // as up), so wait for the port instead.
      port: Number(bridgePort),
      reuseExistingServer: false,
      stdout: "pipe",
    },
    {
      command: "bun run dev",
      cwd: join(root, "ui"),
      env: { KIZN_BRIDGE_PORT: bridgePort, KIZN_VITE_PORT: vitePort },
      url: `http://127.0.0.1:${vitePort}`,
      reuseExistingServer: false,
    },
  ],
});
