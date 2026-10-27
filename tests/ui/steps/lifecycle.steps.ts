import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { expect } from "@playwright/test";
import { Then, When, type World } from "./fixtures";

// Letting a topic go: deleting one from its page, or discarding a draft
// from setup, each asked first.

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

When("I delete {string}", async ({ page, world }, title: string) => {
  await page.goto(`/#/topic/${encodeURIComponent(topicId(world, title))}`);
  await expect(page.getByRole("heading", { level: 1, name: title, exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Delete topic" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Delete topic" }).click();
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
});

When("I discard the draft", async ({ page }) => {
  await page.getByRole("button", { name: "Discard draft" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Discard draft" }).click();
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
});

Then("{string} is gone from Today and my map", async ({ page }, title: string) => {
  await page.goto("/#/");
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  await expect(page.getByRole("link", { name: new RegExp(title) })).toHaveCount(0);
  await page.goto("/#/map");
  await expect(page.getByRole("heading", { name: "Your map" })).toBeVisible();
  await expect(page.getByRole("button", { name: title, exact: true })).toHaveCount(0);
});

Then("{string} and {string} are still there", async ({ page }, a: string, b: string) => {
  for (const t of [a, b]) await expect(page.getByRole("button", { name: t, exact: true })).toBeVisible();
});
