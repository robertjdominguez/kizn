import { createServer } from "node:net";
import { expect, type Page } from "@playwright/test";
import { bridge, Given, Then, When, type World } from "./fixtures";

// Connect a model, through the Settings wizard. The bridge runs in test mode,
// so every non-local model URL goes to the fake model server, whose
// behaviour comes from the URL's first path segment (/no-json/v1) and
// defaults to "works".

async function freshSettings(page: Page, world: World) {
  const res = await page.request.post(`${bridge()}/test/fresh`);
  expect(res.ok(), "the bridge needs its `test` feature").toBeTruthy();
  world.dataDir = (await res.json()).data_dir;
  await page.goto("/#/settings");
}

/** Through the "Other server" tile to the Model step, with the model typed. */
async function addCustomProvider(page: Page, world: World, name: string, url: string, model: string) {
  await page.getByRole("button", { name: /Other server/ }).click();
  await page.getByRole("textbox", { name: "Base URL" }).fill(url);
  await page.getByRole("textbox", { name: "Name in kizn" }).fill(name);
  await page.getByRole("button", { name: "Continue" }).click();
  await page.getByRole("combobox", { name: "Model" }).fill(model);
  world.provider = name;
  world.model = model;
}

/** The Settings list, after a reload (the wizard is gone once a provider exists). */
async function settingsList(page: Page) {
  await page.goto("/#/settings");
  await page.reload();
  await expect(page.getByRole("button", { name: /Add a provider/ })).toBeVisible();
}

async function roleOption(page: Page, role: string, provider: string) {
  const roles = page.getByText("Roles (advanced)");
  if (!(await page.getByRole("combobox", { name: role }).isVisible())) await roles.click();
  return page.getByRole("combobox", { name: role }).locator(`option[value="${provider}"]`);
}

/** "I see works": the test passed, so the wizard reaches Ready; the card then says "works". */
export async function seeProviderWorks(page: Page, world: World) {
  await expect(page.getByText("You're connected")).toBeVisible();
  await page.getByRole("button", { name: "Stay in Settings" }).click();
  const card = page.getByRole("article", { name: `Provider ${world.provider}` });
  await expect(card.getByTestId("check-summary")).toHaveText("works");
}

// Both a check (First run: Today says so) and a starting point (Connect a
// model: fresh Settings, where the wizard opens). playwright-bdd matches steps
// without their keyword, so one definition serves both.
Given("no model is configured", async ({ page, world }) => {
  if (page.url() !== "about:blank") {
    await expect(page.getByText("No model connected")).toBeVisible();
    return;
  }
  await freshSettings(page, world);
  await expect(page.getByText("Where should your model run?")).toBeVisible();
});

When(
  "I add a provider {string} with protocol {string} at {string} and model {string}",
  async ({ page, world }, name: string, _protocol: string, url: string, model: string) => {
    await addCustomProvider(page, world, name, url, model);
  },
);

Given("a provider whose replies are not valid JSON", async ({ page, world }) => {
  await freshSettings(page, world);
  await addCustomProvider(page, world, "chatty", "http://models.test/no-json/v1", "chatty-7b");
});

Given("a provider at an address where nothing is listening", async ({ page, world }) => {
  const port = await new Promise<number>((resolve) => {
    const s = createServer().listen(0, "127.0.0.1", () => {
      const p = (s.address() as { port: number }).port;
      s.close(() => resolve(p));
    });
  });
  world.closedPort = port;
  await freshSettings(page, world);
  await addCustomProvider(page, world, "nowhere", `http://127.0.0.1:${port}/v1`, "m");
});

When("I test the connection", async ({ page }) => {
  await page.getByRole("button", { name: "Test it" }).click();
  // Settled: either it passed (Ready) or a fix is offered.
  await expect(
    page.getByText("You're connected").or(page.getByRole("button", { name: /Try another model|Fix the/ })),
  ).toBeVisible();
});

Then("all three roles use {string}", async ({ page }, name: string) => {
  await page.getByText("Roles (advanced)").click();
  for (const role of ["Author", "Checker", "Tutor"]) {
    await expect(page.getByRole("combobox", { name: role })).toHaveValue(name);
  }
});

Then("I see that it can tutor but can't author items", async ({ page, world }) => {
  await expect(page.getByText(`and ${world.model} can't. Try another model.`)).toBeVisible();
  await expect(page.getByText("streams replies")).toBeVisible();
});

Then("I can't assign it the author role", async ({ page, world }) => {
  await settingsList(page);
  await expect(await roleOption(page, "Author", world.provider ?? "")).toHaveAttribute("disabled");
});

Then("I see that nothing answered at that address", async ({ page, world }) => {
  await expect(page.getByText(`nothing answered at http://127.0.0.1:${world.closedPort}`).first()).toBeVisible();
});

Then("no role can be assigned to it", async ({ page, world }) => {
  await settingsList(page);
  for (const role of ["Author", "Checker", "Tutor"]) {
    await expect(await roleOption(page, role, world.provider ?? "")).toHaveAttribute("disabled");
  }
});

Given("kizn is open on Settings", async ({ page, world }) => {
  await freshSettings(page, world);
  await expect(page.getByText("Where should your model run?")).toBeVisible();
});

When("I add a provider to config.toml by hand", async ({ world }) => {
  const { writeFileSync } = await import("node:fs");
  const { join } = await import("node:path");
  if (!world.dataDir) throw new Error('no data directory: start with "Given kizn is open on Settings"');
  writeFileSync(
    join(world.dataDir, "config.toml"),
    '[providers.handmade]\nprotocol = "openai"\nbase_url = "http://gpu:11434/v1"\nmodel = "qwen3:32b"\n',
  );
  world.provider = "handmade";
});

Then("Settings shows it without a restart", async ({ page }) => {
  await expect(page.getByRole("article", { name: "Provider handmade" })).toBeVisible({ timeout: 5000 });
});
