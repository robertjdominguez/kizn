import { expect } from "@playwright/test";
import { seeProviderWorks } from "./connect_model.steps";
import { bridge, Given, Then, When } from "./fixtures";

Given("a fresh data directory", async ({ request, world }) => {
  const res = await request.post(`${bridge()}/test/fresh`);
  expect(res.ok(), "the bridge needs its `test` feature").toBeTruthy();
  world.dataDir = (await res.json()).data_dir;
});

When("I open kizn", async ({ page }) => {
  await page.goto("/");
});

Then("I see {string}", async ({ page, world }, text: string) => {
  if (text === "works") return seeProviderWorks(page, world);
  await expect(page.getByText(text, { exact: true })).toBeVisible();
});
