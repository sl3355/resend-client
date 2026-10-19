# Technical documentation

Draft baseline · 2026-10-14

| Document | Purpose |
| --- | --- |
| [Framework evaluation](./framework-evaluation.md) | Compare desktop stacks and select a provisional direction. |
| [Resend feasibility](./resend-feasibility.md) | Separate verified provider contracts, proposed behavior, and account-specific unknowns. |
| [Architecture](./architecture.md) | Define boundaries, local data, synchronization, sending, and security. |
| [Delivery and validation](./delivery-and-validation.md) | Define implementation phases, native builds, verification, and release gates. |

Recommendation: Tauri 2 + React + TypeScript, Rust for privileged services, SQLite for local state, and direct HTTPS access to Resend. No hosted backend is required for the baseline. Electron is the fallback after a time-boxed prototype if system WebViews cannot meet the email reader/editor or integration requirements.

These documents prescribe a design; no application or live-account validation has been performed. Linked primary documentation was reviewed on 2026-10-14. Provider APIs and framework support must be rechecked when implementation begins. Dependency versions should be selected, pinned, and tested at that point rather than inferred from examples in documentation.

Research limitation: the Stripe Directory skill was consulted for the named provider. Its CLI is absent and its web endpoint could not be read by the available browser tool. No directory results or vendor ranking are claimed; the evaluation uses the official provider and framework documentation linked in each document. Nothing was installed or provisioned.
