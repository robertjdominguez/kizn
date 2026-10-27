import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { expect, type Page, type Response } from "@playwright/test";
import type { DeckView } from "../../../ui/src/bindings/DeckView";
import { bridge, Given, Then, When, type World } from "./fixtures";

// Practice, study and coming back, in the browser. The topic is seeded
// (topic-with-bank); which option is right is read from the scenario's own
// database, since the deck never carries the key.

const TOPIC = "01SEEDTOPIC00000000000000T";

type PracticeWorld = World & { deck?: DeckView; answered?: number };

type StoredOption = { text: string; correct: boolean; why: string };

/** An item's stored options, read from the scenario's database. */
function optionsOf(world: World, itemId: string): StoredOption[] {
  if (!world.dataDir) throw new Error("no data directory");
  const db = new DatabaseSync(join(world.dataDir, "kizn.db"), { readOnly: true });
  try {
    const row = db.prepare("SELECT options_json FROM items WHERE id = ?").get(itemId) as { options_json: string };
    return JSON.parse(row.options_json) as StoredOption[];
  } finally {
    db.close();
  }
}

/** The stored index of an item's correct option. */
const keyOf = (world: World, itemId: string) => optionsOf(world, itemId).findIndex((o) => o.correct);

async function seedTopic(page: Page) {
  const res = await page.request.post(`${bridge()}/test/seed`, { data: { fixture: "topic-with-bank" } });
  expect(res.ok(), await res.text()).toBeTruthy();
}

/** Opens practice and keeps the deck it was given: the last one, since in
 *  dev React runs the effect that asks for it twice. Which cards it holds is
 *  the same either time; only the order of each card's options differs. */
async function openPractice(page: Page, world: PracticeWorld, hash: string) {
  world.deck = undefined;
  world.answered = 0;
  const keep = async (r: Response) => {
    if (r.url().endsWith("/rpc") && r.request().postData()?.includes("start_practice")) {
      world.deck = ((await r.json()) as { data: DeckView }).data;
    }
  };
  page.on("response", keep);
  await page.goto(hash);
  await expect.poll(() => world.deck !== undefined).toBe(true);
  await page.waitForTimeout(400);
  page.off("response", keep);
}

async function skipIntro(page: Page) {
  const skip = page.getByRole("button", { name: "Skip" });
  if (await skip.isVisible().catch(() => false)) await skip.click();
  await expect(page.getByRole("region", { name: "Question" })).toBeVisible();
}

/** Answers the current card right or wrong, and moves on. */
async function answer(page: Page, world: PracticeWorld, right: boolean, moveOn = true) {
  const i = world.answered ?? 0;
  const card = world.deck?.cards[i];
  if (!card) throw new Error(`no card ${i}`);
  // The option to pick, by its text: the deck's shuffle may not be the one shown.
  const choice = optionsOf(world, card.item_id).find((o) => o.correct === right);
  if (!choice) throw new Error(`card ${i} has no ${right ? "right" : "wrong"} option`);
  const question = page.getByRole("region", { name: "Question" });
  await expect(question).toContainText(card.stem);
  await question.locator("ol > li button", { hasText: choice.text }).click();
  await expect(page.getByText(right ? "Right." : "Not this time.")).toBeVisible();
  world.answered = i + 1;
  if (moveOn) await page.getByRole("button", { name: /^(Next|Finish)/ }).click();
}

async function answerAll(page: Page, world: PracticeWorld, right: (i: number) => boolean) {
  const n = world.deck?.cards.length ?? 0;
  for (let i = world.answered ?? 0; i < n; i++) await answer(page, world, right(i));
  await expect(page.getByRole("region", { name: "Summary" })).toBeVisible();
}

const summary = (page: Page) => page.getByRole("region", { name: "Summary" });
const group = (page: Page, title: string) =>
  summary(page)
    .locator("div", { has: page.getByRole("heading", { name: title, exact: true }) })
    .locator("li");

// ---- first practice ------------------------------------------------------------

Given("a topic whose bank has recall questions for 5 unseen concepts", async ({ page }) => {
  await seedTopic(page);
});

Given("I'm practising a topic", async ({ page, world }) => {
  await seedTopic(page);
  await openPractice(page, world, `/#/practice/${TOPIC}`);
  await skipIntro(page);
});

When("I start practice", async ({ page, world }) => {
  await openPractice(page, world, `/#/practice/${TOPIC}`);
});

Then("I get {int} recall cards, one per concept", async ({ page, world }, n: number) => {
  const deck = (world as PracticeWorld).deck;
  expect(deck?.cards).toHaveLength(n);
  expect(new Set(deck?.cards.map((c) => c.concept)).size).toBe(n);
  await skipIntro(page);
  await expect(page.getByText(`1 of ${n}`)).toBeVisible();
  await expect(page.getByText("Starting point", { exact: true })).toBeVisible();
});

Then("the correct option's position varies between cards", async ({ world }) => {
  const deck = (world as PracticeWorld).deck;
  const positions = new Set(deck?.cards.map((c) => c.order.indexOf(keyOf(world, c.item_id))));
  expect(positions.size).toBeGreaterThan(1);
});

Then("I first see how concepts move from not started to stuck", async ({ page }) => {
  await expect(page.getByRole("heading", { name: "Concepts move from new to stuck" })).toBeVisible();
  for (const word of ["Not started", "Seen", "Recalled", "Stuck"]) {
    await expect(page.getByText(word, { exact: true })).toBeVisible();
  }
});

Then("that the starting point is about finding where I am", async ({ page }) => {
  await page.getByRole("button", { name: /Next/ }).click();
  await expect(page.getByRole("heading", { name: "First, find where you're starting" })).toBeVisible();
  await expect(page.getByText(/Misses are fine/)).toBeVisible();
});

When("I begin", async ({ page }) => {
  await page.getByRole("button", { name: /Begin/ }).click();
});

Then("I get the first card", async ({ page }) => {
  await expect(page.getByRole("region", { name: "Question" })).toBeVisible();
  await expect(page.getByText("1 of 5")).toBeVisible();
});

When("I tap a wrong option", async ({ page, world }) => {
  await answer(page, world as PracticeWorld, false, false);
});

Then("I see that it is wrong, every option's why, and the source", async ({ page, world }) => {
  const card = (world as PracticeWorld).deck?.cards[0];
  if (!card) throw new Error("no card");
  for (const o of optionsOf(world, card.item_id)) {
    await expect(page.getByText(o.why, { exact: true })).toBeVisible();
  }
  await expect(page.getByRole("region", { name: "Question" }).getByText("your pick", { exact: true })).toBeVisible();
  await expect(page.getByText(/common mix-up/)).toBeVisible();
  await expect(page.getByText("From the model's own knowledge, not a cited source")).toBeVisible();
});

Then("the concept is {string}", async ({ page }, state: string) => {
  await expect(page.getByText(new RegExp(`: ${state}$`))).toBeVisible();
});

When("I finish the set", async ({ page, world }) => {
  await answerAll(page, world as PracticeWorld, (i) => i % 2 === 0);
});

Then("I see what held, what slipped, the next due date", async ({ page }) => {
  await expect(group(page, "You know these")).toHaveCount(3);
  await expect(group(page, "Learn these first")).toHaveCount(2);
  await expect(summary(page)).toContainText("Next recall: tomorrow");
});

Then("each outcome's count and what's next for it", async ({ page }) => {
  await expect(summary(page)).toContainText("Outcome 1 · 5 concepts · 3 recalled");
});

Then('the {int} I got right are under "you know these"', async ({ page }, n: number) => {
  await expect(group(page, "You know these")).toHaveCount(n);
});

Then('the {int} I missed are under "learn these first"', async ({ page }, n: number) => {
  await expect(group(page, "Learn these first")).toHaveCount(n);
});

Then("it's my starting point, with the next recall tomorrow", async ({ page }) => {
  await expect(summary(page).getByRole("heading", { name: "Where you're starting" })).toBeVisible();
  await expect(summary(page)).toContainText("Next recall: tomorrow");
});

Given("I finished my first set today", async ({ page, world }) => {
  await seedTopic(page);
  await openPractice(page, world, `/#/practice/${TOPIC}`);
  await skipIntro(page);
  await answerAll(page, world as PracticeWorld, () => true);
});

Then("I see when my next recalls are due", async ({ page }) => {
  await expect(page.getByRole("heading", { name: "You're all caught up" })).toBeVisible();
  await expect(page.getByText(/Your next recall is in 3 days/)).toBeVisible();
});

// ---- learn what you missed -------------------------------------------------------

Given("I missed a concept in my starting point", async ({ page, world }) => {
  await seedTopic(page);
  await openPractice(page, world, `/#/practice/${TOPIC}`);
  await skipIntro(page);
  await answer(page, world as PracticeWorld, false);
});

Given("I answer the rest of the set right", async ({ page, world }) => {
  await answerAll(page, world as PracticeWorld, () => true);
});

/** From the summary: the first concept to learn. */
async function openMissed(page: Page, world: PracticeWorld) {
  await answerAllIfNeeded(page, world);
  await group(page, "Learn these first").first().getByRole("button").click();
  await expect(page.getByRole("dialog")).toBeVisible();
}

async function answerAllIfNeeded(page: Page, world: PracticeWorld) {
  if (
    !(await summary(page)
      .isVisible()
      .catch(() => false))
  )
    await answerAll(page, world, () => true);
}

When("I open it", async ({ page, world }) => {
  await openMissed(page, world as PracticeWorld);
});

Then("I see the sources kizn fetched and could read", async ({ page }) => {
  const sheet = page.getByRole("dialog");
  await expect(sheet.getByText("docs.example.org")).toBeVisible({ timeout: 15_000 });
  await expect(sheet.getByText("blog.example.net")).toBeVisible();
});

Then("none it couldn't", async ({ page }) => {
  const sheet = page.getByRole("dialog");
  await expect(sheet.getByText("learn.example.com")).toHaveCount(0);
  await expect(sheet.getByText("old.example.org")).toHaveCount(0);
});

When("I open it and mark it studied", async ({ page, world }) => {
  await openMissed(page, world as PracticeWorld);
  await page.getByRole("dialog").getByRole("button", { name: "Mark studied" }).click();
  await expect(page.getByRole("dialog").getByText("Studied")).toBeVisible();
  await expect(page.getByRole("dialog").getByText("Comes back tomorrow either way")).toBeVisible();
});

Then("the summary shows it studied", async ({ page }) => {
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(group(page, "Learn these first").first().getByLabel("Studied")).toBeVisible();
});

Then("it still comes back as recall tomorrow", async ({ page }) => {
  await expect(summary(page)).toContainText("Next recall: tomorrow");
});

When("I open Today after the set", async ({ page }) => {
  await page.getByRole("link", { name: "Back to Today" }).click();
});

Then("Today's next step is to read up on {int} concept", async ({ page }, n: number) => {
  await expect(page.getByRole("heading", { name: `Read up on the ${n} concept you missed` })).toBeVisible();
  await expect(page.getByRole("button", { name: /Start reading/ })).toBeVisible();
});

Then("this week shows today as practised", async ({ page }) => {
  await expect(page.getByRole("region", { name: "This week" }).getByLabel(/: practised$/)).toHaveCount(1);
});

// ---- coming back ------------------------------------------------------------------

When("I find my starting point, missing {int}", async ({ page, world }, misses: number) => {
  await openPractice(page, world, `/#/practice/${TOPIC}`);
  await skipIntro(page);
  await answerAll(page, world as PracticeWorld, (i) => i >= misses);
});

When("I read up on what I missed", async ({ page }) => {
  const rows = group(page, "Learn these first");
  const n = await rows.count();
  for (let i = 0; i < n; i++) {
    await rows.nth(i).getByRole("button").click();
    const sheet = page.getByRole("dialog");
    await expect(sheet.getByText("docs.example.org")).toBeVisible({ timeout: 15_000 });
    await sheet.getByRole("button", { name: "Mark studied" }).click();
    await expect(sheet.getByText("Studied")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toHaveCount(0);
  }
});

Then("Today says I'm all caught up", async ({ page }) => {
  await page.goto("/#/");
  await expect(page.getByRole("heading", { name: "You're all caught up" })).toBeVisible();
  await expect(page.getByText(/Your next recall is tomorrow/)).toBeVisible();
});

async function later(page: Page, days: number) {
  const res = await page.request.post(`${bridge()}/test/later`, { data: { days } });
  expect(res.ok()).toBeTruthy();
}

When("{int} day passes", async ({ page }, days: number) => {
  await later(page, days);
});

When("{int} more days pass", async ({ page }, days: number) => {
  await later(page, days);
});

Then("Today has {int} recalls waiting", async ({ page }, n: number) => {
  await page.goto("/#/settings");
  await page.goto("/#/");
  await expect(page.getByRole("heading", { name: `${n} recalls are waiting` })).toBeVisible();
});

When("I practise everything right", async ({ page, world }) => {
  await openPractice(page, world, "/#/practice");
  await skipIntro(page);
  await answerAll(page, world as PracticeWorld, () => true);
});

Then("the topic shows {int} concepts stuck", async ({ page }, n: number) => {
  await page.goto(`/#/topic/${TOPIC}`);
  await expect(page.getByRole("complementary", { name: "About this topic" })).toContainText(
    new RegExp(`${n}\\s*stuck`),
  );
});
