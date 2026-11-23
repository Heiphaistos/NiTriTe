// Arrêt brutal (taskkill /F) par une tâche SYSTEM indépendante, relevé des processus restants,
// puis relance. Résultat dans C:\NiTriTeTest\kill.log (lire après relance avec scenario_ps.mjs).
export default async ({ invoke, log }) => {
  const cmd = [
    "@echo off",
    "timeout /t 3 /nobreak >nul",
    "taskkill /F /IM nitrite.exe > C:\\NiTriTeTest\\kill.log 2>&1",
    "timeout /t 8 /nobreak >nul",
    "powershell -NoProfile -Command \"Get-CimInstance Win32_Process | ? { ($_.Name -match 'nitrite|llama') -or ($_.Name -eq 'msedgewebview2.exe' -and $_.CommandLine -match 'nitrite') } | % { 'RESTE ' + $_.Name + ' ' + $_.ProcessId } | Out-File -Append C:\\NiTriTeTest\\kill.log; 'FIN' | Out-File -Append C:\\NiTriTeTest\\kill.log\"",
    "schtasks /Run /TN NiTriTeTest >> C:\\NiTriTeTest\\kill.log 2>&1",
  ].join("\r\n");
  const ps = "Set-Content -Path C:\\NiTriTeTest\\kill.cmd -Encoding ASCII -Value $env:NIT_CMD; " +
    "schtasks /Create /TN NiTKill /SC ONCE /ST 00:00 /TR C:\\NiTriTeTest\\kill.cmd /RU SYSTEM /RL HIGHEST /F; schtasks /Run /TN NiTKill";
  // Le contenu du .cmd passe par un fichier écrit en base64 : aucune question de guillemets.
  const b64 = Buffer.from(cmd, "utf8").toString("base64");
  const write = `[IO.File]::WriteAllBytes('C:\\NiTriTeTest\\kill.cmd', [Convert]::FromBase64String('${b64}')); ` +
    "schtasks /Create /TN NiTKill /SC ONCE /ST 00:00 /TR C:\\NiTriTeTest\\kill.cmd /RU SYSTEM /RL HIGHEST /F; schtasks /Run /TN NiTKill";
  void ps;
  const r = await invoke("run_system_command", { cmd: "powershell", args: ["-NoProfile", "-Command", write], timeoutSecs: 30 });
  log("tâche", r.success, r.stdout.trim().replace(/\s+/g, " ").slice(0, 200), r.stderr.slice(0, 200));
};
