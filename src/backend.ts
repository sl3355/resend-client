import { invoke, isTauri } from "@tauri-apps/api/core";
import { demoData } from "./demo";
import type { Draft, Mail, Query, Snapshot } from "./types";
export const desktop = isTauri();
const storageKey = "resend-client-demo-v1";
export async function confirmAction(message: string): Promise<boolean> {
  if (desktop) return command<boolean>("confirm_action", { message });
  return window.confirm(message);
}
let demo: Snapshot = (() => {
  try {
    return JSON.parse(localStorage.getItem(storageKey) || "null") || demoData();
  } catch {
    return demoData();
  }
})();
const save = () => localStorage.setItem(storageKey, JSON.stringify(demo));
type Args = Record<string, unknown>;
export async function command<T>(name: string, args: Args = {}): Promise<T> {
  if (desktop) return invoke<T>(name, args);
  const result = await preview(name, args);
  save();
  return structuredClone(result) as T;
}
async function preview(name: string, a: Args): Promise<unknown> {
  switch (name) {
    case "get_snapshot":
      demo.inbox_unread = demo.messages.filter(
        (m) => m.folder === "inbox" && !m.is_read,
      ).length;
      demo.labels = Array.from(
        new Set(demo.messages.flatMap((m) => m.labels)),
      ).sort();
      return demo;
    case "list_messages": {
      const q = a.query as Query;
      return demo.messages
        .filter((m) => {
          if (q.folder === "starred" && (!m.starred || m.folder === "trash"))
            return false;
          if (
            q.folder === "unmatched" &&
            (m.direction !== "incoming" ||
              m.folder === "trash" ||
              m.to.some((x) =>
                demo.profile.identities.some((i) => i.email === x),
              ))
          )
            return false;
          if (q.folder === "all" && m.folder === "trash") return false;
          if (
            !["all", "starred", "unmatched"].includes(q.folder) &&
            q.folder &&
            m.folder !== q.folder
          )
            return false;
          if (
            q.identity &&
            ![...m.to, m.from].some((x) => x.includes(q.identity))
          )
            return false;
          return (q.search.match(/(?:[^\s"]+|"[^"]*")+/g) || []).every(
            (raw) => {
              const [key, ...parts] = raw.replaceAll('"', "").split(":");
              const val = parts.join(":").toLowerCase();
              if (key === "is")
                return val === "unread"
                  ? !m.is_read
                  : val === "starred"
                    ? m.starred
                    : false;
              if (key === "has") return !!m.attachments.length;
              if (key === "label")
                return m.labels.some((l) => l.toLowerCase() === val);
              if (key === "before" || key === "after")
                return key === "before"
                  ? m.created_at < val
                  : m.created_at >= val;
              if (["from", "to", "subject"].includes(key))
                return String(m[key as keyof Mail])
                  .toLowerCase()
                  .includes(val);
              return [m.subject, m.from, ...m.to, m.text]
                .join(" ")
                .toLowerCase()
                .includes(raw.toLowerCase());
            },
          );
        })
        .sort((x, y) => y.created_at.localeCompare(x.created_at))
        .slice(q.offset, q.offset + q.limit);
    }
    case "read_message": {
      const m = demo.messages.find((m) => m.id === a.id);
      if (!m) throw new Error("Message unavailable.");
      m.is_read = true;
      return m;
    }
    case "set_local_flags": {
      const m = demo.messages.find((m) => m.id === a.id);
      if (m) {
        if (a.folder != null) m.folder = a.folder as string;
        if (a.isRead != null) m.is_read = a.isRead as boolean;
        if (a.starred != null) m.starred = a.starred as boolean;
        if (a.labels != null) m.labels = a.labels as string[];
      }
      return null;
    }
    case "permanently_remove":
      demo.messages = demo.messages.filter((m) => m.id !== a.id);
      return null;
    case "save_draft": {
      const d = a.draft as Draft;
      const old = demo.drafts.find((x) => x.id === d.id);
      if (!old || old.revision < d.revision) {
        demo.drafts = demo.drafts.filter((x) => x.id !== d.id);
        demo.drafts.unshift(structuredClone(d));
      }
      return null;
    }
    case "delete_draft":
      demo.drafts = demo.drafts.filter((x) => x.id !== a.id);
      return null;
    case "enqueue_send": {
      const d = a.draft as Draft;
      const id = crypto.randomUUID();
      demo.drafts = demo.drafts.filter((x) => x.id !== d.id);
      demo.outbox.unshift({
        id,
        draft_id: d.id,
        subject: d.subject,
        to: [d.to],
        state: "queued",
        error: "Demo only. No email will be transmitted.",
        first_attempt: null,
        provider_id: null,
      });
      return id;
    }
    case "cancel_intent":
      demo.outbox = demo.outbox.filter((x) => x.id !== a.id);
      return null;
    case "make_intent_copy": {
      const intent = demo.outbox.find((i) => i.id === a.id);
      const draft = demo.drafts.find((d) => d.id === intent?.draft_id);
      if (!intent || !draft)
        throw new Error("Original draft is no longer available.");
      intent.state = "needs_review";
      intent.error =
        "A draft copy was created. This original attempt is paused.";
      return { ...draft, id: crypto.randomUUID(), revision: 1 };
    }
    case "refresh_connection":
      demo.profile.last_sync = new Date().toISOString();
      return null;
    case "save_settings":
      demo.profile.identities =
        a.identities as Snapshot["profile"]["identities"];
      demo.profile.settings = a.settings as Snapshot["profile"]["settings"];
      return null;
    case "connect_account":
      throw new Error(
        "Connect your Resend account in the desktop app. Browser preview never accepts or stores credentials.",
      );
    default:
      throw new Error(
        "This action uses native desktop integration. Run npm run desktop to use it.",
      );
  }
}
