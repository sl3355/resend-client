import { expect, test } from "@playwright/test";

test("workspace switching preserves drafts, settings, queue and last active space", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "New message" }).click();
  await page
    .getByRole("dialog")
    .getByLabel("Subject", { exact: true })
    .fill("Only in Studio");
  await page.getByRole("button", { name: "Save and close composer" }).click();
  await page.getByRole("button", { name: "Switch workspace" }).click();
  await page.getByLabel("New workspace name").fill("Personal");
  await page
    .getByRole("button", { name: "Add workspace", exact: true })
    .click();
  const settings = page.getByRole("dialog");
  await expect(settings.getByLabel("Workspace name")).toHaveValue("Personal");
  // The browser demo must still refuse real credentials in every workspace.
  await expect(settings.getByPlaceholder("re_••••••••••••••••")).toBeDisabled();
  await settings.getByLabel("Workspace name").fill("Personal renamed");
  await settings
    .getByRole("button", { name: "Save changes", exact: true })
    .click();
  await settings
    .getByRole("button", { name: "Sender identities", exact: true })
    .click();
  await expect(
    settings.getByText("Add your first sender identity."),
  ).toBeVisible();
  await settings.getByRole("button", { name: "Back to mail" }).click();
  await expect(page.locator(".message-card")).toHaveCount(0);
  await page.getByRole("button", { name: /^Drafts/ }).click();
  await expect(page.getByText("Only in Studio", { exact: true })).toHaveCount(
    0,
  );
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Switch workspace" }),
  ).toContainText("Personal renamed");
  await page.getByRole("button", { name: "Switch workspace" }).click();
  await page
    .getByRole("combobox", { name: "Workspace", exact: true })
    .selectOption("demo");
  await expect(page.locator(".message-card")).toHaveCount(9);
  await page.getByRole("button", { name: /^Drafts/ }).click();
  await page
    .getByRole("listitem")
    .filter({ hasText: "Only in Studio" })
    .click();
  await expect(
    page.getByRole("dialog").getByLabel("Subject", { exact: true }),
  ).toHaveValue("Only in Studio");
  await page
    .getByRole("dialog")
    .getByLabel("To", { exact: true })
    .fill("test@example.com");
  await page.getByRole("button", { name: "Queue demo message" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByRole("button", { name: "Switch workspace" }).click();
  const personalId = await page
    .getByRole("combobox", { name: "Workspace", exact: true })
    .locator("option")
    .filter({ hasText: "Personal renamed" })
    .getAttribute("value");
  await page
    .getByRole("combobox", { name: "Workspace", exact: true })
    .selectOption(personalId!);
  await page.getByRole("button", { name: "Switch workspace" }).click();
  await expect(
    page
      .getByRole("combobox", { name: "Workspace", exact: true })
      .locator('option[value="demo"]'),
  ).toContainText("in Outbox");
  await page.getByRole("button", { name: "Switch workspace" }).click();
  await page.getByRole("button", { name: /^Outbox/ }).click();
  await expect(page.getByText("Only in Studio", { exact: true })).toHaveCount(
    0,
  );
});

test("switch resets search, selected mail and undo and remains available on small screens", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: /Read A few ideas/ }).click();
  await page
    .getByRole("button", { name: "Archive message", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Undo", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("textbox", { name: "Search downloaded mail" })
    .fill("amelia");
  await page.getByRole("button", { name: "Switch workspace" }).click();
  await page.getByLabel("New workspace name").fill("Second");
  await page
    .getByRole("button", { name: "Add workspace", exact: true })
    .click();
  await page.getByRole("button", { name: "Back to mail" }).click();
  await expect(
    page.getByRole("textbox", { name: "Search downloaded mail" }),
  ).toHaveValue("");
  await expect(
    page.getByRole("button", { name: "Undo", exact: true }),
  ).toHaveCount(0);
  await expect(page.locator(".reader-subject")).toHaveCount(0);
  await page.setViewportSize({ width: 600, height: 800 });
  await page.getByRole("button", { name: "Switch workspace" }).click();
  await expect(
    page.getByRole("combobox", { name: "Workspace", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("combobox", { name: "Workspace", exact: true })
    .selectOption("demo");
  await expect(
    page.getByRole("button", { name: "Switch workspace" }),
  ).toContainText("Demo workspace");
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
