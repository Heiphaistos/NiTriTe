// Gestionnaire de partitions sur un VHDX de test ajouté à la VM + garde-fous sur le disque système.
export default async ({ invoke, log }) => {
  const ps = async (s) => (await invoke("run_system_command", { cmd: "powershell", args: ["-NoProfile", "-Command", s], timeoutSecs: 120 })).stdout.trim();
  const step = async (name, fn) => {
    try { log(`OK? ${name} :: ${String(JSON.stringify(await fn())).slice(0, 300)}`); }
    catch (e) { log(`REFUS/EXC ${name} :: ${String(e?.message ?? e).replace(/\s+/g, " ").slice(0, 300)}`); }
  };
  const raw = JSON.parse(await ps("Get-Disk | ? PartitionStyle -eq 'RAW' | select -First 1 Number,UniqueId | ConvertTo-Json"));
  log("disque RAW", JSON.stringify(raw));
  const sys = JSON.parse(await ps("Get-Disk | ? IsSystem | select -First 1 Number,UniqueId | ConvertTo-Json"));
  log("disque système", JSON.stringify(sys));

  await step("initialiser disque système (doit refuser)", () => invoke("initialize_disk_cmd", { diskIndex: sys.Number, style: "GPT", expectedDiskId: sys.UniqueId }));
  await step("initialiser RAW avec mauvais id (doit refuser)", () => invoke("initialize_disk_cmd", { diskIndex: raw.Number, style: "GPT", expectedDiskId: "FAUX-ID" }));
  await step("initialiser RAW en GPT", () => invoke("initialize_disk_cmd", { diskIndex: raw.Number, style: "GPT", expectedDiskId: raw.UniqueId }));
  await step("ré-initialiser (déjà initialisé, doit refuser)", () => invoke("initialize_disk_cmd", { diskIndex: raw.Number, style: "GPT", expectedDiskId: raw.UniqueId }));
  await step("créer partition", () => invoke("create_partition_cmd", { diskIndex: raw.Number, sizeMb: null, expectedDiskId: raw.UniqueId }));
  const parts = await invoke("get_partition_list");
  const mine = parts.filter((p) => p.disk_index === raw.Number);
  log("partitions du disque test", JSON.stringify(mine).slice(0, 400));
  const data = mine.find((p) => p.letter);
  await step("formater NTFS", () => invoke("format_partition_cmd", { letter: data?.letter ?? "", fs: "NTFS", label: "TESTNIT", expectedDiskId: data?.disk_unique_id }));
  await step("état après formatage", () => ps(`Get-Volume -DriveLetter '${(data?.letter ?? "Z").replace(":", "")}' | % { $_.FileSystem + ' ' + $_.FileSystemLabel }`));
  const rec = parts.find((p) => p.disk_index === sys.Number && /recovery|récup|reserved|system/i.test(`${p.type_name ?? ""} ${p.partition_type ?? ""} ${p.gpt_type ?? ""}`));
  log("partition récupération/système", JSON.stringify(rec ?? parts.filter((p) => p.disk_index === sys.Number)).slice(0, 400));
  if (rec) await step("supprimer partition récupération (doit refuser)", () => invoke("delete_partition_cmd", { diskIndex: rec.disk_index, partIndex: rec.part_index, expectedDiskId: rec.disk_unique_id }));
  if (data) await step("supprimer partition test", () => invoke("delete_partition_cmd", { diskIndex: data.disk_index, partIndex: data.part_index, expectedDiskId: data.disk_unique_id }));
  await step("partitions restantes test", () => ps(`(Get-Partition -DiskNumber ${raw.Number} | % { $_.Type }) -join ','`));
};
