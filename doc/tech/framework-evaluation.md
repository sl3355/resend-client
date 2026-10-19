# Cross-platform framework evaluation

Status: recommended direction, pending prototype · 2026-10-14

## Decision

Choose **Tauri 2 + React + TypeScript + Rust + SQLite** for the first prototype and planned product. The workload is a persistent desktop UI with substantial HTML email content, local search, small background network jobs, and sensitive credentials. A web UI supports familiar reader/composer behavior; a narrow Rust service layer owns network access, data persistence, and OS integration.

This is an engineering judgment under the assumption that the team can maintain Rust and TypeScript. It is not a measured claim that Tauri is always faster or more secure than alternatives. Native architecture support, safe content rendering, editor behavior, and packaging must be proved before committing the implementation baseline.

## Alternatives

| Option | Four-target fit | Benefits for this product | Costs and risks | Disposition |
| --- | --- | --- | --- | --- |
| Tauri 2 + React/TS + Rust | macOS arm64/x86-64; Windows aarch64/x86-64 MSVC targets | OS WebView, shared web UI, privileged Rust core, capability-scoped commands | Two languages; WKWebView/WebView2 differences; plugins and installers need native testing | Preferred, subject to prototype gates. |
| Electron + React/TS | Official binaries cover both architectures on macOS and Windows | Bundled Chromium gives more consistent HTML/editor behavior; common JS/TS environment | Ships browser/runtime; expected larger installation and memory footprint; native modules add architecture work | Best fallback, especially for a TS-only team. Measure resource cost. |
| Flutter + Dart | Current platform matrix lists all four; Intel Mac support is being phased out | Strong custom UI model and potential future mobile reuse | Email HTML still needs rendering integration; rich editing/desktop plugin parity needs proof; Intel roadmap conflicts with long-lived requirement | Do not select while Intel support lifetime is unresolved. |
| Avalonia + C#/.NET | Current support table lists required architectures with OS-dependent tiers | Good fit for an experienced .NET desktop team; structured desktop controls | HTML reader integration and editor need additional evaluation; support tiers constrain older OS policy | Viable alternate if .NET expertise outweighs web UI reuse. |
| Separate Swift/AppKit and Windows-native clients | Architecture feasibility depends on separate platform toolchains | Maximum control over each OS experience | Two UI implementations and release tracks; synchronization behavior can diverge | Too costly for assumed small initial team. |

Primary support evidence: [Tauri Windows installers and ARM64 targets](https://tauri.app/distribute/windows-installer/), [Tauri universal macOS build](https://tauri.app/distribute/app-store/), [Electron supported binaries](https://github.com/electron/electron), [Flutter platform matrix and Intel notice](https://docs.flutter.dev/reference/supported-platforms), and [Avalonia platform tiers](https://docs.avaloniaui.net/docs/supported-platforms). Native plugins must match the target even when the shell supports it.

## Decision rationale

For this product, compare HTML reading/editing, all four native targets, security boundaries, desktop integration, maintenance capacity, and operating cost before cosmetic differences. Both Tauri and Electron are strong contenders because email content is already HTML. Tauri's system WebView avoids bundling a separate browser engine; that suggests a smaller shell but does not establish an application resource benchmark.

Tauri uses WKWebView on macOS and WebView2 on Windows. Their versions and update paths differ, so common web code does not ensure identical behavior. See [WebView versions](https://tauri.app/reference/webview-versions/). Electron's bundled runtime reduces that variation but still needs a secure reader boundary; its [security guidance](https://www.electronjs.org/docs/latest/tutorial/security) calls for sandboxing, isolation, and restricted privileged access.

Do not choose Flutter just to enable hypothetical mobile support. Do not choose separate native clients without a requirement that justifies duplicated UI work. Avalonia deserves reconsideration if the implementation team is substantially stronger in C# than Rust/TypeScript.

## Proposed components

| Concern | Proposed implementation | Evaluation required |
| --- | --- | --- |
| Shell | Tauri 2, narrowly enabled native integrations | Dialogs, notifications, tray, window lifecycle, and update behavior on all targets. |
| UI | React + TypeScript + Vite, accessible components and virtualized message list | Keyboard/screen-reader behavior and WebView compatibility. |
| Composer | ProseMirror-based editor; evaluate Tiptap core with only necessary extensions | IME, paste sanitization, links, undo, selection, license review; emit restricted email HTML and plain text. |
| Services | Rust async tasks; typed HTTPS client via reqwest; serde for DTOs | Contract tests, cancellation, proxies/TLS, safe response size limits. |
| Persistence | SQLite via rusqlite, migrations, FTS5 | Bundled-library builds on four targets; crash durability and indexing. |
| Secrets | Rust keyring integration with explicit macOS Keychain and Windows credential backend selection | Packaging, permissions, secure deletion, locked-store errors. |
| HTML handling | Maintained HTML sanitizer plus isolated sandboxed reader | Sanitizer is only one layer; hostile-content suite is mandatory. |
| Validation | Rust tests, TypeScript unit/component tests, browser UI tests, native integration/manual accessibility checks | Browser tests cannot certify actual WKWebView or OS integrations. |

The frontend receives typed domain commands, not SQL, general HTTP, shell execution, or filesystem access. A React email-template library is not a general user-facing email composer. Avoid Node sidecars unless a demonstrated requirement outweighs packaging complexity.

Candidate libraries are implementation proposals rather than verified compatibility or license claims. Before adding them, inspect official releases, licenses, dependencies, security status, and native architecture availability; pin tested versions and commit lockfiles.

## Prototype gates and fallback

In a proposed 5–8 working-day spike, produce native builds on all four targets and exercise a rich composer, a hostile HTML reader, secure credential storage, SQLite persistence/search, native notification, and signed update feasibility. Use representative incoming mail with tables, CID images, large quoted threads, unusual encodings, and malformed markup.

Keep Tauri if these pass with reasonable implementation effort and the measured resource targets are achievable. Switch to Electron if required reader/editor behavior remains materially broken across WebViews or native plugin gaps block a required architecture. Reuse React UI, provider contracts, data semantics, and test fixtures; budget explicitly for replacing the Rust integration layer or packaging it as a sidecar. That migration is not free.

If Electron is selected, maintain a supported release line, disable Node integration in mail content, enable context isolation and sandboxing, expose minimal validated preload commands, and test every native dependency on ARM64. Supporting an old OS must not require indefinitely freezing a vulnerable runtime.
