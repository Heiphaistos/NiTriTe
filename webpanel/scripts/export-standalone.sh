#!/usr/bin/env bash
# Exporte le panneau web vers son propre depot (Nitrite-We-Panel-), NiTriTe
# y etant un sous-module `upstream/` : aucune copie du code de NiTriTe.
#
# Nouveau depot (dossier vide) :
#   webpanel/scripts/export-standalone.sh <dossier-cible> [url-du-depot-NiTriTe]
#   cd <dossier-cible> && git remote add origin <url> && git push -u origin main
#
# Depot existant (clone de Nitrite-We-Panel-) : met a jour les fichiers,
# garde l'historique et le sous-module, puis il reste a committer/pousser :
#   webpanel/scripts/export-standalone.sh <clone-existant>
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
dest="${1:?usage: export-standalone.sh <dossier-cible> [url-NiTriTe]}"
upstream_url="${2:-https://github.com/Heiphaistos/NiTriTe.git}"

mkdir -p "$dest"
dest="$(cd "$dest" && pwd)"
mode=new
if [ -d "$dest/.git" ]; then
  mode=update
elif [ -n "$(ls -A "$dest")" ]; then
  echo "Le dossier cible doit etre vide ou etre un clone du depot : $dest" >&2
  exit 1
fi

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

# Tout le panneau, sans les sorties de build
tar -C "$here" --exclude=node_modules --exclude=web/dist --exclude=agent/target --exclude=release --exclude=standalone -cf - . | tar -C "$stage" -xf -
# Fichiers propres au depot autonome (release sur tag, dependabot du sous-module)
cp -R "$here/standalone/." "$stage/"

# Chemins vers NiTriTe : ../.. (depot NiTriTe) -> upstream/ (sous-module)
sed -i 's#path = "\.\./\.\./\.\./src-tauri/src/lib.rs"#path = "../../upstream/src-tauri/src/lib.rs"#' "$stage/agent/core/Cargo.toml"
sed -i 's#const UPSTREAM = resolve(__dirname, "\.\./\.\.");#const UPSTREAM = resolve(__dirname, "../upstream");#' "$stage/web/vite.config.ts"

# CI du depot autonome, derivee de celle du depot NiTriTe
mkdir -p "$stage/.github/workflows"
sed -e 's#working-directory: webpanel/agent#working-directory: agent#' \
    -e 's#working-directory: webpanel#working-directory: .#' \
    -e 's#webpanel/scripts/#scripts/#' \
    -e 's#path: webpanel/agent/target#path: agent/target#' \
    -e 's#            webpanel/package-lock.json#            upstream/package-lock.json#' \
    -e '/^    paths:/d' \
    -e 's#- uses: actions/checkout@\(.*\)#- uses: actions/checkout@\1\n        with:\n          submodules: true#' \
    "$here/../.github/workflows/webpanel.yml" > "$stage/.github/workflows/build.yml"

if [ "$mode" = update ]; then
  # Remplace tout sauf l'historique et le sous-module.
  find "$dest" -mindepth 1 -maxdepth 1 ! -name .git ! -name upstream ! -name .gitmodules -exec rm -rf {} +
  cp -R "$stage/." "$dest/"
  echo "Depot mis a jour dans $dest :"
  git -C "$dest" status --short
  echo "Ensuite : cd $dest && git add -A && git commit && git push"
  exit 0
fi

cp -R "$stage/." "$dest/"
cd "$dest"
git init -q -b main
git submodule add -q "$upstream_url" upstream
git add -A
git commit -q -m "NiTriTe Panneau web : agent sans interface + interface navigateur"
echo "Depot pret dans $dest"
echo "Ensuite : git remote add origin https://github.com/<vous>/Nitrite-We-Panel-.git && git push -u origin main"
