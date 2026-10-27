import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { expect, type Page, type Response } from "@playwright/test";
import type { DeckView } from "../../../ui/src/bindings/DeckView";
import { bridge, Given, Then, When, type World } from "./fixtures";

// Foundations: proposed at setup, checked, flagged from misses, and drawn on
// the map. Keys for check questions are read from the scenario's database.

function db(world: World, readOnly = true) {
  if (!world.dataDir) throw new Error("no data directory");
  return new DatabaseSync(join(world.dataDir, "kizn.db"), { readOnly });
}

const keepBox = (page: Page, title: string) => page.getByRole("checkbox", { name: `Keep ${title}` });

Then("the draft proposes {int} foundations, each with why it matters", async ({ page }, n: number) => {
  await expect(page.getByRole("heading", { name: "What it rests on" })).toBeVisible();
  await expect(page.getByRole("checkbox", { name: /^Keep / })).toHaveCount(n);
  await expect(page.getByText("images are layers stacked by a union filesystem")).toBeVisible();
});

When("I drop {string} while shaping it", async ({ page }, title: string) => {
  await keepBox(page, title).uncheck();
  await expect(keepBox(page, title)).not.toBeChecked();
});

Then("{int} foundations are kept", async ({ page }, n: number) => {
  await expect(page.getByRole("checkbox", { name: /^Keep /, checked: true })).toHaveCount(n);
});

Given("I confirmed a topic keeping 2 foundations", async ({ page }) => {
  await page.goto("/#/new-topic");
  await page.getByLabel("Name it").fill("Container fundamentals");
  await page
    .getByLabel("What do you want to be able to do, and why?")
    .fill("explain to customers why deployments behave differently");
  await page.getByRole("button", { name: "Continue" }).click();
  await expect(page.getByRole("heading", { name: "Shape it" })).toBeVisible({ timeout: 60_000 });
  await keepBox(page, "Networking basics").uncheck();
  await expect(keepBox(page, "Networking basics")).not.toBeChecked();
  await page
    .getByRole("button", { name: /Continue/ })
    .last()
    .click();
  await page.getByRole("button", { name: "Confirm" }).click();
  await expect(page.getByText("Check the ground")).toBeVisible();
});

Then("there are {int} questions on each foundation to check", async ({ page }, n: number) => {
  for (const title of ["Union filesystems", "Linux processes"]) {
    await expect(page.getByRole("region", { name: title }).getByLabel(`0 of ${n} answered`)).toBeVisible({
      timeout: 30_000,
    });
  }
});

async function answerChecks(page: Page, world: World, title: string, right: boolean[]) {
  const section = page.getByRole("region", { name: title });
  for (const r of right) {
    const q = section.getByLabel("Check question");
    const stem = ((await q.locator("p").first().textContent()) ?? "").trim();
    const d = db(world);
    const row = d.prepare("SELECT options_json FROM ground_items WHERE stem = ?").get(stem) as { options_json: string };
    d.close();
    const pick = (JSON.parse(row.options_json) as { text: string; correct: boolean }[]).find((o) => o.correct === r);
    await q.locator("ol li button", { hasText: pick?.text ?? "" }).click();
    await section.getByRole("button", { name: /Next question|See how it went/ }).click();
  }
}

When("I answer the questions on {string} mostly right", async ({ page, world }, title: string) => {
  await answerChecks(page, world, title, [true, true, false]);
});

When("I answer the questions on {string} mostly wrong", async ({ page, world }, title: string) => {
  await answerChecks(page, world, title, [true, false, false]);
});

Then("{string} seems solid and {string} might be shaky", async ({ page }, solid: string, shaky: string) => {
  await expect(page.getByRole("region", { name: solid })).toContainText("Seems solid");
  await expect(page.getByRole("region", { name: shaky })).toContainText("Might be shaky");
  await expect(page.getByRole("region", { name: shaky }).getByRole("button", { name: "Start it" })).toBeVisible();
});

// ---- flags -----------------------------------------------------------------------------

const TOPIC = "01SEEDTOPIC00000000000000T";
const flags = (page: Page) => page.getByRole("region", { name: "Misses that point deeper" });

// Every wrong option on the seeded bank's first two concepts points at
// "Union filesystems"; missing every card in the first set covers both.
Given("I missed {int} questions that point at {string}", async ({ page, world }, _n: number, _title: string) => {
  const res = await page.request.post(`${bridge()}/test/seed`, { data: { fixture: "topic-with-bank" } });
  expect(res.ok(), await res.text()).toBeTruthy();
  let deck: DeckView | undefined;
  const keep = async (r: Response) => {
    if (r.url().endsWith("/rpc") && r.request().postData()?.includes("start_practice")) {
      deck = ((await r.json()) as { data: DeckView }).data;
    }
  };
  page.on("response", keep);
  await page.goto(`/#/practice/${TOPIC}`);
  await expect.poll(() => deck !== undefined).toBe(true);
  await page.waitForTimeout(400);
  page.off("response", keep);
  await page.getByRole("button", { name: "Skip" }).click();
  for (const card of deck?.cards ?? []) {
    const d = db(world);
    const row = d.prepare("SELECT options_json FROM items WHERE id = ?").get(card.item_id) as { options_json: string };
    d.close();
    const wrong = (JSON.parse(row.options_json) as { text: string; correct: boolean }[]).find((o) => !o.correct);
    const q = page.getByRole("region", { name: "Question" });
    await expect(q).toContainText(card.stem);
    await q.locator("ol > li button", { hasText: wrong?.text ?? "" }).click();
    await expect(page.getByText("Not this time.")).toBeVisible();
    await page.getByRole("button", { name: /^(Next|Finish)/ }).click();
  }
  await expect(page.getByRole("region", { name: "Summary" })).toBeVisible();
});

Then("the topic and Today flag {string}", async ({ page }, title: string) => {
  await page.goto(`/#/topic/${TOPIC}`);
  await expect(flags(page)).toContainText(title);
  await page.goto("/#/");
  await expect(flags(page)).toContainText(title);
});

When("I add it underneath", async ({ page }) => {
  await flags(page).getByRole("button", { name: "Add it underneath" }).click();
});

Then("the flag is gone and {string} is suggested on my map", async ({ page }, title: string) => {
  await expect(flags(page)).toHaveCount(0);
  await page.goto("/#/map");
  await expect(page.getByRole("button", { name: `${title} (suggested)` })).toBeVisible();
});

// ---- the map ----------------------------------------------------------------------------

Given("{string} is suggested under {string}", async ({ world }, title: string, upper: string) => {
  const d = db(world, false);
  const row = d.prepare("SELECT id FROM topics WHERE title = ?").get(upper) as { id: string };
  d.prepare(
    "INSERT INTO suggestions (id, for_topic, title, why, source, status, topic_id, created_at) VALUES (?, ?, ?, ?, 'flag', 'later', NULL, '2026-10-25 09:00:00')",
  ).run("01SUGGESTION00000000000001", row.id, title, "services talk over ports");
  d.close();
});

When("I open the map", async ({ page }) => {
  await page.goto("/#/map");
  await expect(page.getByRole("heading", { name: "Your map" })).toBeVisible();
});

const nodeY = async (page: Page, name: string) =>
  (await page.getByRole("button", { name, exact: true }).boundingBox())?.y ?? -1;

Then("{string} is at the bottom and {string} at the top", async ({ page }, bottom: string, top: string) => {
  await expect(page.getByRole("button", { name: top, exact: true })).toBeVisible();
  await page.waitForTimeout(800);
  const [b, t] = [await nodeY(page, bottom), await nodeY(page, top)];
  expect(b).toBeGreaterThan(t);
});

Then("{string} is drawn as a suggestion under {string}", async ({ page }, title: string, upper: string) => {
  const s = page.getByRole("button", { name: `${title} (suggested)` });
  await expect(s).toBeVisible();
  expect((await s.boundingBox())?.y ?? 0).toBeGreaterThan(await nodeY(page, upper));
});
