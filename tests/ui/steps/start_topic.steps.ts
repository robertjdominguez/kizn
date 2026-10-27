import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, type Page } from "@playwright/test";
import { bridge, Given, Then, When, type World } from "./fixtures";

// Start a topic, through the setup cards and the topic page. The author is
// the fake model server's slow behaviour (the test bridge sends every
// non-local model URL to it), so drafting and writing unfold on screen.

function writeConfig(world: World, author: string) {
  if (!world.dataDir) throw new Error("no data directory");
  writeFileSync(
    join(world.dataDir, "config.toml"),
    [
      "[providers.author]",
      'protocol = "openai"',
      `base_url = "http://models.test/${author}/v1"`,
      'model = "m"',
      "",
      "[providers.checker]",
      'protocol = "openai"',
      'base_url = "http://models.test/works/v1"',
      'model = "m"',
      "",
      "[roles]",
      'author = "author"',
      'checker = "checker"',
      'tutor = "checker"',
      "",
    ].join("\n"),
  );
}

Given("a configured author model", async ({ page, world }) => {
  const res = await page.request.post(`${bridge()}/test/fresh`);
  expect(res.ok(), "the bridge needs its `test` feature").toBeTruthy();
  world.dataDir = (await res.json()).data_dir;
  writeConfig(world, "slow");
});

async function startTopic(page: Page, world: World, title: string, goal: string) {
  world.goal = goal;
  await page.goto("/#/new-topic");
  await page.getByLabel("Name it").fill(title);
  await page.getByLabel("What do you want to be able to do, and why?").fill(goal);
  await page.getByRole("button", { name: "Continue" }).click();
  // Drafting takes a few seconds with the slow author, then Shape follows.
  await expect(page.getByRole("heading", { name: "Shape it" })).toBeVisible({ timeout: 30_000 });
}

When("I start a topic {string} with the goal {string}", async ({ page, world }, title: string, goal: string) => {
  await startTopic(page, world, title, goal);
});

Given("a drafted topic", async ({ page, world }) => {
  await startTopic(page, world, "Container fundamentals", "explain to customers why deployments behave differently");
});

const outcomeFields = (page: Page) => page.getByRole("textbox", { name: /^Outcome \d+$/ });

Then("I see {int} to {int} drafted outcomes, each starting with a verb", async ({ page }, min: number, max: number) => {
  const fields = outcomeFields(page);
  const n = await fields.count();
  expect(n).toBeGreaterThanOrEqual(min);
  expect(n).toBeLessThanOrEqual(max);
  for (const text of await fields.evaluateAll((els) => els.map((e) => (e as HTMLTextAreaElement).value))) {
    expect(text, "starts with a capitalised word").toMatch(/^[A-Z][a-z]+\b/);
    expect(text).not.toMatch(/^(The|A|An|Understanding|Knowledge|Introduction)\b/);
  }
});

Then("each outcome has a target with a reason that quotes my goal", async ({ page, world }) => {
  const n = await outcomeFields(page).count();
  const items = page.getByRole("listitem").filter({ has: page.getByRole("textbox", { name: /^Outcome \d+$/ }) });
  await expect(items).toHaveCount(n);
  for (let i = 0; i < n; i++) {
    const item = items.nth(i);
    await expect(item.getByRole("combobox")).not.toHaveValue("");
    await expect(item).toContainText(world.goal ?? "");
  }
});

Then("each outcome has {int} to {int} concepts", async ({ page }, min: number, max: number) => {
  const n = await outcomeFields(page).count();
  for (let i = 1; i <= n; i++) {
    // A concept is a one-line button until clicked, then a textbox.
    const concepts = await page.getByRole("button", { name: new RegExp(`^Outcome ${i}, concept \\d+:`) }).count();
    expect(concepts, `outcome ${i}`).toBeGreaterThanOrEqual(min);
    expect(concepts, `outcome ${i}`).toBeLessThanOrEqual(max);
  }
});

When("I rename a concept, delete an outcome and change a target to {string}", async ({ page }, target: string) => {
  await page.getByRole("button", { name: /^Outcome 1, concept 1:/ }).click();
  await page.getByRole("textbox", { name: "Outcome 1, concept 1" }).fill("An image is immutable layers plus config");
  await page.keyboard.press("Enter");
  const n = await outcomeFields(page).count();
  await page.getByRole("button", { name: `Remove outcome ${n}` }).click();
  await expect(outcomeFields(page)).toHaveCount(n - 1);
  const first = page.getByRole("listitem").filter({ has: page.getByRole("textbox", { name: "Outcome 1" }) });
  await first.getByRole("combobox").selectOption({ label: target });
  await expect(page.getByText("Saved")).toBeVisible({ timeout: 5000 });
});

When("I confirm the topic", async ({ page }) => {
  await page.getByRole("button", { name: "Continue" }).click();
  await expect(page.getByRole("heading", { name: "Ready to prepare your practice?" })).toBeVisible();
  await page.getByRole("button", { name: "Confirm" }).click();
  await expect(page.getByRole("heading", { name: "Your concepts" })).toBeVisible();
});

Then("bank writing is queued for each outcome", async ({ page }) => {
  await expect(page.getByText("Questions ready")).toBeVisible();
});

Then("the topic page says its questions are being written", async ({ page }) => {
  await expect(page.getByRole("region", { name: "Next up" })).toContainText(/Writing your first questions|ready/i);
  await expect(page.getByText("Questions ready")).toBeVisible();
});

Given("a confirmed topic whose bank is being written", async ({ page, world }) => {
  await startTopic(page, world, "Container fundamentals", "explain to customers why deployments behave differently");
  await page.getByRole("button", { name: "Continue" }).click();
  await page.getByRole("button", { name: "Confirm" }).click();
  await expect(page.getByRole("heading", { name: "Your concepts" })).toBeVisible();
});

When("I open Today", async ({ page }) => {
  await page.goto("/#/");
});

Then("I see the topic as preparing", async ({ page }) => {
  const card = page.getByRole("link", { name: /Container fundamentals/ });
  await expect(card.getByRole("progressbar", { name: "Questions being written" })).toBeVisible();
});

When("every outcome has items", async ({ world }) => {
  // Speed the rest up: the writer picks the author per outcome.
  writeConfig(world, "works");
});

Then("I see the topic as ready", async ({ page }) => {
  const card = page.getByRole("link", { name: /Container fundamentals/ });
  await expect(card).toContainText("Ready to start", { timeout: 90_000 });
  await expect(card.getByRole("progressbar")).toHaveCount(0, { timeout: 90_000 });
});
