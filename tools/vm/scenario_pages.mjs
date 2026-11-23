// Ouvre chaque page du menu, attend son chargement, capture, repère erreurs et pages vides.
export default async ({ page, go, shot, log }) => {
  const list = ["/", "/diagnostic", "/monitoring", "/optimizations", "/turbo-mode", "/missions", "/master-install", "/portables", "/uninstaller", "/updates", "/dependencies", "/os-downloads", "/usb-workshop", "/boot-lab", "/temperatures", "/benchmark", "/perf-history", "/stats-reports", "/cleaner", "/tools", "/drivers", "/scanvirus", "/restore-points", "/bsod-analyzer", "/boot-manager", "/disk-visualizer", "/big-files", "/duplicate-finder", "/hash-checker", "/backup", "/data-recovery", "/clone", "/network", "/dns-switcher", "/wifi-analyzer", "/port-scanner", "/bluetooth", "/hosts-editor", "/terminal", "/scripts", "/wsl", "/docker", "/ai-agents", "/knowledge-base", "/documentation", "/settings", "/theme-editor", "/profiles", "/logs", "/winpe"];
  log("routes", list.length);
  for (const r of list) {
    const before = Date.now();
    await go(r, 6000);
    const info = await page.evaluate(() => {
      const main = document.querySelector("main") || document.body;
      const text = main.innerText || "";
      return { len: text.trim().length, err: /erreur|error|échec|failed|undefined|NaN/i.test(text) ? (text.match(/.{0,60}(erreur|error|échec|failed|undefined|NaN).{0,60}/i) || [""])[0] : "" };
    });
    await shot(r === "/" ? "accueil" : r.slice(1).replace(/[^a-z0-9]+/gi, "_"));
    log(`${r}\t${Date.now() - before}ms\ttexte=${info.len}\t${info.err.replace(/\s+/g, " ")}`);
  }
};
