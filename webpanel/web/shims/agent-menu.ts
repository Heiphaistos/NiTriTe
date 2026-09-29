/**
 * Menu « Agent » du panneau web : pastille discrete en bas a droite
 * (au-dessus de la barre d'etat) qui indique l'etat de la connexion et donne
 * acces a la version, a la mise a jour disponible et a l'arret de l'agent.
 *
 * Volontairement en DOM brut (pas de composant Vue) : l'interface de NiTriTe
 * reste strictement identique a celle de l'application native.
 */
import { agentFetch, agentStatus } from "./agent";

export interface AgentInfo {
  version: string;
  nitriteVersion: string;
  port: number;
  lan: boolean;
  clients: number;
  idleMinutes: number;
  uptimeSeconds: number;
  update: { version: string; url: string } | null;
  logPath: string | null;
}

export function formatUptime(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return h > 0 ? `${h} h ${m} min` : `${m} min`;
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, css: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  e.style.cssText = css;
  if (text !== undefined) e.textContent = text;
  return e;
}

const DOT_COLORS = { online: "var(--success, #22c55e)", offline: "var(--danger, #ef4444)", unauthorized: "var(--warning, #eab308)" };

let info: AgentInfo | null = null;
let pill: HTMLButtonElement | null = null;
let dot: HTMLSpanElement | null = null;
let menu: HTMLDivElement | null = null;

async function refreshInfo(): Promise<void> {
  try {
    info = await agentFetch<AgentInfo>("/agent", undefined, "GET");
  } catch {
    info = null;
  }
  paintPill();
}

function paintPill(): void {
  if (!pill || !dot) return;
  const status = agentStatus.current;
  dot.style.background = info?.update && status === "online" ? "var(--accent-primary, #f97316)" : DOT_COLORS[status];
  pill.title = status === "online"
    ? (info?.update ? `Nouvelle version de l'agent : ${info.update.version}` : "NiTriTe Agent connecté")
    : "NiTriTe Agent déconnecté";
}

function closeMenu(): void {
  menu?.remove();
  menu = null;
  document.removeEventListener("mousedown", onOutside, true);
  document.removeEventListener("keydown", onEscape, true);
}

function onOutside(e: MouseEvent): void {
  if (menu && !menu.contains(e.target as Node) && e.target !== pill && !pill?.contains(e.target as Node)) closeMenu();
}

function onEscape(e: KeyboardEvent): void {
  if (e.key === "Escape") closeMenu();
}

function row(label: string, value: string): HTMLDivElement {
  const r = el("div", "display:flex;justify-content:space-between;gap:16px;padding:3px 0");
  r.append(el("span", "color:var(--text-muted,#71717a)", label), el("span", "color:var(--text-primary,#fafafa);font-family:'JetBrains Mono',monospace", value));
  return r;
}

async function stopAgent(): Promise<void> {
  if (!window.confirm("Arrêter NiTriTe Agent ?\n\nLe panneau ne répondra plus jusqu'au prochain lancement de NiTriTe-Agent.exe.")) return;
  try { await agentFetch("/host/shutdown"); } catch { /* il s'arrete deja */ }
  closeMenu();
  document.body.replaceChildren(el("div",
    "position:fixed;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;"
    + "background:var(--bg-primary,#09090b);color:var(--text-primary,#fafafa);font:15px system-ui,sans-serif;text-align:center;padding:24px",
    "NiTriTe Agent est arrêté. Vous pouvez fermer cet onglet — relancez NiTriTe-Agent.exe pour rouvrir le panneau."));
}

async function openMenu(): Promise<void> {
  if (menu) { closeMenu(); return; }
  await refreshInfo();
  menu = el("div",
    "position:fixed;right:12px;bottom:68px;z-index:100001;min-width:260px;padding:12px 14px;border-radius:10px;"
    + "background:var(--bg-secondary,#111114);border:1px solid var(--border,#2e2e33);box-shadow:0 10px 30px rgba(0,0,0,.45);"
    + "font:12px system-ui,sans-serif;color:var(--text-secondary,#a1a1aa)");
  menu.setAttribute("role", "dialog");
  menu.setAttribute("aria-label", "NiTriTe Agent");
  menu.append(el("div", "font-weight:700;font-size:13px;color:var(--text-primary,#fafafa);margin-bottom:8px", "NiTriTe Agent"));
  if (!info) {
    menu.append(el("div", "color:var(--danger,#ef4444)", "Agent injoignable."));
  } else {
    menu.append(
      row("Agent", `v${info.version}`),
      row("NiTriTe", `v${info.nitriteVersion}`),
      row("Adresse", `127.0.0.1:${info.port}${info.lan ? " (réseau local)" : ""}`),
      row("Onglets connectés", String(info.clients)),
      row("Actif depuis", formatUptime(info.uptimeSeconds)),
    );
    if (info.idleMinutes > 0) {
      menu.append(el("div", "margin-top:8px;line-height:1.4", `S'arrête seul ${info.idleMinutes} min après la fermeture du dernier onglet.`));
    }
    if (info.update) {
      const a = el("a", "display:block;margin-top:10px;padding:7px 10px;border-radius:6px;text-decoration:none;font-weight:600;"
        + "background:var(--accent-muted,rgba(249,115,22,.12));color:var(--accent-primary,#f97316)",
        `Nouvelle version disponible : v${info.update.version}`);
      a.href = info.update.url;
      a.target = "_blank";
      a.rel = "noopener noreferrer";
      menu.append(a);
    }
    if (info.logPath) {
      menu.append(el("div", "margin-top:8px;font-size:11px;word-break:break-all;color:var(--text-muted,#71717a)", `Journal : ${info.logPath}`));
    }
    const stop = el("button",
      "margin-top:12px;width:100%;padding:7px;border-radius:6px;cursor:pointer;font:600 12px system-ui,sans-serif;"
      + "background:transparent;color:var(--danger,#ef4444);border:1px solid var(--danger,#ef4444)",
      "Arrêter l'agent");
    stop.type = "button";
    stop.addEventListener("click", () => { void stopAgent(); });
    menu.append(stop);
  }
  document.body.appendChild(menu);
  document.addEventListener("mousedown", onOutside, true);
  document.addEventListener("keydown", onEscape, true);
}

export function mountAgentMenu(): void {
  if (pill) return;
  pill = el("button",
    "position:fixed;right:12px;bottom:32px;z-index:100000;display:flex;align-items:center;gap:6px;padding:4px 10px;"
    + "border-radius:999px;cursor:pointer;font:600 11px system-ui,sans-serif;"
    + "background:var(--bg-secondary,#111114);color:var(--text-secondary,#a1a1aa);border:1px solid var(--border,#2e2e33)");
  pill.type = "button";
  pill.setAttribute("aria-haspopup", "dialog");
  dot = el("span", "width:7px;height:7px;border-radius:50%;display:inline-block");
  pill.append(dot, document.createTextNode("Agent"));
  pill.addEventListener("click", () => { void openMenu(); });
  document.body.appendChild(pill);
  agentStatus.on(paintPill);
  void refreshInfo();
}
