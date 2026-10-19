#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  printf '%s\n' 'Usage: ./prod.sh [application arguments]' \
    'Build the optimized desktop app for this machine and run it without a dev server.'
  exit 0
fi

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/scripts/common.sh"
resend_prepare

resend_host="$(rustc -vV | sed -n 's/^host: //p')"
resend_target_dir="$(cargo metadata --manifest-path src-tauri/Cargo.toml --no-deps --format-version 1 |
  node -e 'let input=""; process.stdin.on("data", chunk => input += chunk); process.stdin.on("end", () => console.log(JSON.parse(input).target_directory))')"

# An explicit host target also makes the executable path predictable when
# CARGO_BUILD_TARGET is set. Cargo metadata respects CARGO_TARGET_DIR.
npm run desktop:build -- --no-bundle --target "$resend_host" -- --locked
resend_binary="$resend_target_dir/$resend_host/release/resend-client"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) resend_binary="${resend_binary}.exe" ;;
esac
exec "$resend_binary" "$@"
