import { test as base, createBdd } from "playwright-bdd";

export const test = base;
export const { Given, When, Then } = createBdd(test);

/** The bridge under test, for test-only endpoints. */
export const bridge = () => {
  const url = process.env.KIZN_TEST_BRIDGE;
  if (!url) throw new Error("KIZN_TEST_BRIDGE isn't set (playwright.config.ts sets it)");
  return url;
};
