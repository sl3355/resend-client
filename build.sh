#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  printf '%s\n' 'Usage: ./build.sh [Tauri build options]' \
    'Build optimized desktop packages for this platform.' \
    'Example: ./build.sh --bundles app' \
    'Default output: src-tauri/target/release/bundle/'
  exit 0
fi

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/scripts/common.sh"
resend_prepare
exec npm run desktop:build -- "$@"
