# Experience and acceptance criteria

Status: proposed · 2026-10-14

## Main interface

Use a familiar three-pane arrangement: accounts/identities and folders on the left, searchable message or conversation list in the center, reader on the right. Allow the reader to collapse on small windows. Show sender, subject, preview, date, unread/star/attachment indicators, and send status without relying on color alone.

Compose opens in a dedicated panel or window. Keep sender choice and To visible; expand Cc/Bcc on demand. Provide a formatting toolbar, attachment tray, draft-save indicator, and a single primary Send action. Switching sender updates the signature without overwriting user-written content.

Settings cover connection, identities/signatures, refresh/background behavior, notification privacy, appearance, local storage/backup, and updates. Do not expose provider internals unless needed to diagnose an error.

## Journeys

### Connect and verify

1. Explain that the user connects an existing Resend account and that local organization stays on this device.
2. Accept the credential in a masked field and persist it through secure storage only after a successful check.
3. Perform separate non-mutating checks for incoming and outgoing access. Explain permission failures precisely; never send a test email automatically.
4. Let the user configure exact sender addresses and display names. Offer domain verification information if readable; otherwise allow manual entry and validate on an explicitly requested test send.
5. Import the most recent 100 records per direction first, then fetch older available history incrementally. Show progress and body-download coverage. Initial import does not generate hundreds of notifications.
6. Let the user explicitly send a test message and confirm a reply arrives. Failed tests retain the draft and explain the next action.

### Read and organize

Selecting a message shows cached text immediately. A not-yet-downloaded body shows a loading indicator online or an offline explanation. Reading marks the message read locally. A conversation shows ordered inbound/outbound messages and supports expanding quoted content.

Archive and Trash provide Undo. Labels and stars update immediately after local persistence. A message arriving for an unconfigured address appears in the unmatched view rather than disappearing. The user can inspect recipient details and assign a view without changing DNS.

### Send, reply, and forward

Validate recipients and attachment size before enqueueing. Warn if subject is empty, but allow the user to proceed. Reply uses Reply-To when present, otherwise From. Reply All deduplicates visible recipients and excludes the user's configured identities; never reconstruct recipients from Bcc. The composer lets the user review the final list.

Forward quotes the source content and includes source attachments only when chosen and available. An offline send creates a visible queued item. Once a request has started, cancellation cannot promise recall. A short optional Undo Send delay is local and ends before submission.

Display “Accepted by Resend” after API acceptance. Show later delivery information when available, including bounce/failure; do not infer that a person read the message. For an ambiguous timeout show “Send outcome unknown—checking status” and explain any explicit resend action's duplication risk.

### Work offline and recover

Display an offline banner with the last successful refresh. Cached mail, local organization, and drafts remain usable. Do not show uncached attachments as available offline. After restart, restore committed drafts and queued mail; submitted attempts enter reconciliation before retry.

Offer a local backup including messages, state, drafts, and referenced attachments, excluding credentials. Restore first validates the archive and imports into a separate profile before replacement. A raw incoming .eml may be exported when available; do not call a generated outbound reconstruction the original .eml.

## Acceptance matrix

All applicable cases must pass on macOS ARM64, macOS x86-64, Windows ARM64, and Windows x86-64. Provider tests require an explicitly designated test account and recipient.

| Case | Requirements | Observable result |
| --- | --- | --- |
| A01: missing/revoked permission | P01 | Sending and receiving checks report separately; no secret appears in UI diagnostics. |
| A02: paginated import with new arrivals | P02–P03 | Available records are imported once, cursor recovery survives restart, and new mail is not skipped. |
| A03: outside-client activity | P02–P03 | Retrievable Dashboard sends and inbound mail appear after refresh; local archived/read state survives. |
| A04: hostile HTML | P04 | Scripts, forms, remote CSS/images, navigation, IPC attempts, and tracking requests are blocked. |
| A05: international correspondence | P04–P06 | Unicode addresses where supported, Unicode display names/subjects, emoji, RTL text, and long lines display without data loss. Unsupported addresses produce a clear validation error. |
| A06: reply-all | P06 | Reply-To honored, own addresses excluded, recipients deduplicated, Bcc not exposed, thread references preserved. |
| A07: draft crash | P07 | Last committed revision survives forced termination; save failures are visible. |
| A08: send crash/timeout | P08 | No automatic fresh attempt after uncertain acceptance; retry identity persists and Outbox explains uncertainty. |
| A09: rate/quota error | P03, P08 | Rate backoff respects server timing; quota exhaustion pauses affected work without losing drafts or spinning. |
| A10: attachments | P05, P10 | Size checked after encoding; expired links refreshed; unsafe filenames cannot escape destination; no auto-open. |
| A11: local Trash | P02 | Restore works; permanent local removal stays removed after sync and explains provider copy remains. |
| A12: offline/sleep/resume | P03, P10 | Cached mail works; pending work resumes safely; notifications do not imply operation while quit. |
| A13: search | P09 | Filters compose, local coverage is stated, and unfetched bodies are not falsely reported as searched. |
| A14: accessibility | P11 | Main journeys work with keyboard, VoiceOver/Narrator, 200% zoom, and high contrast. |
| A15: distribution | P12 | Native architecture verified; clean-machine installation and signed update preserve the profile. |
| A16: backup and isolation | P01, P12 | Restore preserves local data; credentials excluded; switching/removing a connection cannot leak another profile's data. |

Test performance against the targets in [requirements](./product-requirements.md), recording OS, CPU architecture, dataset, runtime versions, and network conditions. No simulated architecture counts as native release certification.
