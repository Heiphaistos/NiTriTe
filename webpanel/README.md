# NiTriTe Panneau web

**NiTriTe sans fenêtre native.** Un petit exécutable, `NiTriTe-Agent.exe`, fait tourner tout le moteur de NiTriTe sur le PC ; l'interface s'affiche dans le navigateur déjà installé (Edge, Chrome, Firefox…), à l'adresse `http://127.0.0.1:7878`.

- **Mêmes fonctionnalités que NiTriTe** : les 348 commandes de l'application (diagnostic, Master Install, désinstallation, outils système, réparation, réseau, IA…) sont exposées à l'identique. Un test vérifie qu'il n'en manque aucune.
- **Plus léger** : pas de WebView ni de Chromium embarqué à charger en plus du navigateur. Sur un double cœur, l'interface bascule d'elle-même en profil « Léger ».
- **Aucune copie du code** : l'agent compile le backend de NiTriTe tel quel et le panneau compile l'interface de NiTriTe telle quelle. Toute correction faite dans NiTriTe profite automatiquement au panneau web.

## Utilisation

1. Lancer `NiTriTe-Agent.exe` (Windows demande les droits administrateur, comme NiTriTe).
2. Le navigateur s'ouvre sur le panneau. Rien d'autre à installer.
3. Pour arrêter : fermer l'onglet. L'agent s'arrête seul après 15 minutes sans onglet ouvert.

Relancer l'agent alors qu'il tourne déjà rouvre simplement l'onglet.

| Option | Effet |
|---|---|
| `--port N` | port d'écoute (défaut 7878, le suivant libre si occupé) |
| `--no-browser` | ne pas ouvrir le navigateur |
| `--stay` | ne jamais s'arrêter tout seul |
| `--idle-minutes N` | arrêt après N minutes sans onglet ouvert |
| `--lan` | accès depuis un autre PC du réseau local (voir Sécurité) |

## Sécurité

L'agent exécute des commandes d'administration : il est verrouillé en conséquence.

- Il n'écoute que sur `127.0.0.1` : invisible depuis le réseau (sauf `--lan`, explicite).
- Chaque lancement génère un **jeton aléatoire de 256 bits**, transmis à l'onglet dans le fragment de l'URL (`#t=…`, jamais envoyé sur le réseau ni dans le Referer), puis effacé de la barre d'adresse. Toute l'API l'exige.
- L'en-tête `Host` est vérifié (parade au *DNS rebinding*) et toute requête d'une autre origine est refusée : un site web ouvert dans un autre onglet ne peut pas piloter l'agent.
- Les confirmations d'administration du Terminal restent des **boîtes de dialogue Windows natives** : un script dans la page ne peut pas les valider.
- En mode `--lan`, partagez le lien (jeton compris) uniquement avec des personnes de confiance ; les boîtes de confirmation s'affichent sur le PC où tourne l'agent.

## Architecture

```
navigateur ──HTTP──▶ /api/invoke/<commande>  ──▶ commandes NiTriTe (src-tauri/src, inchangé)
           ◀──WS─── /api/events               ◀── window.emit(...) (progressions, journaux, monitoring)
           ──HTTP──▶ /api/host/*              ──▶ boîtes Ouvrir/Enregistrer, fichiers texte
```

- `agent/crates/tauri` — une cale qui porte le nom `tauri` : `#[tauri::command]` enregistre chaque fonction dans un registre avec un adaptateur JSON, `Emitter::emit` publie sur un bus relayé en WebSocket, `tauri::State` et `Builder::manage` fonctionnent comme dans Tauri. Seules les commandes listées dans `generate_handler!` (la liste blanche de l'application native) sont exposées.
- `agent/crates/plugins/*` — cales des plugins Tauri ; `tauri-plugin-dialog` affiche une vraie boîte Windows.
- `agent/core` — compile `src-tauri/src/lib.rs` de NiTriTe contre ces cales, sans le modifier.
- `agent/server` — le serveur (axum) : API, WebSocket, sécurité, interface embarquée dans l'exe.
- `web/` — compile `src/` de NiTriTe avec Vite ; `web/shims/` remplace les modules `@tauri-apps/*` par des appels à l'agent.

## Compiler

Prérequis : Node 22+, Rust stable, Windows (l'agent utilise WMI, le registre, PowerShell…).

```powershell
powershell -ExecutionPolicy Bypass -File webpanel\scripts\build.ps1
# -> webpanel\release\NiTriTe-Agent.exe
```

Développement de l'interface : lancer l'agent avec `--no-browser --stay`, puis `npm run dev` (port 5176, l'API est relayée vers l'agent) et ouvrir l'URL affichée par l'agent en remplaçant le port par 5176.

Tests : `npm test` (cales JS) et `cargo test -p tauri -p nitrite-agent` (dans `agent/`).

## Dépôt autonome

Le panneau est développé dans le dépôt NiTriTe (dossier `webpanel/`) pour partager le code. Pour le publier dans son propre dépôt `NiTriTe-WebPanel`, avec NiTriTe en sous-module :

```bash
webpanel/scripts/export-standalone.sh ../NiTriTe-WebPanel
cd ../NiTriTe-WebPanel
git remote add origin https://github.com/<compte>/NiTriTe-WebPanel.git
git push -u origin main
```

Mettre à jour le panneau avec la dernière version de NiTriTe : `git submodule update --remote upstream && scripts/sync-version.sh`.
