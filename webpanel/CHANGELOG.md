# Journal des versions — NiTriTe Agent

## 1.1.0

- Cookie de session `HttpOnly; SameSite=Strict` : les autres onglets, les favoris et F5 fonctionnent sans jeton dans l'URL.
- Content-Security-Policy stricte (script d'amorce autorisé par son empreinte), anti-clickjacking, `no-referrer`, COOP/CORP.
- Compression gzip/Brotli de l'interface.
- Menu **Agent** dans le panneau : état, versions, onglets connectés, journal, nouvelle version disponible, arrêt de l'agent.
- Option `--app` : panneau dans une fenêtre d'application Edge.
- Vérification des nouvelles versions au démarrage (`--no-update-check` pour la désactiver).
- Journal `%LOCALAPPDATA%\NiTriTe-WebPanel\agent.log`.
- Exécutable : icône NiTriTe, informations de version, DPI PerMonitorV2.
- Test de fumée sur un vrai Windows en CI, release automatique sur tag, Dependabot (dont le sous-module NiTriTe).

## 1.0.0

- Première version : backend complet de NiTriTe (348 commandes) servi au navigateur, interface de NiTriTe compilée telle quelle, jeton de session, contrôles Host/Origin, confirmations d'administration natives, instance unique, arrêt automatique.
