/**
 * Transport vers NiTriTe Agent : HTTP pour les commandes, WebSocket pour les
 * evenements. Remplace l'IPC de Tauri dans le panneau web.
 */

const TOKEN_KEY = "nitrite-agent-token";

/** Jeton de session : fourni par l'agent dans le fragment `#t=` de l'URL
 *  ouverte, puis garde pour l'onglet (sessionStorage). */
export function agentToken(): string {
  try {
    return sessionStorage.getItem(TOKEN_KEY) ?? "";
  } catch {
    return "";
  }
}

export function captureTokenFromUrl(): void {
  const m = location.hash.match(/(?:^#|&)t=([0-9a-f]{16,})/i);
  if (!m) return;
  try { sessionStorage.setItem(TOKEN_KEY, m[1]); } catch { /* stockage indisponible */ }
  // Efface le jeton de la barre d'adresse (historique, captures d'ecran).
  history.replaceState(history.state, "", location.pathname + location.search);
}

export class AgentError extends Error {
  constructor(public status: number, public payload: unknown) {
    super(typeof payload === "string" ? payload : JSON.stringify(payload));
    this.name = "AgentError";
  }
}

export async function agentFetch<T>(path: string, body?: unknown, method = "POST"): Promise<T> {
  let res: Response;
  try {
    res = await fetch(`/api${path}`, {
      method,
      // Jeton de l'onglet s'il en a un ; sinon le cookie de session (HttpOnly,
      // pose par /api/session) authentifie les autres onglets.
      headers: { "Content-Type": "application/json", ...(agentToken() ? { "X-Nitrite-Token": agentToken() } : {}) },
      body: method === "GET" ? undefined : JSON.stringify(body ?? {}),
    });
  } catch {
    agentStatus.set("offline");
    throw new AgentError(0, "NiTriTe Agent ne répond pas — relancez nitrite-agent.exe");
  }
  if (res.status === 401) agentStatus.set("unauthorized");
  else agentStatus.set("online");
  const text = await res.text();
  let payload: unknown = null;
  try { payload = text ? JSON.parse(text) : null; } catch { payload = text; }
  if (!res.ok) throw new AgentError(res.status, payload);
  return payload as T;
}

// ── Etat de connexion (bandeau dans l'interface) ────────────────────────────

type Status = "online" | "offline" | "unauthorized";
const statusListeners = new Set<(s: Status) => void>();
export const agentStatus = {
  current: "online" as Status,
  set(s: Status) {
    if (s === this.current) return;
    this.current = s;
    statusListeners.forEach((f) => f(s));
  },
  on(f: (s: Status) => void) {
    statusListeners.add(f);
    return () => statusListeners.delete(f);
  },
};

// ── Evenements ───────────────────────────────────────────────────────────────

type Handler = (payload: unknown) => void;
const handlers = new Map<string, Set<Handler>>();
let socket: WebSocket | null = null;
let retry = 0;

function connect(): void {
  if (socket && socket.readyState <= WebSocket.OPEN) return;
  const proto = location.protocol === "https:" ? "wss" : "ws";
  const t = agentToken();
  socket = new WebSocket(`${proto}://${location.host}/api/events${t ? `?token=${encodeURIComponent(t)}` : ""}`);
  socket.onopen = () => { retry = 0; agentStatus.set("online"); };
  socket.onmessage = (m) => {
    try {
      const { event, payload } = JSON.parse(m.data as string) as { event: string; payload: unknown };
      handlers.get(event)?.forEach((h) => { try { h(payload); } catch (e) { console.error(e); } });
    } catch { /* message illisible : ignore */ }
  };
  socket.onclose = () => {
    socket = null;
    if (retry > 2) agentStatus.set("offline");
    // Reconnexion progressive (agent redemarre, veille du PC…).
    const delay = Math.min(10_000, 500 * 2 ** retry++);
    setTimeout(connect, delay);
  };
}

export function onAgentEvent(event: string, h: Handler): () => void {
  if (!handlers.has(event)) handlers.set(event, new Set());
  handlers.get(event)!.add(h);
  connect();
  return () => { handlers.get(event)?.delete(h); };
}

/** Garde le WebSocket ouvert : l'agent s'arrete tout seul quand plus aucun
 *  onglet n'est connecte. */
export function keepAlive(): void {
  connect();
}
