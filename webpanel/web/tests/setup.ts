// Pas d'agent pendant les tests : WebSocket factice (sinon happy-dom tente
// une vraie connexion et journalise ECONNREFUSED).
class FakeWebSocket {
  static readonly OPEN = 1;
  readyState = 0;
  onopen: (() => void) | null = null;
  onmessage: ((m: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  constructor(public url: string) {}
  send(): void {}
  close(): void {}
}
Object.defineProperty(globalThis, "WebSocket", { value: FakeWebSocket, writable: true });

// Idem pour HTTP : la pastille « Agent » interroge /api/agent au chargement.
// Les tests qui en ont besoin remplacent fetch par un vi.spyOn.
Object.defineProperty(globalThis, "fetch", {
  value: async () => new Response("null", { status: 503 }),
  writable: true,
});
