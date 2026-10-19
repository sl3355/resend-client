#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  printf '%s\n' 'Usage: ./dev.sh [Tauri dev options]' \
    'Run the desktop app with frontend hot reload and Rust rebuilds.'
  exit 0
fi

source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/scripts/common.sh"
resend_prepare
exec npm run desktop -- "$@"
