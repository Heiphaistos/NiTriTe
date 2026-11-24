// Analyse de `driverquery /FO CSV /V`.
//
// Colonnes réelles (identiques en FR et EN, seuls les en-têtes sont traduits) :
// 0 Module, 1 Nom complet, 2 Description, 3 Type, 4 Mode de démarrage,
// 5 État, 6 Statut, 7-8 Accepter arrêt/suspension, 9-11 tailles,
// 12 Link Date, 13 Path, 14 Init.
// L'ancien parseur lisait l'état en colonne 2 (Description) et la date en
// colonne 5 (État) : badge d'état = description, âge du pilote toujours « ? ».

export interface DriverQueryRow {
  module: string;
  displayName: string;
  driverType: string;
  linkDate: string;
  state: string;
  path: string;
}

function splitCsvLine(line: string): string[] {
  return (line.match(/("([^"]*)"|[^,]+)/g) ?? []).map((c) => c.replace(/^"|"$/g, "").trim());
}

export function parseDriverQueryCsv(csv: string): DriverQueryRow[] {
  const lines = csv.trim().split(/\r?\n/).filter((l) => l.trim());
  if (lines.length < 2) return [];
  const rows: DriverQueryRow[] = [];
  for (const line of lines.slice(1)) {
    const c = splitCsvLine(line);
    if (c.length < 6) continue;
    rows.push({
      module: c[0] ?? "",
      displayName: c[1] ?? "",
      driverType: c[3] ?? "",
      state: c[5] ?? "",
      linkDate: c[12] ?? "",
      path: c[13] ?? "",
    });
  }
  return rows;
}
