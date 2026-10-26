import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test as base, createBdd } from "playwright-bdd";

/** What a scenario remembers between steps. */
export type World = {
  dataDir?: string;
  /** The provider the scenario is working with. */
  provider?: string;
  model?: string;
  /** A port nothing listens on, for "nothing answered". */
  closedPort?: number;
  goal?: string;
};

export const test = base.extend<{ world: World }>({
  // biome-ignore lint/correctness/noEmptyPattern: Playwright's fixture signature
  world: async ({}, use) => use({}),
});
export const { Given, When, Then } = createBdd(test);

/** The bridge under test, for test-only endpoints. */
export const bridge = () => {
  const url = process.env.KIZN_TEST_BRIDGE;
  if (!url) throw new Error("KIZN_TEST_BRIDGE isn't set (playwright.config.ts sets it)");
  return url;
};

/** The workspace version, read from Cargo.toml: what About should show. */
export const workspaceVersion = () => {
  const toml = readFileSync(resolve(import.meta.dirname, "../../../Cargo.toml"), "utf8");
  const m = toml.match(/\[workspace\.package\][^[]*?\nversion\s*=\s*"([^"]+)"/);
  if (!m?.[1]) throw new Error("no [workspace.package] version in Cargo.toml");
  return m[1];
};
