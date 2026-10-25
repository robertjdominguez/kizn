import { expect } from "@playwright/test";
import { Then, When, workspaceVersion } from "./fixtures";

When("I open About", async ({ page }) => {
  await page.getByRole("button", { name: "About" }).click();
});

Then("I see the version of kizn I'm running", async ({ page }) => {
  await expect(page.getByRole("dialog")).toContainText(`kizn ${workspaceVersion()}`);
});

Then("I see where my data is kept", async ({ page, world }) => {
  if (!world.dataDir) throw new Error('no data directory: start with "Given a fresh data directory"');
  await expect(page.getByRole("dialog")).toContainText(world.dataDir);
});
