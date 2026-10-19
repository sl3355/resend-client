# Resend Client

A personal desktop email client for your existing Resend account. Built with Tauri 2, React, TypeScript, Rust, and SQLite, following the product and architecture documents in [`doc/`](doc/).

Version 0.1 is a working **developer preview**. Real email access runs in the desktop app; the browser preview uses clearly marked synthetic mail and cannot send messages or accept credentials. This repository is not affiliated with Resend.

## Run

Install Node 24 (minimum 22.12), npm, Rust 1.99.1, and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/). On macOS, install Xcode Command Line Tools. On Windows, install the appropriate MSVC build tools and WebView2.

```sh
./dev.sh       # Desktop development with hot reload
./prod.sh      # Build and run the optimized desktop app
./build.sh     # Build desktop packages for the current platform
```

These executable Bash scripts work from any working directory. They install locked npm dependencies on first use and when the lockfile or Node major changes. If your shell uses an older Node version, they try compatible versions already installed with `fnm`; they do not change your default version. Rust and native platform prerequisites must already be installed.

`prod.sh` builds the current source before launching, without a dev server or installer. `build.sh` creates packages without launching; macOS outputs are under `src-tauri/target/release/bundle/`. Extra arguments pass through to Tauri for development/build commands, or to the application for production launch. Each script supports `--help`.

```sh
./build.sh --bundles app   # macOS .app only
```

For a browser demo only:

```sh
npm run dev
```

Open `http://127.0.0.1:1420`. Browser demo state uses localStorage and is separate from the native SQLite profile. No API keys belong in environment files or frontend code.

## Connect your mail

1. Open Settings → Connection in the desktop app and enter a dedicated Resend API key. The app separately probes incoming history and sent history without sending mail. Both must be accessible before it stores the key in macOS Keychain or Windows Credential Manager.
2. Add your exact sender addresses, display names, and signatures under Sender identities. Use addresses on your existing configured domains. This app does not change DNS or create domains.
3. Refresh. The app imports recent incoming/outgoing metadata first and continues available history with saved cursors. Bodies download in bounded background batches; selecting an uncached message downloads it immediately.
4. Explicitly compose a message to a test recipient you control and reply to it. No test message is sent automatically. History access does not independently prove send permission or domain verification.

An API key may expose team-wide mail. Address views are local filters, not separate authenticated mailboxes. This version supports one active personal account. Changing credentials starts a separate profile; export before disconnecting. Old profiles remain in private app storage. Restoring a backup opens another disconnected profile; reconnect it only to its original Resend account.

## Included

- Three-pane mailbox; Inbox, Starred, Sent, Drafts, Outbox, Archive, Trash, All Mail, per-address and unmatched views.
- Safe HTML/plain text reader with an opaque, script-free sandbox, blocked remote content, explicit external-link destinations, recipient details, and reference-based conversations among loaded messages.
- Rich-text composition, To/Cc/Bcc, sender selection, signatures, native file attachments, Reply, Reply All, and Forward. Forwarding does not automatically include source attachments; save and attach chosen files explicitly.
- SQLite draft autosave and revision protection; local read/star/label/folder state; Trash Undo and permanent local removal tombstones.
- Durable Outbox with immutable payloads and persisted idempotency identities, conservative same-attempt retries, and explicit outcome-unknown/review states. Acceptance means “Accepted by Resend,” not delivery. Recent pending sent records and selected outbound mail refresh delivery status.
- Offline cached reading/drafts; manual and process-running background refresh; download-on-demand attachments with native Save As.
- FTS5 downloaded-text search, composable filters, 100-message list pages, light/dark/system appearance, keyboard shortcuts, and modal focus containment.
- Versioned checksummed JSON backups including cached files, excluding credentials. Restore validates and imports a separate profile, and pauses restored send attempts for review.

Search examples:

```text
orchid from:amelia subject:"next chapter"
to:you@example.com label:Work after:2026-10-01 before:2026-11-01
has:attachment is:unread
```

Search excludes attachment contents and undownloaded bodies. Local state does not sync across devices. Refresh stops when you quit or sleep the computer. The local database and backups contain sensitive, unencrypted mail; private OS files and disk encryption provide protection.

## Validate and build

```sh
npm run build
npm test
npm run test:e2e
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
npm run desktop:build -- --bundles app   # macOS developer .app
```

Local browser tests use installed Google Chrome; CI uses Playwright Chromium. Default tests use local fixtures/mock HTTP servers and never contact Resend or real correspondents. Dependency lockfiles are committed. The build workflow targets macOS ARM64/Intel and Windows ARM64/x64 on matching hosts; it creates unsigned preview artifacts, not certified releases.

See [`doc/tech/implementation-status.md`](doc/tech/implementation-status.md) for verified evidence, limits, architecture decisions, and remaining release gates. Signing, notarization, safe signed updates, live-account experiments, assistive-technology/device testing, and four-platform installation certification are required before public release.
