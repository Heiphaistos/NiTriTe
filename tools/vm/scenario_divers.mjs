// Garde-fous WinPE hors WinPE, gestionnaire de démarrage, benchmark disque, températures.
export default async ({ invoke, log }) => {
  const ps = async (s, t = 120) => (await invoke("run_system_command", { cmd: "powershell", args: ["-NoProfile", "-Command", s], timeoutSecs: t })).stdout.trim();
  const step = async (name, fn) => {
    try { log(`OK? ${name} :: ${String(JSON.stringify(await fn())).replace(/\s+/g, " ").slice(0, 300)}`); }
    catch (e) { log(`REFUS/EXC ${name} :: ${String(e?.message ?? e).replace(/\s+/g, " ").slice(0, 300)}`); }
  };
  await step("WinPE reset MDP hors WinPE (doit refuser)", () => invoke("reset_user_password", { windowsDir: "C:\Windows", username: "dev", newPassword: "x" }));
  await step("WinPE activer compte hors WinPE (doit refuser)", () => invoke("enable_offline_account", { windowsDir: "C:\Windows", username: "Administrateur" }));
  await step("WinPE effacer disque 0 système (doit refuser)", () => invoke("disk_wipe", { diskIndex: 0, method: "quick" }));
  const tn = await ps("(Get-Disk | ? { -not $_.IsSystem -and -not $_.IsBoot } | select -First 1).Number");
  await step(`WinPE effacer disque test ${tn} (VHDX de test)`, () => invoke("disk_wipe", { diskIndex: Number(tn), method: "quick" }));
  await step("disque test après", () => ps(`(Get-Disk -Number ${tn}).PartitionStyle`));

  const cfg = await invoke("get_boot_config").catch((e) => ({ err: String(e) }));
  log("boot config", JSON.stringify(cfg).slice(0, 500));
  await step("bcdedit copy test", () => ps("bcdedit /copy '{current}' /d nitrite-test"));
  const cfg2 = await invoke("get_boot_config").catch(() => ({}));
  const entries = cfg2.entries ?? cfg2.boot_entries ?? [];
  const t = entries.find((e) => /nitrite-test/.test(JSON.stringify(e)));
  log("entrée test", JSON.stringify(t));
  if (t) {
    await step("définir par défaut", () => invoke("set_default_boot", { entryId: t.id }));
    await step("défaut bcdedit", () => ps("(bcdedit /enum '{bootmgr}' | sls 'default').Line"));
    await step("remettre {current}", () => ps("bcdedit /default '{current}'; bcdedit /delete '" + t.id + "' /f"));
  }
  await step("benchmark disque", () => invoke("run_disk_bench", { drive: null }));
  await step("restes bench", () => ps("(gci C:\ -Filter '__nitrite_bench_*' -Force | Measure).Count"));
  await step("températures", () => invoke("get_temperatures"));
  await step("toutes sondes (TjMax/ACPI)", async () => (await invoke("get_all_sensors")).filter?.((s) => /tjmax|acpi|zone/i.test(JSON.stringify(s))).slice(0, 4));
};
