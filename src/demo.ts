import type { Mail, Snapshot } from "./types";
const identity = "6e6f19df-ef1b-4333-9a6e-17b51713d980";
export function demoData(): Snapshot {
  const rows: [string, string, string, string, string[], boolean, string[]][] =
    [
      [
        "Amelia Hart <amelia@example.com>",
        "A few ideas for our next chapter",
        "Hi Alex, I’ve been thinking about what comes next for the studio. A few thoughts I wanted to share before we catch up.",
        ["studio.example"][0],
        ["Work"],
        false,
        [
          "The new direction",
          "A little more room to experiment. A little more attention to the things that matter.",
          "Hi Alex,",
          "I’ve been thinking about what comes next for the studio, and I wanted to share a few thoughts before we catch up.",
          "The work we’ve been doing lately feels like the start of something. What if we gave ourselves a little space to explore that? Fewer projects, more intention.",
          "I pulled together a few ideas for our conversation on Thursday:",
          "• A refreshed identity that feels a little more like us\n• A simpler way to share our work\n• Space for a few projects of our own",
          "No rush to make any decisions. Just something to sit with.",
          "Would love to hear what you think.",
          "Amelia",
        ],
      ],
      [
        "Noah Chen <noah@example.com>",
        "Re: The little details",
        "The latest version is looking great. I left a few small notes in the file.",
        "studio.example",
        ["Work"],
        false,
        [
          "Hi Alex,",
          "The latest version is looking great. I left a few small notes in the file.",
          "The typography is feeling much more considered now. Shall we take another look tomorrow?",
          "Thanks,\nNoah",
        ],
      ],
      [
        "Sofia Rivera <sofia@example.com>",
        "Sunday, somewhere new?",
        "Found a little spot I think you’d love. Coffee and a long walk?",
        "alex.example",
        ["Personal"],
        true,
        [
          "Hey Alex,",
          "Found a little spot I think you’d love. Coffee and a long walk this Sunday?",
          "Let me know what works for you.",
          "Sofia",
        ],
      ],
      [
        "Oliver Park <oliver@example.com>",
        "Website handoff · everything in one place",
        "Here’s everything you need for the handoff. Thanks for a wonderful collaboration.",
        "studio.example",
        ["Work"],
        true,
        [
          "Hi Alex,",
          "Here’s everything you need for the handoff. Thanks for a wonderful collaboration.",
          "I have included the project notes below. Let me know if anything needs another pass.",
          "All the best,\nOliver",
        ],
      ],
      [
        "The Paper Trail <letters@example.com>",
        "A slower kind of inspiration",
        "A small collection of things worth taking your time with this week.",
        "alex.example",
        ["Reading"],
        true,
        [
          "A slower kind of inspiration",
          "A small collection of things worth taking your time with this week.",
          "A quiet reminder that good things often take a little longer.",
        ],
      ],
      [
        "Isabel Morgan <isabel@example.com>",
        "Let’s make something good",
        "Really enjoyed our conversation yesterday. I think there’s something here.",
        "studio.example",
        ["Work"],
        true,
        [
          "Hi Alex,",
          "Really enjoyed our conversation yesterday. I think there’s something here.",
          "Would you have time for a follow-up next week?",
          "Isabel",
        ],
      ],
      [
        "Ethan Brooks <ethan@example.com>",
        "Photos from the coast",
        "Finally got around to sorting these. What a good weekend.",
        "alex.example",
        ["Personal"],
        true,
        [
          "Hey,",
          "Finally got around to sorting these. What a good weekend.",
          "We should do it again soon.",
          "Ethan",
        ],
      ],
      [
        "Maya Patel <maya@example.com>",
        "Notes for Thursday",
        "A quick agenda so we can make the most of our time.",
        "studio.example",
        ["Work"],
        true,
        [
          "Hi Alex,",
          "A quick agenda so we can make the most of our time.",
          "1. Review the concept\n2. Agree on scope\n3. Set the next milestone",
          "See you Thursday,\nMaya",
        ],
      ],
      [
        "Jonah Lee <jonah@example.com>",
        "An introduction",
        "Alex, meet Clara. I think you two should talk.",
        "alex.example",
        [],
        true,
        [
          "Alex, meet Clara.",
          "I think you two should talk. You share a love of thoughtful design and independent projects.",
          "I’ll leave you to it!",
          "Jonah",
        ],
      ],
    ];
  const messages: Mail[] = rows.map(
    ([from, subject, preview, domain, labels, read, paragraphs], i) => {
      const id = `00000000-0000-4000-8000-${String(i + 1).padStart(12, "0")}`;
      return {
        id: `incoming:${id}`,
        provider_id: id,
        direction: "incoming",
        from,
        to: [
          domain === "studio.example"
            ? "alex@studio.example"
            : "hello@alex.example",
        ],
        cc: [],
        bcc: [],
        reply_to: [],
        subject,
        text: paragraphs.join("\n\n"),
        html: paragraphs
          .map(
            (p, j) =>
              `<${i === 0 && j === 0 ? "h2" : "p"}>${p.replaceAll("\n", "<br>")}</${i === 0 && j === 0 ? "h2" : "p"}>`,
          )
          .join(""),
        created_at: new Date(Date.now() - i * 42 * 60_000).toISOString(),
        message_id: `<demo-${i}@example.com>`,
        headers: {},
        attachments: [],
        status: "received",
        folder: "inbox",
        is_read: read,
        starred: i === 0 || i === 3,
        labels,
        hydrated: true,
        links: [],
        preview,
      };
    },
  );
  return {
    profile: {
      id: "demo",
      name: "Demo workspace",
      connected: true,
      identities: [
        {
          id: identity,
          name: "Alex Morgan",
          email: "alex@studio.example",
          signature: "Alex Morgan\nIndependent designer",
        },
        {
          id: "6e6f19df-ef1b-4333-9a6e-17b51713d981",
          name: "Alex",
          email: "hello@alex.example",
          signature: "Alex",
        },
      ],
      settings: { theme: "light" },
      last_sync: new Date().toISOString(),
    },
    messages,
    drafts: [],
    outbox: [],
    pending_bodies: 0,
    history_complete: true,
    sync_error: null,
    inbox_unread: messages.filter((m) => !m.is_read).length,
    labels: ["Personal", "Reading", "Work"],
  };
}
