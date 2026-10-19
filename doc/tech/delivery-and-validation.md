# Delivery, platform builds, and validation

Status: proposed · 2026-10-14

## Implementation phases

The following ranges are planning estimates for 1–2 experienced engineers with Rust/TypeScript experience and part-time product/QA support. They are not commitments and exclude signing-account acquisition and beta-access waiting. Reestimate after phase 0.

| Phase | Indicative effort | Deliverables | Exit gate |
| --- | --- | --- | --- |
| 0: feasibility | 1–2 weeks | Redacted provider experiments; native prototype on four architectures; editor/reader/security tests; measured shell resources | Required APIs work with actual permissions; native packaging feasible; Tauri retained or Electron selected. |
| 1: functional slice | 2–3 weeks | Connection, identities, local schema/migrations, received/sent list and reader, compose, draft save, durable send intent | Controlled external email and reply work; crash after submission recovers safely. |
| 2: daily-use workflows | 2–3 weeks | Local organization, conversations, search, attachments, offline mode, background refresh, backup/restore | Product acceptance A01–A13 and A16 pass in integrated builds. |
| 3: release hardening | 2–3 weeks | Accessibility, signing, native installer/update matrix, large-mailbox tuning, recovery/security regression suite | All four targets pass certification and product acceptance; documented limitations match shipping behavior. |

Indicative total: 7–11 weeks under these assumptions. Cross-platform plugin failures, Windows ARM64 hardware access, provider retention restrictions, and editor accessibility can extend it. If synchronized state is essential for v1, revisit scope and architecture before phase 1.

## Proposed source organization

```text
src/                    React application, features, and typed command clients
src-tauri/src/          Rust commands, domain services, provider, storage, security
src-tauri/migrations/   Versioned SQLite migrations
tests/fixtures/        Synthetic email, attachment, and provider-response fixtures
tests/integration/     Provider mocks and native integration scenarios
doc/prod/              Product contract
doc/tech/              Architecture and release evidence
```

This is a proposed layout, not directories or implementation already created. Start with small interfaces and fixtures before introducing multiple workspaces/packages. Lock frontend and Rust dependencies, review licenses, and record toolchain versions. Preserve the existing repository license; review dependency/distribution compatibility before release.

## Native build matrix

| Product target | Rust target | Build host | Initial artifact and certification |
| --- | --- | --- | --- |
| macOS M-series | `aarch64-apple-darwin` | macOS with Xcode tools | Signed/notarized arm64 .app in DMG; test on Apple Silicon without translation. |
| macOS Intel | `x86_64-apple-darwin` | macOS with required target/toolchain | Signed/notarized x86-64 .app in DMG; test on an actual Intel Mac. |
| Windows ARM64 | `aarch64-pc-windows-msvc` | Windows with ARM64 MSVC tools | Signed NSIS installer containing native ARM64 app; actual ARM64 device test. |
| Windows x86-64 | `x86_64-pc-windows-msvc` | Windows with x64 MSVC tools | Signed NSIS installer containing x64 app; actual x64 device test. |

Prefer platform-native build hosts; cross-compilation is not a substitute for device validation. Tauri documents [Windows ARM64 prerequisites](https://tauri.app/distribute/windows-installer/); its NSIS installer itself may run under x86 emulation while the application is native ARM64. If a fully native installer is required too, treat that as a separate packaging gate. Avoid assuming MSI has the same ARM64 installer behavior.

Ship separate Mac downloads initially. A universal Mac binary is an optional distribution simplification, supported by Tauri's [universal build instructions](https://tauri.app/distribute/app-store/); all included native dependencies must contain both slices. A universal package still needs testing on both CPU types.

The proposed certification baseline is macOS 14+ and Windows 11 24H2+ on supported hardware. Verify framework, WebView, dependency, signing, and OS-security compatibility before finalizing minimum versions. This policy does not promise support for every older system the framework can technically start on. Maintain an explicit OS test matrix as new releases arrive and ensure Intel machines can run the versions advertised.

## Release pipeline

1. Validate formatting, types, Rust compilation, relevant tests, dependency/license checks, and migration fixtures. Build from a tagged revision with lockfiles and recorded toolchain versions.
2. Build the four target artifacts in isolated jobs with explicit architecture. Inspect application binaries and linked libraries to reject accidental emulated builds or missing target dependencies.
3. Sign macOS artifacts with the appropriate Developer ID identity and notarize/staple for direct distribution; see [macOS signing](https://tauri.app/distribute/sign/macos/). Protect signing credentials in release-only CI contexts.
4. Authenticode-sign Windows application and installer through a vetted signing setup, then verify on clean machines. Signing does not guarantee a warning-free reputation experience.
5. Generate architecture-specific update artifacts and signed metadata using the [Tauri updater](https://tauri.app/plugin/updater/). Updater signatures and OS code signatures serve different checks. Require HTTPS, protect signing keys, and never route an ARM64 install to an x64 update accidentally.
6. Test install, launch, upgrade from the previous release, interrupted download, invalid signature, wrong architecture, and profile preservation. Distribute a beta channel first, then promote the same verified artifacts.

No release host or signing service is selected here. Updates can be user-initiated initially; enabling automatic downloads requires a privacy/UX decision. Update installation waits for active sends and flushes drafts. Make a recoverable database backup before migrations. Do not support binary downgrade across an incompatible schema; rollback restores an explicitly compatible profile backup.

## Test strategy

| Layer | Meaningful coverage |
| --- | --- |
| Domain/unit | Recipient parsing and Reply All exclusions, message-ID graph linking/cycles, folder semantics, send state transitions, encoded attachment size, secret/log redaction. |
| Storage/integration | Transaction recovery, idempotent page replay, unique constraints, tombstones, cursor invalidation, draft revision races, attachment atomic writes, migrations, consistent backup/restore. |
| Provider contract | Synthetic fixtures for lists/details/errors, null fields and unknown states; controlled account tests for real permissions, threading, attachments, history and status. |
| Fault injection | Timeout before/after acceptance, crash before/after DB commit, revoked keys, slow TLS, rate/quotas, disk full, corrupt file, expired URL, sleep/resume, offline queue. |
| Content security | Malicious HTML/CSS/SVG, encoded URL schemes, tracking pixels, private-network image targets, oversized nested content, fake filenames and redirects, reader-to-IPC access attempts. |
| UI/accessibility | Composer IME/paste/undo, keyboard focus, list virtualization, zoom, RTL, VoiceOver/Narrator, file dialogs, notifications and privacy. |
| Native release | Each CPU architecture, minimum/current supported OS, clean-machine runtime dependencies, signing/notarization, installer removal and update continuity. |

Use browser automation for ordinary React flows, but test real WKWebView/WebView2 behavior with native integration/manual coverage. Do not claim a desktop automation framework supports all targets until verified. Tests must never contact real correspondents or production send endpoints by default; live tests use an explicitly designated test connection and allowlisted recipients.

Performance tests use a synthetic 10,000-message corpus with varied bodies, realistic thread lengths and attachments; record startup, p95 search/list latency, idle CPU/memory, sync volume, and storage growth. Stress larger datasets separately to establish a published capacity rather than guessing it. Measure at least one representative physical device per architecture.

## Release gates and evidence

- All Must requirements have implementation evidence mapped to [A01–A16](../prod/experience-and-acceptance.md); failures are fixed or the product contract is explicitly revised before release.
- Every target has a clean installation/update test record, architecture verification, runtime/dependency versions, and accessibility result. A green cross-compile is insufficient.
- Safe-send scenarios preserve immutable intents and keys across crashes; expired-window ambiguity requires user review. Draft and backup recovery are demonstrated.
- Hostile email cannot access credentials, IPC, filesystem, or arbitrary network destinations. Critical security findings block release.
- Actual retention/permission constraints and local-only behavior are documented in onboarding. New private-beta capabilities remain disabled unless separately certified.
- Known issues, supported OS versions, resource measurements, and a recovery procedure accompany the release. No performance or parity claim exceeds the evidence.

Keep unresolved implementation decisions in a short decision log: chosen framework after spike, dependency/toolchain pins, tested credential scopes, retention findings, actual OS floor, update hosting, and backup format version. Public multi-user distribution or synchronized mailbox state triggers a fresh authorization and data-ownership review.
