import { expect } from "@playwright/test";
import { bridge, Given, Then, When } from "./fixtures";

Given("a fresh data directory", async ({ request }) => {
  const res = await request.post(`${bridge()}/test/fresh`);
  expect(res.ok(), "the bridge needs its `test` feature").toBeTruthy();
});

When("I open kizn", async ({ page }) => {
  await page.goto("/");
});

Then("I see {string}", async ({ page }, text: string) => {
  await expect(page.getByText(text, { exact: true })).toBeVisible();
});

Then("no model is configured", async ({ page }) => {
  await expect(page.getByText("No model connected")).toBeVisible();
});
