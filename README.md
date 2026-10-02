<div align="center">
  <h1>🔧 NiTriTe 8.224.0</h1>
  <p><strong>Suite de diagnostic, réparation, optimisation et administration Windows — 44 outils, interface Tauri v2 native.</strong></p>

  ![Version](https://img.shields.io/badge/version-8.224.0-blue)
  ![Stack](https://img.shields.io/badge/stack-Tauri%20v2%20%2B%20Rust%20%2B%20Vue%203-purple)
  ![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-informational)
  ![Language](https://img.shields.io/badge/language-Rust%20%2B%20TypeScript-orange)
  ![License](https://img.shields.io/badge/licence-MIT-green)
</div>

---

## 📋 Description

NiTriTe est un outil Windows tout-en-un conçu pour les techniciens et utilisateurs avancés. **48 outils** organisés en **10 catégories**, **9 modèles d'interface**, dont un tableau de diagnostic regroupant **33 sous-onglets** d'analyse système — le tout via une interface native moderne construite avec Tauri v2 (backend Rust, frontend Vue 3).

La 8.224.0 couvre le diagnostic matériel/logiciel complet, la réparation Windows (SFC/DISM/WinPE bootable), le clonage et la récupération de données (VSS), la gestion réseau et sécurité, l'automatisation via scripts et un assistant IA local (Ollama / llama.cpp portable), jusqu'au packaging d'une release portable autonome (exe + logiciels + drivers + scripts Windows).

---

## 🆕 Nouveautés

### 8.224.0 — 2 octobre 2026

- **9 modèles d'interface** : Forge (nouveau modèle par défaut), Command Deck, Bento, Console, Orbital, Colonnes, Verre & dock, Mission — et **Classique**, l'interface d'avant, à l'identique. Choix dans Paramètres › Interface ou d'un clic dans la barre d'état ; changer de modèle n'interrompt pas une opération en cours. Toutes les pages, boutons et sous-onglets fonctionnent comme avant.
- **Mode Mission** : enchaîner les outils d'une intervention (Remise en état PC, PC lent, PC neuf, Après un écran bleu, Problème réseau, Sauvegarde & migration, ou vos propres missions), avec une barre de suivi sur chaque page, des notes par étape et un rapport à copier ou exporter.
- **74 thèmes** dont **12 nouveaux thèmes clairs** (Forge, Neige, Lavande, Sable, Ciel, Rosé, Sauge, Ardoise, Solarized Light, GitHub Light, Catppuccin Latte, Nord Snow Storm) ; tous les thèmes sombres sont conservés, filtre Sombres / Clairs dans Paramètres.
- **Tableau de bord** propre à chaque modèle (santé, favoris, récents, missions…) et écran de démarrage aux couleurs du thème.
- **Raccourcis clavier** : Ctrl+1…0 pour les sections, Ctrl+Maj+M pour changer de modèle, Alt+←/→, navigation aux flèches dans tous les menus (aide : touche `?`).
- **Corrections** : barres de progression qui s'affichaient en rouge à tort (score santé à 100, batterie pleine, fin de scan, de récupération, de sauvegarde et d'image disque).

<details>
<summary><b>8.221.0 → 8.223.0 — fin septembre 2026</b> : Master Install, profil de performance, panneau web, démarrage rapide</summary>

- **Master Install fiable de bout en bout** : détection des applications déjà installées dès l'ouverture, installation *et* désinstallation en un clic ou par lot (20 apps d'un coup, file annulable, « Réessayer les échecs »), filtres Installées / Non installées, journal winget en direct. Catalogue nettoyé : 723 apps, identifiants WinGet vérifiés contre le dépôt officiel (55 corrigés, 75 ajoutés), plus aucun doublon, profil « Technicien ».
- **Outils Système** : les commandes chaînées (reset réseau, reset Windows Update, réparation du démarrage…) s'exécutent enfin, avec élévation UAC automatique pour les 43 outils qui l'exigent ; `wmic` (retiré de Windows 11 24H2) remplacé.
- **Mode performance adaptatif** : Complet / Équilibré / Léger, choisi automatiquement selon le processeur et la mémoire (un double cœur passe en Léger : ni flous, ni animations, démarrage et monitoring allégés).
- **62 thèmes** (dont 8 nouveaux : Nitrite Light, Carbon AMOLED, Graphite, Contraste élevé…), tous mémorisés au redémarrage.
- **Menu réorganisé par tâche** en 10 sections ; Monitoring, Turbo Mode et Rapports statistiques y sont désormais accessibles.
- **[Panneau web](webpanel/README.md)** : NiTriTe sans fenêtre native — un agent léger + l'interface dans le navigateur, avec les mêmes fonctionnalités.
- **Démarrage bien plus rapide** : tableau de bord en ~8 s au lieu de plus d'une minute avec des lecteurs réseau ; partages réseau 134 s → 0,7 s, pare-feu ~20 s → 0,2 s, historique système 9,5 s → 0,3 s ; un module lent ne bloque plus l'ouverture.
- **Sécurité** : Control Flow Guard activé, exécutables signés (Authenticode), confirmation Windows native pour les commandes administrateur du Terminal.

</details>

<details>
<summary><b>8.217.0 → 8.219.0 — mi-septembre 2026</b> : mise à jour automatique</summary>

- **Mise à jour automatique** des versions installée *et* portable : au démarrage, NiTriTe propose lui-même la nouvelle version. La mise à jour ne remplace que l'application — les dossiers `logiciel`, `Drivers` et `Script Windows` ne sont jamais touchés.
- Deux téléchargements au choix : **portable complet** (aucune trace sur le PC, clé USB) ou **installateur complet**, avec logiciels, pilotes et scripts inclus.
- **Plus de blocage infini** : 16 opérations système (pilotes, Windows Update, Docker, bcdedit, certificats, pare-feu, inventaire matériel…) ont désormais une limite de temps et rendent la main proprement.
- La proposition de mise à jour arrive dès l'ouverture de la fenêtre, au lieu d'attendre la fin de l'inventaire de démarrage.
- Téléchargement de l'IA locale (llama.cpp) réparé et vérifié par empreinte SHA-256.

</details>

<details>
<summary><b>8.111.0 → 8.216.0 — août 2026</b> : grande campagne de fiabilisation</summary>

- **Fini les faux succès** : nettoyage, Windows Update, winget, cache DNS, désinstallation, signatures Defender, quarantaine, pilotes, démarrage en récupération, corbeille… chaque action affiche désormais son vrai résultat.
- **Confirmations avant toute action irréversible** : suppression de copies Shadow, de conteneurs Docker, de profils, restauration du MBR (confirmation tapée), outils WinPE (registre SYSTEM, arrêt de processus et de services).
- **Windows en français** : SFC et DISM détectent enfin correctement l'état du système, plus de fausse mise à jour WinGet, accents corrigés dans une dizaine d'écrans (terminal, sécurité, programmes au démarrage, Chocolatey, Scoop, hosts…).
- **Sécurité** : correction d'une faille qui pouvait vider les données de l'app via la quarantaine, « Vider le cache Firefox » ne supprime plus tout le profil, chaque redirection de téléchargement est vérifiée.
- **Plus de gel** : délais maximums sur les appels PowerShell, winget/choco, le monitoring en direct, les instantanés de performance et les connexions réseau.
- **Messages clairs** à la place des écrans vides et des chargements infinis (diagnostic, SMART, partitions, recherches, filtres, scanner de ports, BSOD…), vraies erreurs de l'IA locale (Ollama / llama.cpp).

</details>

<details>
<summary><b>8.62.0 → 8.73.0 — juillet 2026 (2)</b> : Master Install 745 apps, navigation sans perte, WinPE réparé</summary>

- **28 boutons d'actions sensibles réparés** (terminer un processus, pare-feu, Bluetooth, registre, points de restauration, DLL, Docker, pilotes…) : leur confirmation ne s'affichait jamais, l'action était annulée à chaque clic.
- **Master Install** : installation silencieuse réellement fonctionnelle sur les 745 applications, avec chaîne de secours WinGet → Chocolatey → Scoop → téléchargement direct, vérification réelle de chaque installation, temps restant estimé, MAJ et désinstallation par application.
- **Navigation sans perte** : un scan, une mise à jour, une installation ou un nettoyage en cours continue et garde son affichage quand on change de page.
- **Mode WinPE** : l'onglet Réparation (effacement disque, SFC / DISM hors ligne, comptes hors ligne, BitLocker) fonctionne enfin réellement.
- Hash Checker, DNS Switcher, copies Shadow et Éditeur Hosts réparés ; version portable **sans aucune trace** sur le PC client.
- **Pack complet auto-extractible** (application + logiciels + pilotes + scripts), vidéo d'introduction au démarrage, plus aucun gel de l'interface pendant les commandes longues.

</details>

<details>
<summary><b>7.59.0 → 8.55.0 — juillet 2026 (1)</b> : Windows français et résultats fiables</summary>

- **Décodage des accents** sur Windows FR pour toutes les sorties d'outils console (pilotes, réseau, WiFi, ARP, bcdedit, journaux, licences, partages…) : fin des caractères illisibles.
- **Résultats fidèles** : scan antivirus (menaces réellement détectées), installations, MAJ de pilotes, points de restauration, Bluetooth, WSL, historique de performance, top processus.
- **Exclusions de mise à jour** WinGet / Chocolatey / Scoop enfin respectées.
- **Sécurité** : protections contre l'injection PowerShell et CSV, confirmation avant d'enregistrer des secrets en clair (mots de passe WiFi, clés BitLocker).

</details>

<details>
<summary><b>6.1.0 → 6.72.0 — mai à juillet 2026</b> : audits de sécurité et 19 corrections</summary>

- **Audit de sécurité complet** : injections PowerShell, écriture de fichiers arbitraire, chemins traversants, activation Windows sans script distant ; analyse CodeQL et alertes Dependabot automatiques.
- **19 corrections** : type de RAM DDR4/DDR5, export du scan vers n'importe quel dossier, benchmark, test de débit 50 Mo, traceroute, rapports batterie et certificats exportables, boutons des gestionnaires Windows.
- Délais maximums sur DISM / SFC / scripts, erreurs réelles affichées au lieu de données simulées.

</details>

<details>
<summary><b>Mars 2026</b> : NiTriTe 2.0, réécriture complète</summary>

- **Réécriture complète en Rust + Tauri v2 + Vue 3** : démarrage quasi instantané, un seul exécutable portable.
- **Diagnostic complet** en dizaines d'onglets, monitoring temps réel, score de santé.
- **IA 100 % locale et portable** : téléchargement automatique de llama.cpp et de modèles GGUF, sans installation.
- **729 applications portables**, pilotes de base et plus de 50 scripts Windows inclus.

</details>

---

## 📺 Démonstration

<video src="https://media.heiphaistos.org/videos/nitrite.mp4" controls width="100%" preload="none"></video>

---

## ✨ Fonctionnalités

### 🖥️ Système
- **Tableau de bord** : vue d'ensemble santé système, raccourcis, score global
- **Diagnostic** (33 sous-onglets, voir détail ci-dessous) : scan complet exportable TXT/HTML/MD/JSON
- **Optimisations** : debloat, réglages performance, tweaks Windows en un clic
- **Monitoring** : surveillance temps réel avec annotations et enregistrement de session

### 📦 Logiciels
- **Outils Système** : accès rapide utilitaires Windows natifs
- **Master Install** : installation en lot multi-source (winget/Chocolatey/Scoop) avec dry-run et résumé
- **Apps Portables** : bibliothèque d'applications portables catégorisées (bureautique, dev, média, réseau, système, utilitaires)
- **OS & USB Tools** : téléchargement ISO Windows, création clé USB bootable
- **Applications** : inventaire logiciels installés, désinstallation groupée

### ⚡ Performance
- **Températures** : sondes CPU/GPU/carte mère en temps réel
- **Benchmark** : test de performance CPU/RAM/disque avec historique de gains
- **Historique Performance** : suivi des métriques dans le temps, comparaison snapshots
- **Turbo Mode** : profil performance temporaire (stats avant/après)
- **Rapports statistiques** : seuils configurables, comparaison session précédente

### 🧪 Avancé (BETA)
- **Clonage Système** : `wbadmin` + `robocopy` avec gestion des codes de retour
- **Récupération de Données** : VSS (Shadow Copy), Corbeille, fichiers supprimés, dossiers utilisateur
- **Visualiseur Disque** : cartographie de l'occupation espace par dossier/type
- **Doublons** : détection et suppression de fichiers dupliqués (hash)
- **Gros Fichiers** : recherche des plus gros consommateurs d'espace
- **Hash Checker** : calcul/vérification SHA-256 de fichiers
- **Boot Manager** : gestion BCD, entrée de démarrage par défaut, timeout
- **Éditeur Hosts** : édition sécurisée du fichier hosts (validation IP/hostname)
- **Analyse BSOD** : parsing dump crash Windows, diagnostic cause probable
- **WSL Linux** : gestion des distributions Windows Subsystem for Linux
- **Points de Restauration** : création/restauration de points système
- **Docker Manager** : gestion conteneurs/images Docker Desktop

### 🔧 Maintenance
- **Mises à jour** : Windows Update + 4 gestionnaires de paquets (WinGet, Chocolatey, Scoop, Windows Update) avec détail des mises à jour disponibles
- **Drivers** : inventaire pilotes, détection critiques/problématiques (Error/Degraded), scanner de mise à jour dédié
- **Désinstallateur** : détection automatique NSIS (`/S`), Inno Setup (`/VERYSILENT`), winget — désinstall silencieux
- **Nettoyeur Avancé** : nettoyage fichiers temporaires, cache navigateurs, registre
- **Sauvegarde** : backup ciblé avec collecteurs et formatage de rapport
- **Scan Antivirus** : lancement scan Windows Defender / rapport menaces
- **Dépendances** : détection runtimes requis (VC++, .NET...), filtre requises/optionnelles, test post-installation

### 🌐 Réseau & Terminal
- **Réseau** : configuration interfaces, statistiques, Wi-Fi
- **DNS Switcher** : changement rapide de serveurs DNS
- **WiFi Analyzer** : scan réseaux à proximité, qualité signal
- **Scanner de Ports** : détection ports ouverts et services exposés
- **Bluetooth** : gestion périphériques appairés
- **Terminal** : terminal intégré (PowerShell/CMD)
- **Scripts & Snippets** : éditeur, bibliothèque de scripts (.bat/.cmd/.ps1) exécutables depuis l'app, moteur de validation

### 🧠 Intelligence
- **Agent IA** : assistant local via Ollama ou llama.cpp portable, appel d'outils (tool calling) sur les commandes Nitrite
- **Base de Connaissances** : articles et procédures de dépannage
- **Documentation** : aide intégrée

### 📊 Rapports
- **Logs** : consultation logs application (rotation, niveaux)
- **Éditeur de Thème** : personnalisation de l'interface

### ⚙️ Configuration
- **Paramètres** : préférences application, export/import config
- **Profils** : profils de configuration multiples

### 💽 WinPE
- **Mode WinPE** : ISO bootable Windows PE 11 pour réparation hors-OS (build via Windows ADK), 15+ commandes PE

---

### 🩺 Diagnostic — détail des 33 sous-onglets

| Bloc | Onglets |
|------|---------|
| Matériel | CPU, RAM, GPU, Stockage (Storage), Capteurs/Températures (Perf) |
| Système | Système (info générale), Historique (événements), Comptes (Accounts), Activation (licence Windows), Certificats numériques |
| Logiciel | Software (inventaire), Services, SysDrivers, Mises à jour (Updates), Cleaner |
| Réseau | Network, Firewall, Partages (Shares), Hosts, NetTools, Bluetooth |
| Sécurité | Security (registre clés de persistance suspectes) |
| Stockage avancé | Dossiers (Folders), Processus (Processes) |
| Réparation | Repair (SFC `/scannow`, DISM `RestoreHealth`), Boot, Analyse BSOD |
| Performance | Benchmark, Historique Perf. (PerfHistory) |
| Pilotes | Driver Updater |
| Autres | WSL, Scan (orchestrateur du scan complet + export) |

Export des résultats en **TXT / HTML / MD / JSON**, score de santé global, choix du périmètre de scan (rapide/complet).

---

## 🛠️ Stack technique

| Couche | Technologies |
|--------|-------------|
| Frontend | Vue 3 + TypeScript + Vite + Pinia + Tailwind CSS |
| Backend natif | Rust + Windows API (`std::process::Command`, `windows-rs`) |
| Framework desktop | Tauri v2 |
| IPC | Tauri commands (`invoke`), déduplication concurrence |
| IA | Ollama / llama.cpp portable, tool calling sur commandes natives |
| Installer | NSIS (bundle Tauri) + SFX 7-Zip (release portable tout-en-un) |
| Build | `build.bat` (kill → tsc → tauri build → packaging 4 modes) |

---

## 🚀 Installation

### Prérequis

- Windows 10 / 11 (x64)
- Rust stable (`rustup`) + Node.js 18+
- WebView2 Runtime (inclus dans Windows 11, auto-installé sinon)

### Installer (utilisateur final)

Télécharger et exécuter le setup NSIS :

```
Nitrite_8.219.0_x64-setup.exe
```

Ou la release portable tout-en-un (`Nitrite_v8.219.0_full.exe`) : app + logiciels portables + drivers + scripts Windows, sans installation.

### Build depuis les sources

```bat
REM Prérequis : npm install (première fois)
npm install

REM Build interactif — 4 modes de packaging au choix
build.bat
```

`build.bat` propose :
1. **EXE portable seul** (~15 Mo) — application uniquement
2. **Dossier portable complet** (~2,5 Go) — exe + `logiciel/` + `Drivers/` + `Script Windows/`
3. **SFX tout-en-un** — un seul `.exe` qui extrait tout et lance Nitrite
4. **ISO WinPE 11 bootable** — nécessite Windows ADK + WinPE Add-on

L'exécutable généré : `src-tauri\target\release\nitrite.exe`
L'installeur NSIS : `src-tauri\target\release\bundle\nsis\Nitrite_8.219.0_x64-setup.exe`

### Développement

```bat
npm run tauri dev
```

---

## 📂 Architecture

```
NiTriTe/
├── src/
│   ├── components/diagnostic/     # 33 composants Vue (DiagTab*.vue)
│   ├── data/
│   │   ├── navigation.ts          # 9 catégories, structure du menu
│   │   └── portable/              # catalogue apps portables par catégorie
│   ├── pages/                     # 44 pages (une par route)
│   └── router/                    # routes Vue Router
├── src-tauri/
│   ├── src/
│   │   ├── system/                 # ~50 modules (clone, boot_manager, drivers, security...)
│   │   ├── installer/              # winget, chocolatey, scoop, uninstaller, smart_install
│   │   ├── maintenance/            # cleanup, debloat, browser_cleanup, terminal
│   │   ├── backup/collector/       # collecteurs de sauvegarde + rendu rapport
│   │   ├── scripts/                # executor + validator (scripts .bat/.ps1)
│   │   └── ai/                     # Ollama / llama.cpp portable, tool calling
│   └── target/release/bundle/nsis/ # Installeur NSIS final
├── logiciel/                       # apps portables bundlées release complète (gitignored, ~2,5 Go)
├── Drivers/                        # runtimes (Visual C++) bundlés release complète
├── Script Windows/                 # scripts .bat/.ps1 accessibles depuis l'app
├── boot/                           # build-bootable.bat — génération ISO WinPE
├── build.bat                       # Script de build Windows (4 modes)
└── package.json
```

---

## 📝 Notes techniques

- **CREATE_NO_WINDOW** (`0x08000000`) appliqué sur tous les modules Rust — pas de flash CMD
- Robocopy : codes de retour `< 8` = succès (comportement normal)
- VSS paths : format `\\?\GLOBALROOT` + device_object
- Désinstall NSIS détecté via metadata `VersionInfo` (`Nullsoft`) → `/S`
- Désinstall Inno Setup détecté via `Inno|Jordan Russell` → `/VERYSILENT`
- `set_default_boot` : validation stricte GUID avant interpolation `bcdedit`
- Version applicative injectée au build depuis `package.json` (`__APP_VERSION__`, `vite.config.ts`)

---

## 📝 Licence

MIT — © 2026 Heiphaistos
