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
  await expect(page.getByRole("heading", { name: "Read up on what you missed" })).toBeVisible();
  await expect(page.getByText(`${n} concept still to read`)).toBeVisible();
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

// ---- this key is wrong -----------------------------------------------------------

/** One row from the scenario's database. */
function row<T>(world: World, sql: string, ...args: string[]): T {
  if (!world.dataDir) throw new Error("no data directory");
  const db = new DatabaseSync(join(world.dataDir, "kizn.db"), { readOnly: true });
  try {
    return db.prepare(sql).get(...args) as T;
  } finally {
    db.close();
  }
}

Given("I missed a card", async ({ page, world }) => {
  await seedTopic(page);
  await openPractice(page, world, `/#/practice/${TOPIC}`);
  await skipIntro(page);
  await answer(page, world as PracticeWorld, false, false);
});

When("I say {string}", async ({ page }, what: string) => {
  await page.getByRole("button", { name: what }).click();
});

When("I say {string} and confirm", async ({ page }, what: string) => {
  await page.getByRole("button", { name: what }).click();
  await page.getByRole("group", { name: "Look again" }).getByRole("button", { name: "Yes, set it aside" }).click();
});

Then("that answer moves nothing", async ({ page }) => {
  await expect(page.getByText(/^Set aside\./)).toBeVisible();
  await expect(page.getByText(/: not started$/)).toBeVisible();
});

Then("the question never appears again", async ({ world }) => {
  const card = (world as PracticeWorld).deck?.cards[0];
  if (!card) throw new Error("no card");
  await expect
    .poll(() => row<{ status: string }>(world, "SELECT status FROM items WHERE id = ?", card.item_id).status)
    .toBe("disputed");
});

Then("a replacement is written for its slot", async ({ world }) => {
  const card = (world as PracticeWorld).deck?.cards[0];
  if (!card) throw new Error("no card");
  const item = row<{ outcome_id: string; concept_ids_json: string }>(
    world,
    "SELECT outcome_id, concept_ids_json FROM items WHERE id = ?",
    card.item_id,
  );
  const concept = (JSON.parse(item.concept_ids_json) as string[])[0] ?? "";
  // The fixture held two; the disputed one leaves one until the writer refills.
  await expect
    .poll(
      () =>
        row<{ n: number }>(
          world,
          "SELECT count(*) AS n FROM items WHERE outcome_id = ? AND kind = 'recall' AND status = 'active' AND json_extract(concept_ids_json, '$[0]') = ?",
          item.outcome_id,
          concept,
        ).n,
      { timeout: 60_000 },
    )
    .toBeGreaterThanOrEqual(2);
});

Then("I see kizn's reason for its key first", async ({ page, world }) => {
  const card = (world as PracticeWorld).deck?.cards[0];
  if (!card) throw new Error("no card");
  const key = optionsOf(world, card.item_id).find((o) => o.correct);
  const look = page.getByRole("group", { name: "Look again" });
  await expect(look).toContainText("kizn's key is");
  await expect(look).toContainText(key?.why ?? "");
  await expect(look).toContainText("Still think the key is wrong?");
});

When("I keep the question", async ({ page }) => {
  await page.getByRole("button", { name: "Keep it" }).click();
});

Then("my answer still counts", async ({ page, world }) => {
  await expect(page.getByRole("group", { name: "Look again" })).toHaveCount(0);
  await expect(page.getByText("Not this time.")).toBeVisible();
  const card = (world as PracticeWorld).deck?.cards[0];
  const a = row<{ disputed_at: string | null }>(
    world,
    "SELECT disputed_at FROM attempts WHERE item_id = ?",
    card?.item_id ?? "",
  );
  expect(a.disputed_at).toBeNull();
});

// ---- ask about this ----------------------------------------------------------------

const tutor = (page: Page) => page.getByRole("dialog", { name: "Ask about this" });

async function askInSheet(page: Page, text: string, replyStarts = "You picked") {
  const box = tutor(page).getByLabel("Your question");
  await box.fill(text);
  await box.press("Enter");
  await expect(tutor(page).getByLabel("The tutor's reply").last()).toContainText(replyStarts, { timeout: 15_000 });
}

When("I ask about it {string}", async ({ page }, text: string) => {
  await page.getByRole("button", { name: /Ask about this/ }).click();
  await expect(tutor(page)).toBeVisible();
  await askInSheet(page, text);
});

When("I ask about it again in a new conversation {string}", async ({ page }, text: string) => {
  await tutor(page).getByRole("button", { name: "New conversation" }).click();
  await askInSheet(page, text);
});

Then("the tutor's reply streams in, about my pick and the key", async ({ page, world }) => {
  const card = (world as PracticeWorld).deck?.cards[0];
  if (!card) throw new Error("no card");
  const key = optionsOf(world, card.item_id).find((o) => o.correct);
  const reply = tutor(page).getByLabel("The tutor's reply").last();
  await expect(reply).toContainText(key?.text ?? "");
  await expect(reply).toContainText(key?.why ?? "");
});

Then("the conversation is kept with the card", async ({ page }) => {
  await page.keyboard.press("Escape");
  await expect(tutor(page)).toHaveCount(0);
  await expect(page.getByRole("button", { name: /Ask about this · 1 conversation$/ })).toBeVisible();
});

Then("the card shows {int} conversations", async ({ page }, n: number) => {
  await page.keyboard.press("Escape");
  await expect(tutor(page)).toHaveCount(0);
  await expect(page.getByRole("button", { name: new RegExp(`Ask about this · ${n} conversations$`) })).toBeVisible();
});

Then("I can reopen either", async ({ page }) => {
  await page.getByRole("button", { name: /Ask about this/ }).click();
  const earlier = tutor(page).getByRole("region", { name: "Earlier conversations" });
  await expect(earlier).toContainText("Earlier · 2 conversations");
  for (const [i, title] of ["Why is the key right?", "Give me an example"].entries()) {
    // Back to the list (it shows while no conversation is open).
    if (i > 0) await tutor(page).getByRole("button", { name: "New conversation" }).click();
    await earlier.getByRole("button", { name: title }).click();
    await expect(tutor(page).getByLabel("The tutor's reply")).toContainText("You picked");
  }
});

// ---- read in kizn --------------------------------------------------------------------

const sheet = (page: Page) => page.getByRole("dialog").filter({ hasText: "Learn this" });
const reader = (page: Page) => page.getByRole("dialog", { name: "Reader" });

Given("a concept with a verified page", async ({ page, world }) => {
  await seedTopic(page);
  await openPractice(page, world, `/#/practice/${TOPIC}`);
  await skipIntro(page);
  await answer(page, world as PracticeWorld, false);
  await openMissed(page, world as PracticeWorld);
  await expect(sheet(page).getByText("docs.example.org")).toBeVisible({ timeout: 15_000 });
});

When("I open it in kizn", async ({ page }) => {
  await sheet(page)
    .getByRole("button", { name: /docs\.example\.org/ })
    .click();
  await expect(reader(page)).toBeVisible();
});

Then("I read its text with the concept on top", async ({ page }) => {
  await expect(reader(page)).toContainText("Reading for");
  await expect(reader(page)).toContainText("An image is immutable layers plus config");
  await expect(reader(page).getByRole("heading", { level: 2 }).first()).toBeVisible();
});

Then("not the page's scripts or navigation", async ({ page }) => {
  await expect(reader(page)).not.toContainText("analytics()");
  await expect(reader(page)).not.toContainText("Docs Guides Reference");
});

When("kizn looks for resources for it", async ({ page, world }) => {
  await openMissed(page, world as PracticeWorld);
  await expect(sheet(page).getByText("docs.example.org")).toBeVisible({ timeout: 15_000 });
});

Then("a video with captions is offered", async ({ page }) => {
  await expect(sheet(page).getByRole("button", { name: /youtube\.com/ })).toBeVisible();
  await expect(sheet(page).getByLabel("Video")).toBeVisible();
});

Then("opening it shows its transcript with timestamps", async ({ page }) => {
  await sheet(page)
    .getByRole("button", { name: /youtube\.com/ })
    .click();
  await expect(reader(page).getByRole("button", { name: "0:00", exact: true })).toBeVisible();
  await expect(reader(page).getByRole("button", { name: "0:05", exact: true })).toBeVisible();
  await expect(reader(page).getByRole("button", { name: "Watch on YouTube" })).toBeVisible();
});

When("I write a note while reading it", async ({ page }) => {
  await sheet(page)
    .getByRole("button", { name: /docs\.example\.org/ })
    .click();
  await reader(page).getByLabel("Your note").fill("Layers are shared, so rebuilds are cheap.");
  await expect(reader(page).getByText("Saved")).toBeVisible();
});

When("I come back to the concept later", async ({ page }) => {
  await reader(page).getByRole("button", { name: "Back" }).click();
  await page.keyboard.press("Escape");
  await page.goto(`/#/topic/${TOPIC}`);
  await page.reload();
  await page.getByRole("region", { name: "Learn these" }).getByRole("button").first().click();
});

Then("the note is there", async ({ page }) => {
  await expect(sheet(page).getByRole("region", { name: "Your note" })).toContainText(
    "Layers are shared, so rebuilds are cheap.",
  );
});

// ---- asking about concepts and passages ------------------------------------------

When("I ask about the concept {string}", async ({ page }, text: string) => {
  await sheet(page)
    .getByRole("button", { name: /Ask about this concept/ })
    .click();
  await expect(tutor(page)).toBeVisible();
  await askInSheet(page, text, "About");
});

Then("the tutor's reply draws on the concept's resources", async ({ page }) => {
  const reply = tutor(page).getByLabel("The tutor's reply").last();
  await expect(reply).toContainText("From ");
  await expect(reply).toContainText("This page explains");
});

Then("the concept shows {int} conversation", async ({ page }, n: number) => {
  await page.keyboard.press("Escape");
  await expect(tutor(page)).toHaveCount(0);
  await expect(
    sheet(page).getByRole("button", { name: new RegExp(`Ask about this concept · ${n} conversation`) }),
  ).toBeVisible();
});

When("I pick a passage in the page and ask {string}", async ({ page }, text: string) => {
  await sheet(page)
    .getByRole("button", { name: /docs\.example\.org/ })
    .click();
  const article = reader(page).locator("article");
  const para = article.locator("p", { hasText: "This page explains" }).first();
  await expect(para).toBeVisible();
  // Select the first paragraph with the mouse, as a learner would.
  const box = await para.boundingBox();
  if (!box) throw new Error("no paragraph");
  await page.mouse.move(box.x + 2, box.y + 4);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width - 4, box.y + box.height - 4, { steps: 8 });
  await page.mouse.up();
  await page.getByRole("button", { name: "Ask about the selection" }).click();
  await expect(tutor(page).locator("blockquote")).toContainText("This page explains");
  await askInSheet(page, text, "About");
});

Then("the tutor's reply is about that passage, from the page", async ({ page }) => {
  const reply = tutor(page).getByLabel("The tutor's reply").last();
  await expect(reply).toContainText('On "This page explains');
  await expect(reply).toContainText("From it:");
});

Then("the page shows {int} conversation", async ({ page }, n: number) => {
  await page.keyboard.press("Escape");
  await expect(tutor(page)).toHaveCount(0);
  await expect(
    reader(page).getByRole("button", { name: new RegExp(`Ask about this · ${n} conversation`) }),
  ).toBeVisible();
});

// ---- when I'm stuck ------------------------------------------------------------------

When("I close the tutor", async ({ page }) => {
  await page.keyboard.press("Escape");
  await expect(tutor(page)).toHaveCount(0);
});

When("I finish the rest of the set", async ({ page, world }) => {
  await page.getByRole("button", { name: /^(Next|Finish)/ }).click();
  await answerAll(page, world as PracticeWorld, () => true);
});

When("I open what I missed", async ({ page }) => {
  await group(page, "Learn these first").first().getByRole("button").click();
  await expect(sheet(page).getByText("docs.example.org")).toBeVisible({ timeout: 15_000 });
});

When("I close the tutor and the reader, and mark it studied", async ({ page }) => {
  await page.keyboard.press("Escape");
  await expect(tutor(page)).toHaveCount(0);
  await reader(page).getByRole("button", { name: "Back" }).click();
  await expect(reader(page)).toHaveCount(0);
  await sheet(page).getByRole("button", { name: "Mark studied" }).click();
  await expect(sheet(page).getByText("Studied")).toBeVisible();
  await page.keyboard.press("Escape");
});

// ---- reading list ----------------------------------------------------------------------

const readingList = (page: Page) => page.getByRole("dialog", { name: "Your reading" });

Given("I missed {int} concepts in my starting point", async ({ page, world }, n: number) => {
  await seedTopic(page);
  await openPractice(page, world, `/#/practice/${TOPIC}`);
  await skipIntro(page);
  await answerAll(page, world as PracticeWorld, (i) => i >= n);
});

When("I open my reading list", async ({ page }) => {
  await page.goto("/#/");
  await page.getByRole("button", { name: /Start reading/ }).click();
  await expect(readingList(page)).toBeVisible();
  // kizn finds and checks resources as the list opens.
  await expect(readingList(page).getByText("finding…")).toHaveCount(0, { timeout: 20_000 });
});

Then("I see both concepts, with what there is to learn from for each", async ({ page }) => {
  await expect(readingList(page).getByRole("img", { name: "2 pages, 1 video" })).toHaveCount(2);
});

Then("none of them studied yet", async ({ page }) => {
  await expect(readingList(page)).toContainText("0 of 2 studied");
});

When("I mark one of them studied", async ({ page }) => {
  await readingList(page).locator("section button").first().click();
  await sheet(page).last().getByRole("button", { name: "Mark studied" }).click();
  await expect(sheet(page).last().getByText("Studied")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(sheet(page).last()).toHaveCount(0);
});

Then("the reading list shows {int} of {int} studied", async ({ page }, a: number, b: number) => {
  await expect(readingList(page)).toContainText(`${a} of ${b} studied`);
  await expect(readingList(page).getByLabel("Studied", { exact: true })).toHaveCount(a);
});
