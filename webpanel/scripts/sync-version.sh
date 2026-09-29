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
# Versions des dependances : celles de NiTriTe (hors tauri*, remplacees par
# les cales) doivent se retrouver a l'identique dans l'agent (core ou
# workspace), sinon le backend compile ici contre d'autres API.
dep_versions() {
  sed -n -E 's/^([a-z0-9_-]+) *= *(\{ *version *= *)?"([^"]+)".*/\1 \3/p' "$@"
}
check_deps() {
  local bad=0 name ver
  local agent_deps
  agent_deps="$(dep_versions "$core" "$here/agent/Cargo.toml")"
  while read -r name ver; do
    case "$name" in tauri*|name|version|description|edition|path|crate-type|opt-level|panic|codegen-units|lto|strip) continue ;; esac
    if ! grep -qx "$name $ver" <<<"$agent_deps"; then
      echo "Dependance desalignee : $name = \"$ver\" dans NiTriTe, pas dans l'agent (agent/core/Cargo.toml)" >&2
      bad=1
    fi
  done < <(dep_versions "$upstream/src-tauri/Cargo.toml")
  return $bad
}

if [ "${1:-}" = "--check" ]; then
  if [ "$want" != "$have" ]; then
    echo "Version de l'agent ($have) differente de NiTriTe ($want) : lancez webpanel/scripts/sync-version.sh" >&2
    exit 1
  fi
  check_deps
  echo "Versions et dependances alignees : $want"
  exit 0
fi
sed -i.bak "0,/^version = \".*\"/s//version = \"$want\"/" "$core" && rm -f "$core.bak"
echo "agent/core : $have -> $want"
