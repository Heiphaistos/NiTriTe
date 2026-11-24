// Actions qui modifient le système, déclenchées comme le fait l'interface, puis vérifiées
// dans la VM. À ne lancer QUE dans la VM de test (checkpoint avant-NiTriTe-test).
export default async ({ invoke, log }) => {
  const ps = async (script, t = 120) => {
    const r = await invoke("run_system_command", { cmd: "powershell", args: ["-NoProfile", "-Command", script], timeoutSecs: t });
    return (r.stdout || r.stderr || "").trim();
  };
  const step = async (name, fn) => {
    try { log(`OK? ${name} :: ${String(await fn()).replace(/\s+/g, " ").slice(0, 400)}`); }
    catch (e) { log(`EXC ${name} :: ${String(e?.message ?? e).replace(/\s+/g, " ").slice(0, 400)}`); }
  };
  const only = (process.env.STEPS || "").split(",").filter(Boolean);
  const want = (k) => !only.length || only.includes(k);

  if (want("ps")) await step("powershell via run_system_command", () => ps("$PSVersionTable.PSVersion.ToString(); whoami; ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole('Administrators')"));

  if (want("corbeille")) {
    await step("corbeille: 3 fichiers", () => ps("$d='C:\\t_bin'; md $d -f|Out-Null; 1..3|%{ [IO.File]::WriteAllBytes(\"$d\\f$_.bin\", (New-Object byte[] 3MB)) }; Add-Type -AssemblyName Microsoft.VisualBasic; gci $d -File|%{ [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($_.FullName,'OnlyErrorDialogs','SendToRecycleBin') }; (New-Object -ComObject Shell.Application).NameSpace(10).Items().Count"));
    await step("corbeille: empty_recycle_bin", async () => JSON.stringify(await invoke("empty_recycle_bin")));
    await step("corbeille: reste", () => ps("(New-Object -ComObject Shell.Application).NameSpace(10).Items().Count"));
    await step("corbeille: déjà vide", async () => JSON.stringify(await invoke("empty_recycle_bin")));
  }

  if (want("demarrage")) {
    await step("démarrage: création", () => ps("reg add HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run /v TestNit /d C:\\Windows\\notepad.exe /f; reg add HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Run /v TestNit2 /d C:\\Windows\\notepad.exe /f"));
    const progs = await invoke("get_startup_programs").catch((e) => { log("EXC get_startup_programs", e); return []; });
    for (const n of ["TestNit", "TestNit2"]) {
      const p = progs.find((x) => x.name === n);
      await step(`démarrage: disable ${n} (${p?.location})`, async () => p ? JSON.stringify(await invoke("disable_startup_program", { name: p.name, location: p.location })) : "INTROUVABLE dans get_startup_programs");
    }
    await step("démarrage: reste", () => ps("(reg query HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run /v TestNit 2>&1) -join ' '; (reg query HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Run /v TestNit2 2>&1) -join ' '"));
  }

  if (want("restauration")) {
    await step("restauration: activer + valeur avant", () => ps("Enable-ComputerRestore -Drive 'C:\\'; (Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\SystemRestore' -Name SystemRestorePointCreationFrequency -EA SilentlyContinue).SystemRestorePointCreationFrequency; 'fin'"));
    await step("restauration: création", async () => JSON.stringify(await invoke("create_restore_point_cmd", { description: "test-nitrite" })));
    await step("restauration: valeur après", () => ps("(Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\SystemRestore' -Name SystemRestorePointCreationFrequency -EA SilentlyContinue).SystemRestorePointCreationFrequency; 'fin'"));
    const pts = await invoke("list_restore_points_cmd").catch((e) => { log("EXC list_restore_points", e); return []; });
    log("points", JSON.stringify(pts).slice(0, 300));
    const p = pts.find((x) => String(x.description).includes("test-nitrite"));
    await step("restauration: suppression", async () => p ? JSON.stringify(await invoke("delete_restore_point_cmd", { sequenceNumber: p.sequence_number })) : "point introuvable");
    await step("restauration: reste", () => ps("(Get-ComputerRestorePoint | ? Description -eq 'test-nitrite' | Measure).Count"));
  }

  if (want("hosts")) {
    await step("hosts: ajout", async () => JSON.stringify(await invoke("add_hosts_entry", { ip: "127.0.0.1", hostname: "test.nitrite.local", comment: "essai" })));
    await step("hosts: doublon refusé", async () => JSON.stringify(await invoke("add_hosts_entry", { ip: "127.0.0.1", hostname: "test.nitrite.local", comment: "" })));
    await step("hosts: bak + BOM", () => ps("$h=\"$env:SystemRoot\\System32\\drivers\\etc\"; (Test-Path \"$h\\hosts.bak\"); $b=[IO.File]::ReadAllBytes(\"$h\\hosts\"); 'BOM=' + ($b[0] -eq 0xEF); (Select-String -Path \"$h\\hosts\" -Pattern 'test.nitrite.local').Line"));
    const entries = await invoke("get_hosts_entries").catch(() => []);
    const e = entries.find((x) => x.hostname === "test.nitrite.local");
    await step("hosts: désactiver", async () => e ? JSON.stringify(await invoke("toggle_hosts_entry", { lineNumber: e.line_number, enable: false, expectedHostname: e.hostname })) : "entrée introuvable " + JSON.stringify(entries).slice(0, 200));
    const e2 = (await invoke("get_hosts_entries").catch(() => [])).find((x) => x.hostname === "test.nitrite.local");
    await step("hosts: supprimer", async () => e2 ? JSON.stringify(await invoke("delete_hosts_entry", { lineNumber: e2.line_number, expectedHostname: e2.hostname })) : "entrée introuvable");
    await step("hosts: reste", () => ps("(Select-String -Path \"$env:SystemRoot\\System32\\drivers\\etc\\hosts\" -Pattern 'test.nitrite.local' | Measure).Count"));
  }

  if (want("dns")) {
    const ads = await invoke("get_network_adapters_for_dns").catch((e) => { log("EXC adapters", e); return []; });
    log("adaptateurs", JSON.stringify(ads).slice(0, 300));
    const a = ads[0]?.name ?? ads[0];
    await step("dns: cloudflare v4+v6", async () => await invoke("switch_dns", { adapter: a, primary: "1.1.1.1", secondary: "1.0.0.1", ipv6: ["2606:4700:4700::1111", "2606:4700:4700::1001"] }));
    await step("dns: état", () => ps(`(Get-DnsClientServerAddress -InterfaceAlias '${a}' | % { $_.ServerAddresses }) -join ','`));
    await step("dns: auto", async () => await invoke("switch_dns", { adapter: a, primary: "", secondary: "", ipv6: [] }));
    await step("dns: état auto", () => ps(`(Get-DnsClientServerAddress -InterfaceAlias '${a}' | % { $_.ServerAddresses }) -join ','`));
  }

  if (want("mdns")) {
    await step("debloat: disable_mdns", async () => JSON.stringify(await invoke("debloat_run_extra", { action: "disable_mdns" })));
    await step("debloat: EnableMDNS", () => ps("(Get-ItemProperty 'HKLM:\\SYSTEM\\CurrentControlSet\\Services\\Dnscache\\Parameters' -Name EnableMDNS -EA SilentlyContinue).EnableMDNS"));
  }

  if (want("nettoyeur")) {
    await step("nettoyeur: WER + CBS avant", () => ps("md \"$env:LOCALAPPDATA\\Microsoft\\Windows\\WER\\ReportArchive\\x\" -f|Out-Null; sc \"$env:LOCALAPPDATA\\Microsoft\\Windows\\WER\\ReportArchive\\x\\r.txt\" 1; (gci C:\\Windows\\Logs\\CBS).Name -join ','"));
    for (const t of ["Logs CBS", "Windows Error Reports"]) await step(`nettoyeur: ${t}`, async () => JSON.stringify(await invoke("clean_target", { targetName: t })));
    await step("nettoyeur: après", () => ps("(gci C:\\Windows\\Logs\\CBS).Name -join ','; (gci \"$env:LOCALAPPDATA\\Microsoft\\Windows\\WER\\ReportArchive\" -Recurse -EA SilentlyContinue | Measure).Count"));
  }

  if (want("install")) {
    await step("winget présent", () => ps("winget --version", 60));
    await step("install 7-Zip", async () => JSON.stringify(await invoke("install_app", { appId: null, wingetId: "7zip.7zip" })));
    await step("7-Zip présent", () => ps("Test-Path 'C:\\Program Files\\7-Zip\\7z.exe'"));
    await step("réinstall 7-Zip (pas de choco/scoop)", async () => JSON.stringify(await invoke("install_app", { appId: null, wingetId: "7zip.7zip" })));
    await step("choco/scoop absents", () => ps("'choco=' + (Test-Path C:\\ProgramData\\chocolatey) + ' scoop=' + (Test-Path \"$env:USERPROFILE\\scoop\")"));
    const apps = await invoke("list_installed_apps_for_uninstall").catch((e) => { log("EXC list apps", e); return []; });
    const z = apps.find((x) => /7-Zip/i.test(x.name));
    await step("résidus 7-Zip (aperçu)", async () => z ? JSON.stringify(await invoke("preview_residuals", { appName: z.name, publisher: z.publisher })) : "7-Zip absent de la liste");
    await step("désinstall propre 7-Zip", async () => z ? JSON.stringify(await invoke("uninstall_app_clean", { appName: z.name, uninstallString: z.uninstall_string, publisher: z.publisher })) : "absent");
    await step("7-Zip après", () => ps("Test-Path 'C:\\Program Files\\7-Zip\\7z.exe'; Test-Path \"$env:APPDATA\\Microsoft\""));
  }
};
