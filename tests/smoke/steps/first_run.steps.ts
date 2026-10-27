import { readdirSync } from "node:fs";
import { Given, Then, When } from "@wdio/cucumber-framework";
import { $, expect } from "@wdio/globals";

// The app was started on an empty temp directory (wdio.conf.ts); here we
// check it was empty apart from what kizn itself created.
Given("a fresh data directory", () => {
  const dir = process.env.KIZN_SMOKE_DATA_DIR;
  if (!dir) throw new Error("KIZN_SMOKE_DATA_DIR isn't set (wdio.conf.ts sets it)");
  const unexpected = readdirSync(dir).filter((f) => !f.startsWith("kizn.db"));
  if (unexpected.length) throw new Error(`data directory isn't fresh: ${unexpected.join(", ")}`);
});

When("I open kizn", async () => {
  // tauri-driver starts the app with the session; wait for the UI to render.
  await $("main").waitForExist({ timeout: 15_000 });
});

// On a cold runner, the first launch and Today's staged entrance can take a
// while: wait as long as opening the app may.
const SHOWN = { wait: 15_000 };

Then("I see {string}", async (text: string) => {
  await expect($(`//*[normalize-space(text())="${text}"]`)).toBeDisplayed(SHOWN);
});

Then("no model is configured", async () => {
  await expect($(`//*[normalize-space(text())="No model connected"]`)).toBeDisplayed(SHOWN);
});
