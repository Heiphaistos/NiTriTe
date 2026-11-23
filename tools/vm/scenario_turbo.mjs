// Mode turbo (gaming puis défaut), bloatware, scan Defender rapide + EICAR, état réel vérifié.
export default async ({ invoke, log }) => {
  const ps = async (s, t = 120) => { const r = await invoke("run_system_command", { cmd: "powershell", args: ["-NoProfile", "-Command", s], timeoutSecs: t }); return (r.stdout + " " + r.stderr).trim(); };
  const step = async (n, f) => { try { log(`OK? ${n} :: ${String(typeof (await (f)) === "string" ? "" : "")}${JSON.stringify(await f()).replace(/\s+/g, " ").slice(0, 400)}`); } catch (e) { log(`EXC ${n} :: ${String(e?.message ?? e).slice(0, 300)}`); } };
  await step("plan avant", () => ps("powercfg /getactivescheme"));
  await step("turbo gaming", () => invoke("apply_turbo_mode", { mode: "gaming" }));
  await step("plan après gaming", () => ps("powercfg /getactivescheme; (Get-ItemProperty HKCU:\System\GameConfigStore -EA SilentlyContinue).GameDVR_Enabled"));
  await step("turbo défaut", () => invoke("apply_turbo_mode", { mode: "default" }));
  await step("plan après défaut", () => ps("powercfg /getactivescheme"));
  await step("bloatware", async () => (await invoke("debloat_remove_bloatware", { apps: [] })).map((r) => `${r.action ?? r.name}:${r.success}:${r.message}`).slice(0, 12));
  await step("EICAR", () => ps("md C:\eicar -f | Out-Null; Set-Content C:\eicar\e.txt ('X5O!P%@AP[4\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*') -NoNewline; 'ok'"));
  await step("scan perso EICAR", () => ps("Start-MpScan -ScanType CustomScan -ScanPath 'C:\eicar' -ErrorAction Stop; (Get-MpThreatDetection | Measure).Count", 600));
};
