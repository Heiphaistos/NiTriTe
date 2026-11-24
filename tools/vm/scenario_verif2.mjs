// Après correctifs : CSP de l'éditeur de thème, page Réseau (chargement), sortie diskpart, fermeture.
export default async ({ page, invoke, go, shot, log }) => {
  log("ferm.txt", (await invoke("run_system_command", { cmd: "powershell", args: ["-NoProfile", "-Command", "Get-Content $env:PUBLIC\ferm.txt"], timeoutSecs: 30 })).stdout);
  await page.waitForTimeout(15000);
  await go("/theme-editor", 4000); await shot("theme-editor");
  const t0 = Date.now(); await go("/network", 1000);
  for (let i = 0; i < 60; i++) { if (!(await page.locator("text=Analyse du réseau").count())) break; await page.waitForTimeout(1000); }
  log("réseau chargé en", Date.now() - t0, "ms"); await shot("network");
  await go("/winpe", 5000); await shot("winpe");
  const tn = (await invoke("run_system_command", { cmd: "powershell", args: ["-NoProfile", "-Command", "(Get-Disk | ? { -not $_.IsSystem -and -not $_.IsBoot } | select -First 1).Number"], timeoutSecs: 30 })).stdout.trim();
  const r = await invoke("disk_wipe", { diskIndex: Number(tn), method: "quick" }).catch((e) => String(e));
  log("diskpart", JSON.stringify(r).slice(0, 300));
};
