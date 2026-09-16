# PRD — Mise a jour automatique sur LES DEUX versions, et packaging complet des deux

Statut : **en attente du feu vert de Momo**. Redige le 2026-09-16. Remplace `PRD-test-mise-a-jour-auto.md`.

## 1. Ce que Momo veut

1. **Les deux versions ont le contenu complet.** Installeur *et* portable embarquent `logiciel\`, `Drivers\` et `Script Windows\`. Nitrite tourne sans, mais il tourne *avec* : ce n'est pas un supplement, ca fait partie du produit.
2. **Les deux versions se mettent a jour toutes seules.** La portable est la version principale — celle qui ne laisse pas de traces. Elle n'a pas moins droit a la mise a jour que la version installee.
3. **La mise a jour ne touche JAMAIS `logiciel\`, `Drivers\`, `Script Windows\`.** Ces dossiers appartiennent a Momo, il les fait evoluer a la main. Une mise a jour remplace l'application, rien d'autre.
4. **Les noms des fichiers publies doivent dire ce qu'ils sont.** Aujourd'hui rien sur la page GitHub ne dit qu'un asset est la version portable. `full.exe` disparait.
5. **Un client ne va JAMAIS chercher un fichier de mise a jour.** Il telecharge Nitrite une fois. Deux semaines et treize versions plus tard, il relance son exemplaire, une fenetre dit « une nouvelle version est sortie, voulez-vous mettre a jour ? », il repond oui, et tout se fait seul jusqu'au redemarrage. La page GitHub ne montre que ce qu'un humain telecharge.

Le point 5 est deja construit pour la version installee (fenetre, telechargement, installation, redemarrage). Ce qui manque : le meme chemin pour la portable (§3.2), et sortir la tuyauterie de la page de release (§3.4).

Ce PRD remplace la decision prise le 2026-09-15 (« la portable ne se met pas a jour »), qui reglait le probleme en supprimant la fonctionnalite. `is_portable_install()` sera retire au profit d'un vrai chemin de mise a jour portable.

## 2. Ce qui bloque aujourd'hui, verifie dans le code

**(a) Le plugin Tauri ne sait pas mettre a jour une application portable sous Windows.**
`tauri-plugin-updater-2.11.0/src/updater.rs` ne connait que deux formes : `WindowsUpdaterType::Nsis` et `::Msi`. Il lance l'installeur via `ShellExecuteW` puis appelle `std::process::exit(0)`. Il n'existe aucun chemin « remplacer l'executable sur place ». **Le chemin portable doit donc etre ecrit a la main.**

**(b) Livrer le contenu via `bundle.resources` DETRUIRAIT le contenu a la mise a jour suivante.**
Lu dans le script genere (`src-tauri/target/release/nsis/x64/installer.nsi`) : a l'installation d'une nouvelle version, l'installeur lit `UninstallString` dans le registre et **execute le desinstalleur de la version PRECEDENTE** (`ExecWait '$R1 /UPDATE /P _?=$INSTDIR'`, lignes 344-354). Ce vieux desinstalleur supprime ce que LUI avait installe, section « Delete resources » comprise. Donc : si la 8.217.0 pose les 733 Mo en `resources`, la 8.218.0 les fera effacer par le desinstalleur de la 8.217.0 — et ne les remettra pas si elle est legere. **Le contenu ne doit jamais entrer dans le manifeste de l'installeur.**

**(c) Bonne nouvelle qui decoule du meme fichier** : la desinstallation fait `Delete` sur les fichiers connus puis `RMDir "$INSTDIR"` **sans `/r`**. Un dossier que l'installeur n'a pas pose n'est ni supprime ni vide. Un `logiciel\` depose a cote survit donc a toutes les mises a jour, par construction.

**(d) Question du PRD precedent, tranchee** : `relaunch()` apres `downloadAndInstall()` n'est jamais atteint sous Windows (`process::exit(0)` juste apres). Le redemarrage est gere par le plugin lui-meme, qui passe `/R` a NSIS quand `restart_after_install` est vrai (vrai par defaut). L'appel `relaunch()` sera retire du code : il ne sert a rien et laisse croire qu'il fait quelque chose.

## 3. Architecture retenue

Principe : **ce que le client telecharge** contient tout ; **ce que l'updater telecharge** ne contient que l'application.

### 3.1 Version installee (client)

- Livraison : un SFX qui extrait `logiciel\`, `Drivers\`, `Script Windows\` puis lance l'installeur NSIS leger. Le NSIS pose l'application, le SFX pose le contenu **a cote**, hors de son manifeste — condition (b)/(c) respectee.
  Le dossier d'installation se lit dans le registre apres l'installeur : `WriteRegStr SHCTX "${MANUPRODUCTKEY}" "" $INSTDIR` (ligne 652 du script genere).
- Mise a jour : chemin Tauri standard, inchange. `latest.json` continue de pointer l'installeur NSIS leger (12 Mo). Le contenu n'est pas retelecharge.

### 3.2 Version portable

- Livraison : SFX complet, comme aujourd'hui (application + les trois dossiers), simplement renomme.
- Mise a jour : **a ecrire**, sur le modele du lanceur ForgeMT2 que Momo connait —
  1. lire un second manifeste `latest-portable.json` ;
  2. telecharger le binaire nu (~21 Mo) et sa signature ;
  3. **verifier la signature minisign avec la meme cle publique que l'updater** — un binaire non signe par la cle du projet est jete, jamais ecrit ;
  4. Windows interdit de supprimer un `.exe` en cours d'execution mais autorise a le **renommer** : `Nitrite.exe` -> `Nitrite.exe.ancien`, puis mise en place du neuf, puis relance ;
  5. balayer `.ancien` au demarrage suivant.
  Les trois dossiers ne sont jamais ouverts par ce chemin : il ne connait qu'un seul fichier.
- Cout estime : ~150 lignes de Rust (`reqwest` et `minisign-verify` sont deja dans l'arbre de dependances), plus le cablage dans le composable existant.

### 3.3 Detection du mode

`is_portable_install()` (dossiers `Drivers\`/`Script Windows\` a cote de l'executable) ne sert plus a couper la mise a jour mais a **choisir le chemin** : portable -> §3.2, sinon -> §3.1. Le temoin doit changer, puisque les deux versions auront desormais ces dossiers. Temoin propose : la presence de `uninstall.exe` a cote de l'executable, ecrit par NSIS (ligne 649) et par lui seul. A verifier a l'implementation.

### 3.4 Ou vit la tuyauterie — sur le VPS, pas sur GitHub

Decision de Momo, 2026-09-16 : **le canal de mise a jour est heberge sur `nitrite.heiphaistos.org`**, modele ForgeMT2. La page GitHub ne porte plus que les deux fichiers complets destines a un humain.

Precision qui leve le malentendu : ces fichiers de mise a jour existent precisement **pour que le client n'ait jamais a aller les chercher**. C'est l'application qui les lit, seule, au demarrage. Ils n'avaient rien a faire sur la page de release — ils vont maintenant la ou personne ne les voit.

Ce qui est en ligne aujourd'hui, verifie :
- `https://nitrite.heiphaistos.org` repond 200 ; `/etc/nginx/sites-enabled/nitrite` fait `proxy_pass http://localhost:8080` (une application tourne derriere) ;
- ForgeMT2 sert deja son manifeste par un `location /telechargement/ { alias /var/www/forgemt2-telechargement/; }`.

A poser, sur le meme patron :

```
location /maj/ {
    alias /var/www/nitrite-maj/;
    # Ce location a ses propres add_header : il faut y REPETER les en-tetes
    # de securite, nginx ne les herite plus (piege paye sur ce VPS).
    add_header X-Frame-Options        "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff"    always;
    add_header Referrer-Policy        "strict-origin-when-cross-origin" always;
}
```

Contenu de `/var/www/nitrite-maj/` : `latest.json`, `latest-portable.json`, la charge installee (~12 Mo) et la charge portable (~21 Mo), chacune avec sa `.sig`.

**Depot des fichiers** : depuis la machine de Momo, en meme temps que la construction du SFX qui s'y fait deja. Pas de secret SSH a poser dans la CI — la CI construit et signe, le depot sur le VPS se fait dans la foulee du meme passage local. Un acces SSH fonctionnel a `root@212.227.140.45` a ete verifie.

**Ce que cela coute, a savoir avant de dire oui** : si le VPS est a terre, aucune mise a jour ce jour-la. L'application doit donc traiter l'echec comme elle le fait deja — un mot dans `.logs`, rien a l'ecran, et on joue quand meme. C'est deja le comportement du composable.

## 4. Assets publies — a valider par Momo

**Sur la page GitHub de la release — rien d'autre :**

| Nom propose | Taille | Contenu |
|---|---|---|
| `Nitrite_8.217.0_portable_complet.exe` | ~350 Mo | application + logiciel + Drivers + Script Windows, aucune installation, aucune trace |
| `Nitrite_8.217.0_installateur_complet.exe` | ~350 Mo | idem, installe dans Program Files |

**Sur `nitrite.heiphaistos.org/maj/` — invisible pour le client, lu par l'application seule :**

| Fichier | Taille | Role |
|---|---|---|
| `latest.json` + `latest-portable.json` | ~1 Ko | manifestes : version, URL, signature |
| `nitrite-8.217.0-installee.exe` + `.sig` | ~12 Mo | installeur NSIS leger, applique par le chemin Tauri |
| `nitrite-8.217.0-portable.exe` + `.sig` | ~21 Mo | binaire nu, applique par le chemin portable |

`Nitrite_8.216.0_portable.zip` (13,7 Mo, le binaire seul) est **retire de la release** : il porte le mot « portable » en promettant le contraire de ce que le mot veut dire ici. `Nitrite_8.216.0_x64-setup.exe`, `.sig` et `latest.json` quittent aussi la page — ils partent sur le VPS.

**Decision attendue de Momo** : ces deux noms de fichiers clients, ou les siens.

## 5. Contenu applicatif de la 8.217.0

Une version qui ne servirait qu'a tester la mise a jour ne vaut pas une publication. Deja identifie, a corriger :

| # | Fichier | Probleme |
|---|---|---|
| 1 | `src/pages/SettingsPage.vue`, onglet A propos | annonce `github.com/Heiphaistos/NiTriTe-v8-AppWindows`, qui n'est pas le depot vivant. **Tranche par Momo le 2026-09-16 : le depot est `Heiphaistos/NiTriTe`**, decrit « NiTriTe V8, maintenance Windows tout-en-un ». Corriger le lien. La version Python est archivee, seule la version Rust vit. |
| 2 | `src-tauri/src/ai/llamacpp.rs:515` | le binaire du serveur llama.cpp est telecharge et lance **sans verification d'empreinte** (`TODO` en place). OWASP A08. |
| 3 | `src-tauri/src`, 11 avertissements clippy | cosmetique. Optionnel. |
| 4 | depot GitHub `Heiphaistos/NiTriTe` | description a poser : **« NiTriTe V8 — maintenance Windows tout-en-un »**, sans numero de version detaille. Momo, 2026-09-16 : « quand on passera a la V9, on le modifiera ». |

Plus une passe de chasse selon la methode etablie sur ce depot : trouver un defaut, puis `grep` de **toute sa famille** dans le reste du code. Familles deja ratissees, ne pas refaire : faux-succes `run_system_command`, absence de timeout sur `Command::output()`, echappement des exports. **Aucune correction fabriquee** : si la chasse ne rend rien, la version sort avec les points 1 et 2 et on le dit.

## 6. Recette

**Prealable** : la 8.216.0 actuellement en ligne ne sait pas se mettre a jour en portable. Le test part donc d'une 8.217.0 installee des deux facons, et c'est la 8.218.0 qui sera observee. Autrement dit : **la publication 8.217.0 sert a poser le point de depart, la 8.218.0 sert a prouver.**

Version installee :
- [ ] l'installeur complet pose l'application ET les trois dossiers dans le dossier d'installation ;
- [ ] « Verifier les mises a jour » repond « a jour » tant que la 8.218.0 n'existe pas ;
- [ ] a la publication de la 8.218.0 : fenetre annoncant la version, elevation UAC, installeur en mode passif, application relancee seule ;
- [ ] **apres mise a jour, `logiciel\`, `Drivers\` et `Script Windows\` sont intacts** — comparer le nombre de fichiers avant/apres, pas seulement l'existence du dossier ;
- [ ] `.logs` porte `Mise a jour disponible : 8.218.0` puis `Demarrage NiTriTe 8.218.0`.

Version portable :
- [ ] memes points, sans UAC ni installeur ;
- [ ] l'executable est remplace sur place, le dossier ne bouge pas d'un fichier a part lui ;
- [ ] `Nitrite.exe.ancien` a disparu au demarrage suivant ;
- [ ] **aucune trace hors du dossier** : ni Program Files, ni cle de registre, ni `%APPDATA%` — c'est la raison d'etre de cette version, elle se verifie ;
- [ ] un binaire dont la signature ne correspond pas est refuse : a tester pour de vrai en signant un faux binaire avec une autre cle, pas en supposant.

Deux frictions attendues, a noter sans les traiter comme des pannes : l'installeur n'a aucune signature Authenticode (SmartScreen et UAC diront « editeur inconnu »), et le premier telechargement fait 350 Mo.

## 7. Pieges d'execution deja payes

- **`build.bat` est inutilisable depuis une session Claude** (auto-elevation UAC + menu au clavier). Refaire le SFX a la main : `7z a -t7z -m0=lzma2 -mx=5 -mmt=on -ms=on release/_tmp.7z ./logiciel ./Drivers "./Script Windows"`, ajouter l'executable, puis concatener `tools/7zSD.sfx` + configuration en CRLF + archive. Ne pas recopier les 733 Mo dans un dossier de preparation : 7-Zip archive directement depuis le depot.
- **`tools/7zSD.sfx` ne se telecharge plus nulle part** (retire de « 7-Zip Extra », 404 sur les anciennes versions). Present localement ; s'il disparait, le reprendre dans les 215040 premiers octets d'un SFX deja publie.
- **Controler le lockfile apres tout `npm i`** : `emnapi` doit toujours apparaitre 30 fois et `npm ci` a froid doit passer, sinon la CI casse.
- **Televerser les assets de 350 Mo en arriere-plan**, ils depassent le delai d'un appel normal.
- **La cle privee `~/.tauri/nitrite-updater.key` signe tout**, y compris le futur chemin portable. Perdue, plus aucun poste deja installe ne peut etre mis a jour.

## 8. Retour arriere

Tant que la 8.216.0 reste en ligne, une installation abimee se repare en la reinstallant. Pour la portable, le retour arriere est le dossier lui-meme : il suffit de ne pas ecraser l'ancien. Aucun poste ne peut recevoir un binaire non signe par la cle du projet.

## 9. Hors perimetre

- Mise a jour de `logiciel\`, `Drivers\`, `Script Windows\` — **explicitement exclue par Momo**, ces dossiers restent les siens.
- Signature Authenticode de l'installeur : coute un certificat, sa decision.
- Mises a jour differentielles : la charge fait 12 a 21 Mo, rien a optimiser.
