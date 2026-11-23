// Ferme NiTriTe par le vrai chemin (fermeture de la fenêtre), vérifie qu'aucun processus ne reste,
// remplace l'exe et relance. Un PowerShell détaché fait l'échange après la fermeture.
export default async ({ invoke, log }) => {
  const swap = "Start-Sleep 12; $r = @(Get-Process nitrite,llama-server -EA SilentlyContinue).Count; " +
    "$w = @(Get-CimInstance Win32_Process -Filter \"Name='msedgewebview2.exe'\" | ? { $_.CommandLine -match 'nitrite' }).Count; " +
    "\"restants nitrite/llama=$r webview=$w\" | Out-File C:\NiTriTeTest\fermeture.log; " +
    "Copy-Item C:\NiTriTeTest\nitrite-new.exe C:\NiTriTeTest\nitrite.exe -Force; schtasks /Run /TN NiTriTeTest";
  const r = await invoke("run_system_command", { cmd: "powershell", args: ["-NoProfile", "-Command", `Start-Process powershell -WindowStyle Hidden -ArgumentList '-NoProfile','-Command',${JSON.stringify(swap).replace(/^"|"$/g, "'").replace(/'/g, "''").replace(/^''|''$/g, "'")}`], timeoutSecs: 30 });
  log("détaché", r.success, r.stderr);
  await invoke("plugin:window|close", { label: "main" }).catch((e) => log("close", String(e)));
};
