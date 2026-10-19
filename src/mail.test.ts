import { describe, expect, it } from "vitest";
import { demoData } from "./demo";
import {
  addressKey,
  relatedMessages,
  respond,
  sanitize,
  readerDocument,
} from "./mail";

describe("correspondence semantics", () => {
  it("honors Reply-To and preserves RFC thread headers", () => {
    const s = demoData(),
      mail = s.messages[0];
    mail.reply_to = ["Support <support@example.com>"];
    mail.headers.references = "<parent@example.com>";
    const draft = respond(mail, "reply", s.profile.identities);
    expect(draft.to).toBe("Support <support@example.com>");
    expect(draft.headers["In-Reply-To"]).toBe(mail.message_id);
    expect(draft.headers.References).toBe(
      `<parent@example.com> ${mail.message_id}`,
    );
    expect(draft.identity_id).toBe(s.profile.identities[0].id);
  });
  it("reply all deduplicates visible recipients, removes own identities, and excludes Bcc", () => {
    const s = demoData(),
      m = s.messages[0];
    m.reply_to = ["a@example.com"];
    m.to = ["a@example.com", "alex@studio.example", "other@example.com"];
    m.cc = ["other@example.com", "hello@alex.example", "third@example.com"];
    m.bcc = ["hidden@example.com"];
    const d = respond(m, "replyAll", s.profile.identities);
    expect(d.to).toBe("a@example.com");
    expect(d.cc).toBe("other@example.com, third@example.com");
    expect(d.bcc).toBe("");
    expect(JSON.stringify(d)).not.toContain("hidden@example.com");
  });
  it("preserves local-part case and normalizes only domains", () => {
    expect(addressKey("Name <Me@EXAMPLE.COM>")).toBe("Me@example.com");
    expect(addressKey("me@example.com")).not.toBe(addressKey("Me@example.com"));
  });
  it("forwards source text without automatically attaching files", () => {
    const s = demoData(),
      m = s.messages[0];
    m.attachments = [
      {
        id: "attachment",
        filename: "private.pdf",
        content_type: "application/pdf",
        size: 100,
        cached: false,
      },
    ];
    const d = respond(m, "forward", s.profile.identities);
    expect(d.subject).toMatch(/^Fwd:/);
    expect(d.text).toContain(m.text);
    expect(d.attachments).toEqual([]);
    expect(d.to).toBe("");
  });
  it("links by references, including late parents, without merging subjects", () => {
    const s = demoData(),
      [a, b, c] = s.messages;
    b.subject = a.subject;
    c.subject = a.subject;
    b.headers["in-reply-to"] = a.message_id;
    expect(
      relatedMessages(b, [c, b, a])
        .map((m) => m.id)
        .sort(),
    ).toEqual([a.id, b.id].sort());
  });
  it("bounds cyclic conversation graphs", () => {
    const s = demoData(),
      [a, b] = s.messages;
    a.headers.references = b.message_id;
    b.headers.references = a.message_id;
    expect(relatedMessages(a, [a, b])).toHaveLength(2);
  });
});
describe("opaque email reader", () => {
  it("removes active content, URLs, CSS, images and namespace tricks", () => {
    const safe = sanitize(
      '<script>alert(1)</script><svg><a xlink:href="javascript:x()">bad</a></svg><form action="ipc:"><input></form><img src="https://tracking.example/x"><p style="background:url(https://track)">Hello 世界</p><iframe src="file:///private"></iframe><a href="https://example.com">Link</a>',
    );
    expect(safe).toContain("Hello 世界");
    for (const value of [
      "<script",
      "<svg",
      "<form",
      "<input",
      "<img",
      "<iframe",
      "href=",
      "src=",
      "style=",
    ])
      expect(safe).not.toContain(value);
  });
  it("provides a restrictive independent CSP", () => {
    const doc = readerDocument("<p>Test</p>", false);
    expect(doc).toContain("default-src 'none'");
    expect(doc).toContain("form-action 'none'");
    expect(doc).not.toContain("<script");
  });
});
