/**
 * Bandeau d'etat de l'agent (connexion perdue / jeton invalide) et maintien
 * du WebSocket. Charge des le premier import de la cale `core`.
 */
import { agentStatus, keepAlive } from "./agent";
import { mountAgentMenu } from "./agent-menu";

const MESSAGES = {
  offline: "NiTriTe Agent ne répond plus. Relancez nitrite-agent.exe — la page se reconnectera automatiquement.",
  unauthorized: "Session expirée : relancez nitrite-agent.exe pour rouvrir le panneau (le lien contient un nouveau jeton).",
} as const;

function render(status: string): void {
  let el = document.getElementById("nitrite-agent-banner");
  if (status === "online") { el?.remove(); return; }
  if (!el) {
    el = document.createElement("div");
    el.id = "nitrite-agent-banner";
    el.setAttribute("role", "alert");
    el.style.cssText = "position:fixed;left:50%;bottom:36px;transform:translateX(-50%);z-index:100000;"
      + "max-width:640px;padding:10px 16px;border-radius:10px;font:13px system-ui,sans-serif;"
      + "background:#7f1d1d;color:#fff;border:1px solid #ef4444;box-shadow:0 8px 24px rgba(0,0,0,.4)";
    document.body.appendChild(el);
  }
  el.textContent = MESSAGES[status as keyof typeof MESSAGES] ?? "";
}

agentStatus.on(render);

function start(): void {
  keepAlive();
  mountAgentMenu();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", start);
} else {
  start();
}
