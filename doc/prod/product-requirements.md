# Product requirements

Status: proposed baseline · Evidence reviewed: 2026-10-14

## Purpose and users

Enable a domain owner to handle everyday correspondence through Resend in a desktop application: read incoming mail, compose, reply, forward, manage attachments, find previous conversations, and work with saved mail offline.

The initial user is the account owner who already sends and receives through the Resend Dashboard. Assume one trusted person, one Resend team/account connection, and multiple addresses across their configured domains. A Resend team is a provider account boundary; a sender identity is an address plus display name and signature; a mailbox view is a local filter. These must not be confused with separate authenticated mailboxes.

Cross-platform means the same core features and data format with platform-appropriate shortcuts, menus, dialogs, accessibility, and notifications. Native binaries are required on all four architectures; emulation alone does not satisfy the requirement.

## Provider fit and product promise

The documented [received-mail list API](https://resend.com/docs/api-reference/emails/list-received-emails) and [sent-mail list API](https://resend.com/docs/api-reference/emails/list-emails) make a directly connected desktop client feasible. Their account-level email records are inputs to the client; local folders are not assumed to be provider folders.

Resend also documents [Inboxes in private beta](https://resend.com/docs/api-reference/inboxes/create-inbox). Availability is restricted and its contract can change. The baseline must work without beta access. A later optional adapter may provide provider-managed mailbox state after account-level validation.

The first release promises familiar personal email workflows. It does not promise Gmail/Outlook feature parity, synchronized folders across devices, or indefinite provider history. See [provider feasibility](../tech/resend-feasibility.md) for the evidence and limitations.

## Scope

| ID | Priority | Requirement |
| --- | --- | --- |
| P01 | Must | Connect using a user-owned credential, verify sending and receiving access separately, and configure sender identities without editing DNS. |
| P02 | Must | Inbox, Sent, Drafts, Outbox, Archive, Trash, and All Mail views; unread indicators, stars, and local labels. |
| P03 | Must | Background refresh while the process is running, manual refresh, last successful sync time, and understandable offline/error states. |
| P04 | Must | Safe HTML and plain-text reading, conversation display, sender/recipient details, date, and attachment list. |
| P05 | Must | Compose with To/Cc/Bcc, subject, plain text and basic formatting, sender selection, signature, and multiple attachments. |
| P06 | Must | Reply, Reply All, and Forward with correct recipients and conversation headers. |
| P07 | Must | Automatically save drafts locally and recover committed drafts after a crash or restart. |
| P08 | Must | Durable Outbox with visible queued, submitting, accepted, failed, and outcome-unknown states. Never silently discard a failed send. |
| P09 | Must | Search downloaded mail by text, sender, recipient, date, subject, label, and attachment presence; state the downloaded scope. |
| P10 | Must | Read downloaded content and edit drafts offline; download and save attachments on demand. |
| P11 | Must | Keyboard navigation, screen-reader labels, zoom, high-contrast testing, light/dark mode, and native file dialogs. |
| P12 | Must | Signed release packages for all four targets, safe updates, and export/backup of local data. |
| P13 | Should | Tray/menu-bar operation, configurable notifications, address autocomplete from local correspondence, and mailto handling. |
| P14 | Later | Multiple Resend accounts, synced organization, advanced rules, scheduled sending, managed shared inboxes, and optional cloud relay. |

Excluded from the first release: arbitrary IMAP/POP accounts, calendar, tasks, marketing campaigns, bulk mail, contact-list marketing management, shared-user permissions, mobile/web clients, end-to-end encrypted mail, and an always-on background service. A personal address book must not automatically subscribe people to Resend marketing contacts.

## Mailbox semantics

- Inbox contains received messages not locally archived or trashed. Sent contains accepted outbound mail, including retrievable messages sent outside the client; acceptance and delivery are visibly different states.
- Drafts and organization are device-local in the baseline. Installing on another device retrieves available provider messages but does not restore these local changes.
- Archive removes mail from Inbox while keeping it in All Mail. Trash is reversible. Permanent local removal requires confirmation and creates a tombstone so refresh does not immediately restore the item. It does not delete the provider copy.
- Stars, labels, read state, and Trash actions apply to selected messages. Conversation bulk actions explicitly affect currently loaded members; newly received replies start unread and in Inbox.
- Exact-address filters and an unmatched/catch-all view keep multiple domain identities usable. Recipient filters are convenience views, not security boundaries.
- Offline search covers downloaded text only. Attachment contents and unfetched bodies are excluded. Show incomplete-body and incomplete-history indicators.

## Reliability and privacy requirements

The app must commit a send intent before transmitting it, preserve the same attempt identity when retrying, and distinguish an unknown outcome from a definite rejection. A timeout must not invite an immediate duplicate send. There is no guarantee of exactly-once delivery over the network.

Opening an email must not run scripts, load tracking pixels, navigate the application, read files, or access application commands. External images are blocked by default; loading them is an explicit per-message action. Attachments never execute automatically. Notification previews default to sender and subject, with an option to hide both.

Credentials remain in OS-backed secure storage and never enter logs or exports. The local mail archive contains sensitive data. The baseline uses private per-user files and relies on OS account protection and disk encryption; it must not advertise an encrypted database. No message-content telemetry is enabled by default.

## Service boundaries users must understand

Refresh and notifications require the desktop process and network. Quitting the application or sleeping the computer pauses its work; queued mail remains queued until it resumes. Background operation requires the user to enable keeping the application running.

Provider storage settings and retention may limit historical recovery. Downloaded content remains available until the user removes it, but local storage is not a backup. Uninstall and account-disconnect flows must explain what is retained and offer export before removal.

An API credential may authorize access beyond a selected address. The initial personal-user model accepts that boundary; distribution to employees or unrelated mailbox users requires a separate authorization design before release.

## Success measures and proposed targets

Targets are engineering acceptance goals, not benchmarks already achieved:

| Measure | Target and conditions |
| --- | --- |
| First successful use | Connect, choose an identity, receive a test email, and reply within 10 minutes, excluding credential creation and provider delays. |
| Incoming visibility | New mail visible within 60 seconds while online, app running, account below limits, and provider record available. |
| Draft durability | Save within 2 seconds after idle; navigate/close flushes pending edits or reports save failure. |
| Local interaction | p95 list/search response below 300 ms on 10,000 downloaded messages, excluding network and first indexing. |
| Startup | Usable cached Inbox within 3 seconds on representative SSD hardware for every target. |
| Resource use | Idle CPU below 1% averaged over 5 minutes without sync; proposed private-memory budget 250 MB with one ordinary reader open. Validate before committing. |
| Safety | No credential exposure or privileged execution in the malicious-email test suite; no silent message loss in crash/retry tests. |

## Delivery sequence and unresolved decisions

First prove provider access and secure desktop operation; then build a single-account send/receive slice; add organization, offline use, search, and recovery; finally harden and distribute on all targets. Detailed gates are in the [delivery plan](../tech/delivery-and-validation.md).

Working assumptions that can be revised without blocking documentation: personal use first, English interface initially, direct API connection, local state, direct-download distribution, and no hosted service. Before implementation baseline is finalized, confirm oldest required OS versions, expected mailbox size, backup destination, whether multi-device folder/draft sync is essential, and whether beta Inbox access exists. Proposed initial certification is macOS 14+ and Windows 11 24H2+ on supported hardware; this is a product policy, not the framework's technical minimum.
