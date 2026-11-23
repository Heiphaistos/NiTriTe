// Pilote NiTriTe dans la VM par CDP brut (WebSocket de la page, sans Playwright : indépendant
// de la version de WebView2). Usage : node vm_cdp.mjs <ip> <scenario.mjs> [sortie]
// Le scénario exporte `default async ({ page, invoke, go, shot, log }) => {}`.
import { pathToFileURL } from "node:url";
import { mkdirSync, writeFileSync } from "node:fs";

const [ip, scenarioPath, out = "vm-shots"] = process.argv.slice(2);
mkdirSync(out, { recursive: true });
const targets = await (await fetch(`http://${ip}:9334/json/list`)).json();
const target = targets.find((t) => t.type === "page" && t.url.includes("tauri.localhost")) ?? targets[0];
const ws = new WebSocket(target.webSocketDebuggerUrl.replace("127.0.0.1:9333", `${ip}:9334`));
await new Promise((ok, ko) => { ws.onopen = ok; ws.onerror = ko; });
let seq = 0;
const pending = new Map();
const errors = [];
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
  if (msg.method === "Runtime.exceptionThrown") errors.push(msg.params.exceptionDetails?.exception?.description ?? "exception");
  if (msg.method === "Runtime.consoleAPICalled" && msg.params.type === "error") errors.push(msg.params.args.map((a) => a.value ?? a.description).join(" "));
  if (msg.method === "Log.entryAdded" && msg.params.entry.level === "error") errors.push(msg.params.entry.text);
};
const send = (method, params = {}) => new Promise((ok) => { const id = ++seq; pending.set(id, ok); ws.send(JSON.stringify({ id, method, params })); });
await send("Runtime.enable"); await send("Log.enable");

const evaluate = async (expression) => {
  const r = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (r.result?.exceptionDetails) throw new Error(r.result.exceptionDetails.exception?.description ?? r.result.exceptionDetails.text);
  return r.result?.result?.value;
};
const page = {
  evaluate: (fn, arg) => evaluate(`(${fn})(${JSON.stringify(arg)})`),
  waitForTimeout: (ms) => new Promise((r) => setTimeout(r, ms)),
  text: (sel = "main") => evaluate(`(document.querySelector(${JSON.stringify(sel)})||document.body).innerText`),
};
const invoke = (cmd, args = {}) => evaluate(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args)})`);
const go = async (route, wait = 2500) => {
  await evaluate(`history.pushState({}, "", ${JSON.stringify(route)}); dispatchEvent(new PopStateEvent("popstate"))`);
  await page.waitForTimeout(wait);
};
const shot = async (name) => {
  const r = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(`${out}/${name}.png`, Buffer.from(r.result.data, "base64"));
};
const log = (...a) => console.log(...a);

const scenario = (await import(pathToFileURL(scenarioPath).href)).default;
try { await scenario({ page, invoke, go, shot, log }); }
finally {
  log("ERREURS_CONSOLE", JSON.stringify(errors.slice(0, 40)));
  ws.close();
}
