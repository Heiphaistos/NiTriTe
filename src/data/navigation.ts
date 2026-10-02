export interface NavItem {
  id: string;
  label: string;
  icon: string;
  route: string;
  badge?: number;
}

export interface NavSection {
  title: string;
  items: NavItem[];
}

// Organisation par tâche (ce que l'on vient faire) plutôt que par technique.
// Les `id` sont stables : les favoris épinglés (nitrite-pinned) les référencent.
export const navigationSections: NavSection[] = [
  {
    title: "Système",
    items: [
      { id: "dashboard",     label: "Tableau de bord", icon: "layout-dashboard", route: "/" },
      { id: "diagnostic",    label: "Diagnostic",      icon: "stethoscope",      route: "/diagnostic" },
      { id: "monitoring",    label: "Monitoring",      icon: "activity",         route: "/monitoring" },
      { id: "optimizations", label: "Optimisations",   icon: "zap",              route: "/optimizations" },
      { id: "turbo-mode",    label: "Turbo Mode",      icon: "rocket",           route: "/turbo-mode" },
      { id: "missions",      label: "Missions",        icon: "list-checks",      route: "/missions" },
    ],
  },
  {
    title: "Logiciels",
    items: [
      { id: "master-install", label: "Master Install",  icon: "download",   route: "/master-install" },
      { id: "portables",      label: "Apps Portables",  icon: "package",    route: "/portables" },
      { id: "uninstaller",    label: "Désinstallateur", icon: "trash-2",    route: "/uninstaller" },
      { id: "updates",        label: "Mises à jour",    icon: "refresh-cw", route: "/updates" },
      { id: "dependencies",   label: "Dépendances",     icon: "layout-grid", route: "/dependencies" },
      { id: "os-downloads",   label: "OS & USB Tools",  icon: "hard-drive", route: "/os-downloads" },
    ],
  },
  {
    title: "Performance",
    items: [
      { id: "temperatures",  label: "Températures",         icon: "thermometer",    route: "/temperatures" },
      { id: "benchmark",     label: "Benchmark",            icon: "gauge",          route: "/benchmark" },
      { id: "perf-history",  label: "Historique Perf.",     icon: "bar-chart-3",    route: "/perf-history" },
      { id: "stats-reports", label: "Rapports statistiques", icon: "clipboard-list", route: "/stats-reports" },
    ],
  },
  {
    title: "Maintenance & Réparation",
    items: [
      { id: "cleaner",        label: "Nettoyeur Avancé",   icon: "sparkles",     route: "/cleaner" },
      { id: "tools",          label: "Outils Système",     icon: "wrench",       route: "/tools" },
      { id: "drivers",        label: "Drivers",            icon: "cpu",          route: "/drivers" },
      { id: "scanvirus",      label: "Scan Antivirus",     icon: "shield",       route: "/scanvirus" },
      { id: "restore-points", label: "Points de restauration", icon: "shield-check", route: "/restore-points" },
      { id: "bsod-analyzer",  label: "Analyse BSOD",       icon: "bug",          route: "/bsod-analyzer" },
      { id: "boot-manager",   label: "Boot Manager",       icon: "server",       route: "/boot-manager" },
    ],
  },
  {
    title: "Stockage & Données",
    items: [
      { id: "disk-visualizer",  label: "Visualiseur Disque",   icon: "pie-chart",   route: "/disk-visualizer" },
      { id: "big-files",        label: "Gros Fichiers",        icon: "file-search", route: "/big-files" },
      { id: "duplicate-finder", label: "Doublons",             icon: "files",       route: "/duplicate-finder" },
      { id: "hash-checker",     label: "Hash Checker",         icon: "hash",        route: "/hash-checker" },
      { id: "backup",           label: "Sauvegarde",           icon: "save",        route: "/backup" },
      { id: "data-recovery",    label: "Récupération Données", icon: "database",    route: "/data-recovery" },
      { id: "clone",            label: "Clonage Système",      icon: "copy",        route: "/clone" },
    ],
  },
  {
    title: "Réseau",
    items: [
      { id: "network",       label: "Réseau",           icon: "wifi",      route: "/network" },
      { id: "dns-switcher",  label: "DNS Switcher",     icon: "globe",     route: "/dns-switcher" },
      { id: "wifi-analyzer", label: "WiFi Analyzer",    icon: "radio",     route: "/wifi-analyzer" },
      { id: "port-scanner",  label: "Scanner de Ports", icon: "network",   route: "/port-scanner" },
      { id: "bluetooth",     label: "Bluetooth",        icon: "bluetooth", route: "/bluetooth" },
      { id: "hosts-editor",  label: "Éditeur Hosts",    icon: "file-text", route: "/hosts-editor" },
    ],
  },
  {
    title: "Terminal & Automatisation",
    items: [
      { id: "terminal", label: "Terminal",           icon: "terminal",        route: "/terminal" },
      { id: "scripts",  label: "Scripts & Snippets", icon: "file-code",       route: "/scripts" },
      { id: "wsl",      label: "WSL Linux",          icon: "terminal-square", route: "/wsl" },
      { id: "docker",   label: "Docker Manager",     icon: "container",       route: "/docker" },
    ],
  },
  {
    title: "Intelligence & Aide",
    items: [
      { id: "ai-agents",      label: "Agent IA",              icon: "bot",       route: "/ai-agents" },
      { id: "knowledge-base", label: "Base de Connaissances", icon: "book-open", route: "/knowledge-base" },
      { id: "documentation",  label: "Documentation",         icon: "file-text", route: "/documentation" },
    ],
  },
  {
    title: "Personnalisation",
    items: [
      { id: "settings",     label: "Paramètres",       icon: "settings",    route: "/settings" },
      { id: "theme-editor", label: "Éditeur de Thème", icon: "palette",     route: "/theme-editor" },
      { id: "profiles",     label: "Profils",          icon: "user",        route: "/profiles" },
      { id: "logs",         label: "Logs",             icon: "scroll-text", route: "/logs" },
    ],
  },
  {
    title: "WinPE",
    items: [
      { id: "winpe", label: "Mode WinPE", icon: "hard-drive", route: "/winpe" },
    ],
  },
];
