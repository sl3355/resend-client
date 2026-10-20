// @vitest-environment jsdom
import { beforeEach, expect, test, vi } from "vitest";

beforeEach(() => {
  localStorage.clear();
  vi.resetModules();
});

test("a delayed callback cannot save or send into a newly selected workspace", async () => {
  const { command, workspaceCommand } = await import("./backend");
  const { profile } = await command<import("./types").Snapshot>("get_snapshot");
  const old = workspaceCommand(profile.id);
  await old("create_workspace", { name: "Second" });
  const draft = { id: crypto.randomUUID(), revision: 1, subject: "A secret" };
  await expect(old("save_draft", { draft })).rejects.toThrow(
    "active workspace changed",
  );
  await expect(old("enqueue_send", { draft })).rejects.toThrow(
    "active workspace changed",
  );
  await expect(
    old("save_settings", {
      identities: profile.identities,
      settings: { theme: "dark" },
    }),
  ).rejects.toThrow("active workspace changed");
  const second = await command<import("./types").Snapshot>("get_snapshot");
  expect(second.drafts).toEqual([]);
  expect(second.outbox).toEqual([]);
  expect(second.profile.identities).toEqual([]);
});

test("a failed workspace creation does not change the current selection", async () => {
  const { command, workspaceCommand } = await import("./backend");
  const before = await command<import("./types").Snapshot>("get_snapshot");
  await expect(
    workspaceCommand(before.profile.id)("create_workspace", { name: "  " }),
  ).rejects.toThrow("workspace name");
  expect(
    (await command<import("./types").Snapshot>("get_snapshot")).profile.id,
  ).toBe(before.profile.id);
});
