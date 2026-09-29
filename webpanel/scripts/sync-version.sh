#!/usr/bin/env bash
# Aligne la version du crate `nitrite` de l'agent sur celle de NiTriTe
# (src-tauri/Cargo.toml) : le backend affiche env!("CARGO_PKG_VERSION").
#   sync-version.sh          met a jour agent/core/Cargo.toml
#   sync-version.sh --check  echoue si les versions different (CI)
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
upstream="${NITRITE_UPSTREAM:-$here/..}"
[ -f "$upstream/src-tauri/Cargo.toml" ] || upstream="$here/upstream"
want="$(grep -m1 '^version' "$upstream/src-tauri/Cargo.toml" | cut -d'"' -f2)"
core="$here/agent/core/Cargo.toml"
have="$(grep -m1 '^version' "$core" | cut -d'"' -f2)"
if [ "${1:-}" = "--check" ]; then
  if [ "$want" != "$have" ]; then
    echo "Version de l'agent ($have) differente de NiTriTe ($want) : lancez webpanel/scripts/sync-version.sh" >&2
    exit 1
  fi
  echo "Versions alignees : $want"
  exit 0
fi
sed -i.bak "0,/^version = \".*\"/s//version = \"$want\"/" "$core" && rm -f "$core.bak"
echo "agent/core : $have -> $want"
