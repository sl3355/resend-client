import DOMPurify from "dompurify";
import type { Draft, Identity, Mail } from "./types";

export const uid = () => crypto.randomUUID();
export const emailAddress = (s: string) =>
  (s.match(/<([^>]+)>/)?.[1] || s).trim();
export const personName = (s: string) =>
  s.includes("<")
    ? s.slice(0, s.indexOf("<")).replaceAll('"', "").trim()
    : s.split("@")[0];
export const initials = (s: string) =>
  personName(s)
    .split(/[\s._-]+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((p) => p[0])
    .join("")
    .toUpperCase();
export const addressKey = (s: string) => {
  const a = emailAddress(s);
  const index = a.lastIndexOf("@");
  return index < 0 ? a : a.slice(0, index) + a.slice(index).toLowerCase();
};
export function blankDraft(identities: Identity[]): Draft {
  return {
    id: uid(),
    revision: 1,
    identity_id: identities[0]?.id || "",
    to: "",
    cc: "",
    bcc: "",
    subject: "",
    text: "",
    html: "",
    attachments: [],
    headers: {},
    updated_at: new Date().toISOString(),
  };
}
export function respond(
  mail: Mail,
  mode: "reply" | "replyAll" | "forward",
  identities: Identity[],
): Draft {
  const d = blankDraft(identities);
  const own = new Set(identities.map((i) => addressKey(i.email)));
  const identity = identities.find((i) =>
    [...mail.to, ...mail.cc, mail.from].some(
      (a) => addressKey(a) === addressKey(i.email),
    ),
  );
  if (identity) d.identity_id = identity.id;
  if (mode === "forward") {
    d.subject = /^fwd?:/i.test(mail.subject)
      ? mail.subject
      : `Fwd: ${mail.subject}`;
    d.text = `\n\n---------- Forwarded message ----------\nFrom: ${mail.from}\nDate: ${mail.created_at}\nSubject: ${mail.subject}\nTo: ${mail.to.join(", ")}\n\n${mail.text || htmlToText(mail.html)}`;
  } else {
    const target = mail.reply_to.length ? mail.reply_to : [mail.from];
    const seen = new Set<string>();
    const unique = (items: string[], excludeOwn: boolean) =>
      items.filter((a) => {
        const key = addressKey(a);
        if (seen.has(key) || (excludeOwn && own.has(key))) return false;
        seen.add(key);
        return true;
      });
    d.to = unique(target, mode === "replyAll").join(", ");
    if (mode === "replyAll")
      d.cc = unique([...mail.to, ...mail.cc], true).join(", ");
    d.subject = /^re:/i.test(mail.subject)
      ? mail.subject
      : `Re: ${mail.subject}`;
    if (mail.message_id) {
      d.headers["In-Reply-To"] = mail.message_id;
      d.headers.References = [mail.headers.references, mail.message_id]
        .filter(Boolean)
        .join(" ")
        .slice(-8000);
    }
    d.text = `\n\nOn ${new Date(mail.created_at).toLocaleString()}, ${mail.from} wrote:\n${(
      mail.text || htmlToText(mail.html)
    )
      .split("\n")
      .map((l) => `> ${l}`)
      .join("\n")}`;
  }
  d.html = `<p></p><blockquote>${escapeHtml(d.text.trim()).replaceAll("\n", "<br>")}</blockquote>`;
  return d;
}
export const escapeHtml = (s: string) =>
  s
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
export function htmlToText(html: string): string {
  const el = document.createElement("div");
  el.innerHTML = sanitize(html);
  return el.textContent || "";
}
const tags = [
  "p",
  "br",
  "div",
  "span",
  "strong",
  "b",
  "em",
  "i",
  "u",
  "s",
  "ul",
  "ol",
  "li",
  "blockquote",
  "pre",
  "code",
  "h1",
  "h2",
  "h3",
  "h4",
  "hr",
  "table",
  "thead",
  "tbody",
  "tr",
  "td",
  "th",
];
export function sanitize(html: string): string {
  if (html.length > 2_000_000)
    return "<p>This message exceeds the safe HTML size limit. Use plain text.</p>";
  return DOMPurify.sanitize(html, {
    ALLOWED_TAGS: tags,
    ALLOWED_ATTR: [],
    ALLOW_DATA_ATTR: false,
    ALLOW_ARIA_ATTR: false,
    FORBID_TAGS: [
      "svg",
      "math",
      "form",
      "style",
      "script",
      "iframe",
      "object",
      "embed",
      "img",
      "a",
    ],
  });
}
export function readerDocument(html: string, dark: boolean): string {
  return `<!doctype html><html><head><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src 'none'; form-action 'none'; base-uri 'none';"><meta name="referrer" content="no-referrer"><style>html{color-scheme:${dark ? "dark" : "light"}}body{font:15px/1.8 -apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;color:${dark ? "#dbdcd8" : "#3e4140"};margin:0;overflow-wrap:anywhere}p{margin:0 0 18px}h1,h2,h3{line-height:1.3}blockquote{border-left:2px solid #ccc;margin:20px 0;padding-left:18px;color:#80837d}pre{white-space:pre-wrap}table{max-width:100%;border-collapse:collapse}td,th{padding:8px;border:1px solid #bbb}hr{border:0;border-top:1px solid #ddd;margin:25px 0}</style></head><body dir="auto">${sanitize(html)}</body></html>`;
}
export const formatBytes = (n: number) =>
  n < 1024
    ? `${n} B`
    : n < 1048576
      ? `${(n / 1024).toFixed(1)} KB`
      : `${(n / 1048576).toFixed(1)} MB`;
export function relatedMessages(mail: Mail, messages: Mail[]): Mail[] {
  // References link messages; a shared subject alone is never sufficient.
  const refs = (m: Mail) =>
    [
      m.message_id,
      ...((m.headers.references || "").match(/<[^>]+>/g) || []),
      ...((m.headers["in-reply-to"] || "").match(/<[^>]+>/g) || []),
    ].filter(Boolean);
  const linked = new Set(refs(mail));
  const ids = new Set([mail.id]);
  for (let pass = 0; pass < 20; pass++) {
    let changed = false;
    for (const m of messages)
      if (!ids.has(m.id) && refs(m).some((x) => linked.has(x))) {
        ids.add(m.id);
        refs(m).forEach((r) => linked.add(r));
        changed = true;
      }
    if (!changed) break;
  }
  return messages
    .filter((m) => ids.has(m.id))
    .sort((a, b) => a.created_at.localeCompare(b.created_at));
}

export function matchesAddressScope(
  mail: Mail,
  identity: string,
  domain = "",
  includeSubdomains = false,
): boolean {
  const addresses =
    mail.direction === "incoming"
      ? [...mail.to, ...mail.cc, ...mail.bcc]
      : [mail.from];
  return (
    addresses.some((raw) => {
      const key = addressKey(raw);
      if (identity) return key === addressKey(identity);
      const host = key.slice(key.lastIndexOf("@") + 1);
      const target = domain.toLowerCase();
      return (
        host === target || (includeSubdomains && host.endsWith("." + target))
      );
    }) ||
    (!identity && !domain)
  );
}
