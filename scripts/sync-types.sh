#!/bin/bash
# Rust models → TypeScript 型定義を生成
# Usage: bash scripts/sync-types.sh
set -euo pipefail
cd "$(dirname "$0")/.."

cargo test -p alc-core --features ts-export --test export_ts -- --nocapture

echo ""
echo "✓ Types exported to ~/js/alc-app/web/app/types/generated/"
echo "  $(ls ~/js/alc-app/web/app/types/generated/ | wc -l) files"
