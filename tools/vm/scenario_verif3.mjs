// Vérifie : nouvel exe, plus d'erreur CSP (ipc, styles), sparkline, diskpart lisible, version WinPE.
export default async ({ page, invoke, go, shot, log }) => {
  log("version", await invoke("plugin:app|version").catch(() => "?"));
  for (const r of ["/", "/monitoring", "/theme-editor", "/winpe"]) { await go(r, 5000); await shot(r === "/" ? "accueil" : r.slice(1)); }
  log("installs", JSON.stringify(await invoke("detect_windows_installs").catch((e) => String(e))));
  const tn = (await invoke("run_system_command", { cmd: "powershell", args: ["-NoProfile", "-Command", "(Get-Disk | ? { -not $_.IsSystem -and -not $_.IsBoot } | select -First 1).Number"], timeoutSecs: 30 })).stdout.trim();
  const r = await invoke("disk_wipe", { diskIndex: Number(tn), method: "quick" }).catch((e) => String(e));
  log("diskpart", JSON.stringify(r?.output ?? r).slice(-120));
};
