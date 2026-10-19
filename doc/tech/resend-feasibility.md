# Resend feasibility and integration contract

Evidence reviewed: 2026-10-14 · Account access: not tested

## Feasibility conclusion

A personal client is feasible using Resend's HTTPS APIs for outbound and inbound records, with a local database providing desktop mailbox behavior. The design does not require an IMAP server, POP mailbox, or publicly reachable desktop webhook endpoint. It must not assume protocol compatibility with conventional mail servers.

Domain/DNS readiness is accepted from the user. Credential permissions, provider storage settings, historical availability, plan limits, and private-beta access remain account-specific checks. No application design can recreate mail content that the provider no longer retains and the client never downloaded.

## Verified provider facts and proposed use

| Capability | Primary evidence | Design consequence |
| --- | --- | --- |
| Send mail | [Send Email](https://resend.com/docs/api-reference/emails/send-email) documents recipients, bodies, custom headers, attachments, and a response ID. Attachment limit is 40 MB per email after Base64 encoding. Idempotency keys expire after 24 hours. | Use a durable Outbox; treat response ID as acceptance, not delivery. Validate encoded payload size conservatively. |
| Discover incoming mail | [List Received Emails](https://resend.com/docs/api-reference/emails/list-received-emails) lists team-received records. | Poll and paginate without a hosted webhook receiver. Do not treat an address filter as authorization. |
| Read incoming content | [Retrieve Received Email](https://resend.com/docs/api-reference/emails/retrieve-received-email) supplies HTML/text, headers, message ID, raw-download metadata, and attachment metadata. Inline content may use data URIs or CID references. | Persist downloaded content; honor actual HTML format; keep raw export separate from reconstructed content. |
| Incoming attachments | [Retrieve received attachment](https://resend.com/docs/api-reference/emails/retrieve-received-email-attachment) returns a download URL and expiry. | Store bytes or stable IDs, not an expired URL as the attachment. Refresh metadata when necessary. |
| Discover outgoing mail | [List Sent Emails](https://resend.com/docs/api-reference/emails/list-emails) exposes sent records. | Import retrievable sends made through the Dashboard or other integrations, and reconcile by provider ID. |
| Threaded replies | [Reply guide](https://resend.com/docs/dashboard/receiving/reply-to-emails) uses In-Reply-To and References. | Use RFC message IDs, not Resend resource IDs, for conversation headers. |
| Catch-all receiving | [Receiving introduction](https://resend.com/docs/dashboard/receiving/introduction) describes receiving for any address at a configured receiving domain. | Configure identities and unmatched-address views; separate address selection from mailbox security. |
| Limits and quotas | [Usage limits](https://resend.com/docs/api-reference/rate-limit) documents team-wide request limits, rate headers, Retry-After, and quota errors. It currently states 10 requests/second by default and that sent and received mail count toward email quotas. | Share a limiter across connection jobs, adapt to headers, distinguish rate pressure from exhausted quotas, and recheck actual account limits. |
| Stored incoming mail | [Managing received emails](https://resend.com/docs/dashboard/receiving/manage-emails) describes API retrieval and provider storage. | Retrieve existing available history, but verify account retention and storage policy before promising full historical recovery. |
| Managed inbox | [Create Inbox](https://resend.com/docs/api-reference/inboxes/create-inbox) explicitly states private beta and a changeable response contract. | No default dependency; test access and migration semantics before enabling an optional adapter. |
| Managed organization | [Update Thread](https://resend.com/docs/api-reference/inboxes/update-thread) documents read state, folder moves, and label operations. | Future server-managed state is possible; do not invent these writes for general email records. |

## Baseline API surface

Use the official [API index](https://resend.com/docs/llms.txt) to revalidate endpoint names and schemas when implementing. Initial adapter operations:

| Operation | HTTP route |
| --- | --- |
| Submit new/reply/forward email | `POST /emails` |
| List incoming | `GET /emails/receiving` |
| Retrieve incoming | `GET /emails/receiving/{email_id}` |
| List incoming attachments | `GET /emails/receiving/{email_id}/attachments` |
| Retrieve incoming attachment | `GET /emails/receiving/{email_id}/attachments/{attachment_id}` |
| List outgoing | `GET /emails` |
| Retrieve outgoing | `GET /emails/{email_id}` |

Wrap these in a small typed Rust adapter rather than coupling domain logic to SDK objects. Parse documented fields with optional/null handling; tolerate additive fields and unknown delivery states. Domain inspection is an optional onboarding operation; email capabilities must be probed independently.

Credentials are user-owned secrets. [Resend's key-management guidance](https://resend.com/docs/dashboard/api-keys/introduction) recommends confidentiality and separate keys for isolation. Use a dedicated key with the minimum permissions actually available for both sending and reading. Do not claim a sending-only key can read mail. Record the tested permission configuration during the provider spike. If reading requires broad team access, disclose that boundary before connection. Never ship a developer's shared key in the app.

## Two supported architectural directions

**Baseline: personal client with local state.** Direct API access, local drafts/labels/read state, periodic polling, and no backend. This fits the stated initial workspace and avoids operating another service. It does not synchronize local organization between installations.

**Future: managed mailbox or relay.** Reevaluate the private-beta Inboxes API if server-managed state becomes necessary and accessible. Separately, a hosted relay can receive verified webhooks, hydrate and persist content, and serve authenticated clients. These solve different problems: notification transport alone does not synchronize local drafts or labels.

A relay requires public HTTPS, webhook signature verification over the original request body, replay protection, durable event deduplication, tenant authorization, a persistent job queue, retention policy, monitoring, and operational ownership. It is justified by always-on ingestion or multi-user requirements, not by the existence of inbound webhooks. An event payload must not be assumed to contain full mail content. No hosted provider is selected or provisioned by these documents.

## Required account-level experiments

1. Confirm list/retrieve permissions using a dedicated test credential. Record whether different scoped keys are necessary and which operations expose team-wide data.
2. Send a controlled message, receive a reply, and verify recipients, headers, Unicode, delivery-status retrieval, and attachment/raw exports.
3. Establish list ordering, page limits, cursor direction, simultaneous-arrival behavior, and whether any server filtering can safely reduce requests. Do not assume timestamps are unique or that a list is a snapshot.
4. Check oldest accessible sent/received records, body availability, storage-disable behavior, attachment expiry/refresh, raw availability, and retention terms for the actual plan. Dashboard visibility alone is not a durable archive guarantee.
5. Verify repeated identical send requests with one idempotency key, payload-mismatch errors, and timeout reconciliation. Never test ambiguous retry behavior against real correspondents.
6. If beta access exists, test historical association of general received/sent records with inbox threads, drafts, labels, and IDs. Do not assume creating an inbox imports old mail or preserves local thread IDs.

Save redacted results as implementation evidence. Any failed required operation is a scope or integration blocker to resolve before building the full UI.
