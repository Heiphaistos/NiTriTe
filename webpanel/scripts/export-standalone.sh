#!/usr/bin/env bash
# Exporte le panneau web vers son propre depot (NiTriTe-WebPanel), NiTriTe
# y etant un sous-module `upstream/` : aucune copie du code de NiTriTe.
#
#   webpanel/scripts/export-standalone.sh <dossier-cible> [url-du-depot-NiTriTe]
#   cd <dossier-cible> && git remote add origin <url-NiTriTe-WebPanel> && git push -u origin main
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
dest="${1:?usage: export-standalone.sh <dossier-cible> [url-NiTriTe]}"
upstream_url="${2:-https://github.com/Heiphaistos/NiTriTe.git}"

mkdir -p "$dest"
dest="$(cd "$dest" && pwd)"
[ -z "$(ls -A "$dest")" ] || { echo "Le dossier cible doit etre vide : $dest" >&2; exit 1; }

# Tout le panneau, sans les sorties de build
tar -C "$here" --exclude=node_modules --exclude=web/dist --exclude=agent/target --exclude=release -cf - . | tar -C "$dest" -xf -

# Chemins vers NiTriTe : ../.. (depot NiTriTe) -> upstream/ (sous-module)
sed -i 's#path = "\.\./\.\./\.\./src-tauri/src/lib.rs"#path = "../../upstream/src-tauri/src/lib.rs"#' "$dest/agent/core/Cargo.toml"
sed -i 's#const UPSTREAM = resolve(__dirname, "\.\./\.\.");#const UPSTREAM = resolve(__dirname, "../upstream");#' "$dest/web/vite.config.ts"

# CI du depot autonome
mkdir -p "$dest/.github/workflows"
sed -e 's#working-directory: webpanel/agent#working-directory: agent#' \
    -e 's#working-directory: webpanel#working-directory: .#' \
    -e 's#webpanel/scripts/#scripts/#' \
    -e 's#path: webpanel/agent/target#path: agent/target#' \
    -e 's#            webpanel/package-lock.json#            upstream/package-lock.json#' \
    -e '/^    paths:/d' \
    -e 's#- uses: actions/checkout@\(.*\)#- uses: actions/checkout@\1\n        with:\n          submodules: true#' \
    "$here/../.github/workflows/webpanel.yml" > "$dest/.github/workflows/build.yml"

cd "$dest"
git init -q -b main
git submodule add -q "$upstream_url" upstream
git add -A
git commit -q -m "NiTriTe Panneau web : agent sans interface + interface navigateur"
echo "Depot pret dans $dest"
echo "Ensuite : git remote add origin https://github.com/<vous>/NiTriTe-WebPanel.git && git push -u origin main"
