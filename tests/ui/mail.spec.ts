import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Inbox", exact: true }),
  ).toBeVisible();
});
test("reads in an opaque frame with no message network requests", async ({
  page,
}) => {
  const requests: string[] = [];
  page.on("request", (r) => requests.push(r.url()));
  await page.getByRole("button", { name: /Read A few ideas/ }).click();
  const frame = page.locator('iframe[title="Email message content"]');
  await expect(frame).toHaveAttribute("sandbox", "");
  await expect(
    page.frameLocator("iframe").getByText("The new direction"),
  ).toBeVisible();
  await page.getByRole("button", { name: "Plain text", exact: true }).click();
  await expect(page.locator(".plain-message")).toContainText(
    "Would love to hear what you think.",
  );
  expect(
    requests.filter((u) => !u.startsWith("http://127.0.0.1:1420/")),
  ).toEqual([]);
});
test("combines search filters and clears them", async ({ page }) => {
  await page
    .getByRole("textbox", { name: "Search downloaded mail" })
    .fill("from:amelia label:Work");
  await expect(
    page.getByRole("button", { name: /Read A few ideas/ }),
  ).toBeVisible();
  await expect(page.locator(".message-card")).toHaveCount(1);
  await page.getByRole("button", { name: "Clear search" }).click();
  await expect(page.locator(".message-card")).toHaveCount(9);
});
test("archives with undo and preserves local state after reload", async ({
  page,
}) => {
  await page.getByRole("button", { name: /Read A few ideas/ }).click();
  await page
    .getByRole("button", { name: "Archive message", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: /Read A few ideas/ }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(
    page.getByRole("button", { name: /Read A few ideas/ }),
  ).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole("button", { name: /Read A few ideas/ }),
  ).toBeVisible();
});
test("saves a rich draft and recovers it after page restart", async ({
  page,
}) => {
  await page.getByRole("button", { name: "New message" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("To", { exact: true }).fill("friend@example.com");
  await dialog.getByLabel("Subject", { exact: true }).fill("A durable thought");
  await dialog
    .getByRole("textbox", { name: "Message body" })
    .fill("This draft should survive a restart.");
  await expect(dialog.getByText("Saved on this device")).toBeVisible();
  await page.reload();
  await page.getByRole("button", { name: /^Drafts/ }).click();
  await page
    .getByRole("listitem")
    .filter({ hasText: "A durable thought" })
    .click();
  await expect(
    page.getByRole("textbox", { name: "Message body" }),
  ).toContainText("This draft should survive a restart.");
});
test("validates recipients, queues explicitly and exposes the Outbox", async ({
  page,
}) => {
  await page.getByRole("button", { name: "New message" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Queue demo message" }).click();
  await expect(dialog.getByRole("alert")).toContainText(
    "Add at least one To recipient.",
  );
  await dialog.getByLabel("To", { exact: true }).fill("friend@example.com");
  await dialog.getByLabel("Subject", { exact: true }).fill("Hello");
  await dialog.getByRole("textbox", { name: "Message body" }).fill("A note.");
  await dialog.getByRole("button", { name: "Queue demo message" }).click();
  await expect(dialog).not.toBeVisible();
  await page.getByRole("button", { name: /^Outbox/ }).click();
  await expect(
    page.getByText("Demo only. No email will be transmitted."),
  ).toBeVisible();
  await page.getByRole("button", { name: "Cancel queued send" }).click();
  await expect(page.getByText("All clear here")).toBeVisible();
});
test("reply uses the selected sender identity and quotes the source", async ({
  page,
}) => {
  await page.getByRole("button", { name: /Read A few ideas/ }).click();
  await page
    .getByRole("button", { name: "Reply to message", exact: true })
    .click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByLabel("To", { exact: true })).toHaveValue(
    "Amelia Hart <amelia@example.com>",
  );
  await expect(dialog.getByLabel("Subject", { exact: true })).toHaveValue(
    "Re: A few ideas for our next chapter",
  );
  await expect(
    dialog.getByRole("textbox", { name: "Message body" }),
  ).toContainText("Amelia Hart");
});
test("settings changes theme and keeps keyboard focus inside the dialog", async ({
  page,
}) => {
  await page.getByRole("button", { name: /Settings v0.1/ }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Appearance", exact: true }).click();
  await dialog.getByRole("button", { name: "Dark", exact: true }).click();
  await dialog
    .getByRole("button", { name: "Save changes", exact: true })
    .click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await dialog.getByRole("button", { name: "Close settings" }).focus();
  await page.keyboard.press("Shift+Tab");
  expect(
    await dialog.evaluate((el) => el.contains(document.activeElement)),
  ).toBe(true);
  await dialog.getByRole("button", { name: "Back to mail" }).click();
  await expect(dialog).not.toBeVisible();
});
test("narrow layout opens a reader and returns to the list", async ({
  page,
}) => {
  await page.setViewportSize({ width: 800, height: 800 });
  await page.getByRole("button", { name: "Back to message list" }).click();
  await page.getByRole("button", { name: /Read A few ideas/ }).click();
  await page.getByRole("button", { name: "Back to message list" }).click();
  await expect(
    page.getByRole("button", { name: /Read Re: The little details/ }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});

test("an asynchronous Cancel keeps the draft intact", async ({ page }) => {
  await page.getByRole("button", { name: "New message" }).click();
  const dialog = page.getByRole("dialog");
  await dialog
    .getByRole("textbox", { name: "Message body" })
    .fill("Keep this draft.");
  // Tauri's confirmation integration is asynchronous. A Promise must never be treated as consent.
  await page.evaluate(() => {
    window.confirm = (() =>
      Promise.resolve(false)) as unknown as typeof window.confirm;
  });
  await dialog.getByRole("button", { name: "Discard draft" }).click();
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByRole("textbox", { name: "Message body" }),
  ).toContainText("Keep this draft.");
});
