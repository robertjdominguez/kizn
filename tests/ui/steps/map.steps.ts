import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { expect, type Page } from "@playwright/test";
import { bridge, Given, Then, When, type World } from "./fixtures";

// Linking topics on the map, from their pages. The topics come from the
// map-topics fixture; titles are looked up in the scenario's database.

function topicId(world: World, title: string): string {
  if (!world.dataDir) throw new Error("no data directory");
  const db = new DatabaseSync(join(world.dataDir, "kizn.db"), { readOnly: true });
  try {
    const row = db.prepare("SELECT id FROM topics WHERE title = ?").get(title) as { id: string } | undefined;
    if (!row) throw new Error(`no topic ${title}`);
    return row.id;
  } finally {
    db.close();
  }
}

const onMap = (page: Page) => page.getByRole("region", { name: "On your map" });

async function seedTopics(page: Page) {
  const res = await page.request.post(`${bridge()}/test/seed`, { data: { fixture: "map-topics" } });
  expect(res.ok(), await res.text()).toBeTruthy();
}

async function makeFoundation(page: Page, world: World, lower: string, upper: string) {
  await page.goto(`/#/topic/${encodeURIComponent(topicId(world, upper))}`);
  await onMap(page).getByRole("button", { name: "Foundation" }).click();
  await onMap(page).getByRole("option", { name: lower, exact: true }).click();
}

// The fixture holds every topic the scenarios name.
Given("topics {string} and {string}", async ({ page }, _a: string, _b: string) => {
  await seedTopics(page);
});

Given("topics {string}, {string} and {string}", async ({ page }, _a: string, _b: string, _c: string) => {
  await seedTopics(page);
});

Given(
  "{string} rests on {string}, which rests on {string}",
  async ({ page, world }, a: string, b: string, c: string) => {
    await makeFoundation(page, world, b, a);
    await expect(onMap(page)).toContainText(b);
    await makeFoundation(page, world, c, b);
    await expect(onMap(page)).toContainText(c);
  },
);

When("I make {string} a foundation of {string}", async ({ page, world }, lower: string, upper: string) => {
  await makeFoundation(page, world, lower, upper);
});

Then("{string} rests on {string}", async ({ page }, _upper: string, lower: string) => {
  await expect(onMap(page)).toContainText("Rests on");
  await expect(onMap(page).getByRole("link", { name: lower })).toBeVisible();
});

Then("{string} supports {string}", async ({ page, world }, lower: string, upper: string) => {
  await page.goto(`/#/topic/${encodeURIComponent(topicId(world, lower))}`);
  await expect(onMap(page)).toContainText("Supports");
  await expect(onMap(page).getByRole("link", { name: upper })).toBeVisible();
});

Then("I'm told it would make a loop", async ({ page }) => {
  await expect(onMap(page)).toContainText("would make a loop");
});
