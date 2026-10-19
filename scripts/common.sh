#!/usr/bin/env bash
# Shared setup for the root-level desktop commands.

RESEND_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RESEND_ROOT"

resend_node_supported() {
  command -v node >/dev/null 2>&1 &&
    node -e 'const [major, minor] = process.versions.node.split(".").map(Number); process.exit(major > 22 || (major === 22 && minor >= 12) ? 0 : 1)'
}

resend_prepare() {
  if ! resend_node_supported && command -v fnm >/dev/null 2>&1; then
    # Only changes this script's environment; never changes the user's default.
    eval "$(fnm env --shell bash)"
    for resend_version in "$(cat .node-version)" 24 22; do
      if fnm use "$resend_version" >/dev/null 2>&1 && resend_node_supported; then
        break
      fi
    done
  fi

  if ! resend_node_supported; then
    printf '%s\n' 'Node.js 22.12+ is required (Node 24 recommended).' \
      'Install a compatible version and rerun this command.' >&2
    exit 1
  fi
  if ! command -v npm >/dev/null 2>&1; then
    printf '%s\n' 'npm is required. Install it with Node.js and rerun this command.' >&2
    exit 1
  fi
  if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
  fi
  if ! command -v cargo >/dev/null 2>&1 || ! command -v rustc >/dev/null 2>&1; then
    printf '%s\n' 'Rust 1.99.1 and the Tauri platform prerequisites are required. See README.md.' >&2
    exit 1
  fi

  # Reinstall only on first use, a changed lockfile, or a changed Node major.
  local resend_stamp
  resend_stamp="$(cksum < package-lock.json):$(node -p 'process.versions.node.split(".")[0]')"
  if [ ! -x node_modules/.bin/tauri ] ||
    [ ! -f node_modules/.resend-dependencies ] ||
    [ "$(cat node_modules/.resend-dependencies)" != "$resend_stamp" ]; then
    printf '%s\n' 'Installing locked frontend dependencies…'
    npm ci
    printf '%s\n' "$resend_stamp" > node_modules/.resend-dependencies
  fi
}
