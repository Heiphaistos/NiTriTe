import type { Component } from "vue";
import {
  LayoutDashboard, Stethoscope, Activity, Zap, LayoutGrid, Wrench,
  Download, Package, HardDrive, RefreshCw, Cpu, Scan, Save, Shield, ShieldCheck,
  Wifi, Terminal, FileCode, Bot, BookOpen, FileText, ScrollText,
  BarChart3, Settings, Palette, Trash2, Gauge, Server, Globe, Bug, TerminalSquare,
  Bluetooth, Sparkles, Copy, Database, User,
  Thermometer, PieChart, Files, FileSearch, Hash, Container, Radio, Network, Code2,
  Rocket, ClipboardList, Monitor, AppWindow, Brush, Usb, ListChecks,
} from "lucide-vue-next";

/** Icônes Lucide des entrées de menu, par nom (`NavItem.icon`). Partagé par toutes les coques. */
export const iconMap: Record<string, Component> = {
  "layout-dashboard": LayoutDashboard,
  stethoscope: Stethoscope, activity: Activity, zap: Zap,
  "layout-grid": LayoutGrid, wrench: Wrench, download: Download,
  package: Package, "hard-drive": HardDrive, "refresh-cw": RefreshCw,
  cpu: Cpu, scan: Scan, save: Save, shield: Shield, wifi: Wifi,
  terminal: Terminal, "file-code": FileCode, bot: Bot,
  "book-open": BookOpen, "file-text": FileText, "scroll-text": ScrollText,
  "bar-chart-3": BarChart3, settings: Settings, palette: Palette,
  "trash-2": Trash2, gauge: Gauge, server: Server, globe: Globe,
  bug: Bug, "terminal-square": TerminalSquare, bluetooth: Bluetooth,
  sparkles: Sparkles, copy: Copy, database: Database, "shield-check": ShieldCheck,
  user: User,
  thermometer: Thermometer, "pie-chart": PieChart, files: Files,
  "file-search": FileSearch, hash: Hash, container: Container,
  radio: Radio, network: Network, "code-2": Code2,
  rocket: Rocket, "clipboard-list": ClipboardList, "list-checks": ListChecks,
};

export function getNavIcon(name: string): Component {
  return iconMap[name] ?? LayoutDashboard;
}

/** Icône représentant chaque section (titre de `NavSection`). */
const sectionIconMap: Record<string, Component> = {
  "Système": Monitor,
  "Logiciels": AppWindow,
  "Performance": Gauge,
  "Maintenance & Réparation": Wrench,
  "Stockage & Données": HardDrive,
  "Réseau": Wifi,
  "Terminal & Automatisation": Terminal,
  "Intelligence & Aide": Bot,
  "Personnalisation": Brush,
  "WinPE": Usb,
};

export function getSectionIcon(title: string): Component {
  return sectionIconMap[title] ?? LayoutGrid;
}

/** Libellé court d'une section, pour les barres étroites (onglets, dock, orbite). */
const sectionShortMap: Record<string, string> = {
  "Maintenance & Réparation": "Maintenance",
  "Stockage & Données": "Stockage",
  "Terminal & Automatisation": "Terminal",
  "Intelligence & Aide": "IA & Aide",
};

export function sectionShortLabel(title: string): string {
  return sectionShortMap[title] ?? title;
}
