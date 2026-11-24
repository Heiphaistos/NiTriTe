import { describe, it, expect } from "vitest";
import { parseDriverQueryCsv } from "../driverquery";

// Sortie réelle de `driverquery /FO CSV /V` sur Windows 11 FR (2026-11-23).
const FR = [
  '"Nom du module","Nom complet","Description","Type de pilote","Mode de démarrage","État","Statut","Accepter l’arrêt","Accepter la suspension","Paged Pool(bytes)","Code(bytes)","BSS(bytes)","Link Date","Path","Init(bytes)"',
  '"1394ohci","Contrôleur d’hôte compatible OHCI 1394","Contrôleur d’hôte compatible OHCI 1394","Kernel ","Manual","Stopped","OK","FALSE","FALSE","4 096","229 376","0","","C:\\WINDOWS\\system32\\drivers\\1394ohci.sys","4 096"',
  '"3ware","3ware","3ware","Kernel ","Manual","Running","OK","FALSE","FALSE","0","81 920","0","19/05/2015 00:28:03","C:\\WINDOWS\\system32\\drivers\\3ware.sys","4 096"',
].join("\r\n");

describe("parseDriverQueryCsv", () => {
  it("lit l'état et la date dans les bonnes colonnes", () => {
    const rows = parseDriverQueryCsv(FR);
    expect(rows).toHaveLength(2);
    expect(rows[0]).toMatchObject({ module: "1394ohci", driverType: "Kernel", state: "Stopped", linkDate: "" });
    expect(rows[1]).toMatchObject({ module: "3ware", state: "Running", linkDate: "19/05/2015 00:28:03" });
    expect(rows[1].path).toContain("3ware.sys");
  });

  it("renvoie une liste vide sans données", () => {
    expect(parseDriverQueryCsv("")).toEqual([]);
    expect(parseDriverQueryCsv('"Nom du module","Nom complet"')).toEqual([]);
  });
});
