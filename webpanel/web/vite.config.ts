/**
 * Build du panneau web : l'interface de NiTriTe (dossier `src/` de
 * l'application) compilee telle quelle, les modules `@tauri-apps/*` etant
 * remplaces par les cales de `shims/` qui parlent a NiTriTe Agent en HTTP.
 * Aucune copie de l'interface : une amelioration de NiTriTe profite
 * automatiquement au panneau web.
 */
import { defineConfig, type Plugin } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "path";
import { readFileSync } from "fs";

// Racine de l'application NiTriTe (reecrit par scripts/export-standalone.sh).
const UPSTREAM = resolve(__dirname, "../..");
const shim = (f: string) => resolve(__dirname, "shims", f);
const pkg = JSON.parse(readFileSync(resolve(UPSTREAM, "package.json"), "utf-8")) as { version: string };

/** Avant tout autre script : capture du jeton (#t=...) et marqueur « contexte
 *  applicatif » (`isTauriContext()` de NiTriTe teste __TAURI_INTERNALS__). */
function agentBootstrap(): Plugin {
  return {
    name: "nitrite-agent-bootstrap",
    transformIndexHtml(html) {
      const boot = `<script>
(function(){
  window.__TAURI_INTERNALS__ = { webpanel: true };
  var m = location.hash.match(/(?:^#|&)t=([0-9a-f]{16,})/i);
  if (m) {
    try { sessionStorage.setItem("nitrite-agent-token", m[1]); } catch (e) {}
    history.replaceState(null, "", location.pathname + location.search);
    fetch("/api/session", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ token: m[1] }) });
  }
})();
</script>`;
      return html
        .replace(/<title>[^<]*<\/title>/, "<title>NiTriTe — Panneau web</title>")
        .replace("<head>", "<head>\n    " + boot);
    },
  };
}

export default defineConfig({
  root: UPSTREAM,
  publicDir: resolve(UPSTREAM, "public"),
  plugins: [agentBootstrap(), vue(), tailwindcss()],
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  resolve: {
    alias: [
      { find: "@/", replacement: resolve(UPSTREAM, "src") + "/" },
      { find: /^@tauri-apps\/api\/core$/, replacement: shim("core.ts") },
      { find: /^@tauri-apps\/api\/event$/, replacement: shim("event.ts") },
      { find: /^@tauri-apps\/api\/window$/, replacement: shim("window.ts") },
      { find: /^@tauri-apps\/api\/path$/, replacement: shim("path.ts") },
      { find: /^@tauri-apps\/plugin-dialog$/, replacement: shim("dialog.ts") },
      { find: /^@tauri-apps\/plugin-fs$/, replacement: shim("fs.ts") },
      { find: /^@tauri-apps\/plugin-shell$/, replacement: shim("shell.ts") },
      { find: /^@tauri-apps\/plugin-opener$/, replacement: shim("shell.ts") },
      { find: /^@tauri-apps\/plugin-process$/, replacement: shim("process.ts") },
      { find: /^@tauri-apps\/plugin-updater$/, replacement: shim("updater.ts") },
      { find: /^@tauri-apps\/plugin-(os|notification|http)$/, replacement: shim("generic.ts") },
    ],
  },
  build: {
    outDir: resolve(__dirname, "dist"),
    emptyOutDir: true,
    target: "es2021",
    chunkSizeWarningLimit: 1500,
  },
  server: {
    port: 5176,
    // Developpement : l'API est servie par l'agent lance a cote (--no-browser).
    proxy: { "/api": { target: "http://127.0.0.1:7878", ws: true } },
    fs: { allow: [UPSTREAM, __dirname] },
  },
});
