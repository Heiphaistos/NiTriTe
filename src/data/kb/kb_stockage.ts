import type { KBCategory } from "../knowledgeBase";

export const kbStockage: KBCategory[] = [
  {
    id: "stockage",
    label: "Stockage",
    icon: "HardDrive",
    items: [
      {
        title: "Disque plein",
        symptoms: "Barre d'espace disque rouge, impossible de sauvegarder, applications qui crashent",
        solution: [
          "Lancer le nettoyage de disque avec fichiers système : cleanmgr /d C: /sageset:1 puis cleanmgr /d C: /sagerun:1",
          "Analyser l'espace utilisé avec WinDirStat ou TreeSize Free",
          "Vider la Corbeille",
          "Supprimer les fichiers temporaires : del /q /f /s %TEMP%\\*",
          "Supprimer les anciennes installations Windows : Nettoyage disque > Nettoyer fichiers système > Installations Windows précédentes",
          "Déplacer les gros fichiers (vidéos, ISOs) sur un disque externe",
          "Désinstaller les programmes inutilisés (Ajout/Suppression)",
          "Compresser les anciens dossiers en ZIP ou utiliser Windows Compact",
        ],
        command: "cleanmgr /d C:",
      },
      {
        title: "Disque dur bruyant ou lent",
        symptoms: "Clics, grattements, temps d'accès élevés, Gestionnaire des tâches montre disque à 100%",
        solution: [
          "SAUVEGARDER IMMÉDIATEMENT toutes les données importantes vers un disque externe",
          "Lancer un diagnostic SMART : Get-PhysicalDisk | Get-StorageReliabilityCounter (PowerShell admin)",
          "Utiliser CrystalDiskInfo pour voir l'état SMART détaillé (chercher 'Caution' ou 'Bad')",
          "Lancer chkdsk /f /r (programme un scan au prochain redémarrage)",
          "Si SMART indique 'Reallocated Sectors' élevé : remplacement urgent",
          "Si secteurs défaillants détectés : récupérer les données avec Recuva ou GetDataBack",
          "Vérifier câble SATA (desserré = ralentissement ou erreurs)",
          "Remplacer par un SSD SATA ou NVMe — amélioration de performance x5 minimum",
        ],
        command: "powershell -c \"Get-PhysicalDisk | Select FriendlyName, HealthStatus, MediaType, Size\"",
      },
      {
        title: "SSD santé et optimisation",
        symptoms: "SSD qui ralentit avec le temps, écriture lente, durée de vie inquiétante",
        solution: [
          "Vérifier la santé avec CrystalDiskInfo : regarder 'Total Host Writes' et '% de vie restante'",
          "S'assurer que TRIM est activé : fsutil behavior query DisableDeleteNotify (0 = TRIM actif)",
          "Activer TRIM si désactivé : fsutil behavior set DisableDeleteNotify 0",
          "NE PAS défragmenter un SSD (inutile et réduit la durée de vie)",
          "Garder 15-20% d'espace libre pour permettre l'over-provisioning",
          "Mettre à jour le firmware du SSD depuis le site fabricant (Samsung Magician, etc.)",
          "Désactiver l'hibernation si espace critique : powercfg /h off",
          "Vérifier que le SSD est en mode AHCI dans le BIOS (pas IDE)",
        ],
        command: "fsutil behavior query DisableDeleteNotify",
      },
      {
        title: "Partitions et gestion de l'espace",
        symptoms: "Partition trop petite, espace non alloué, besoin de redimensionner",
        solution: [
          "Ouvrir Gestion des disques : diskmgmt.msc",
          "Rétrécir une partition : clic droit > Réduire le volume",
          "Étendre une partition : clic droit > Étendre le volume (espace non alloué doit être adjacent à droite)",
          "Si espace non adjacent : utiliser MiniTool Partition Wizard (gratuit) ou AOMEI Partition Assistant",
          "Convertir MBR en GPT sans perte de données : via MBR2GPT.exe (Windows 10/11)",
          "Créer partition de récupération séparée pour images système",
          "Attention : toujours sauvegarder avant toute opération sur les partitions",
        ],
        command: "diskmgmt.msc",
      },
      {
        title: "RAID - configuration et récupération",
        symptoms: "Disque RAID défaillant, rebuild RAID, performance RAID dégradée",
        solution: [
          "RAID 0 (striping) : performance maximale, zéro tolérance aux pannes — 1 disque mort = tout perdu",
          "RAID 1 (mirroring) : redondance totale, 50% de l'espace disponible — résiste à 1 panne",
          "RAID 5 : équilibre performance/redondance, minimum 3 disques, résiste à 1 panne",
          "RAID 10 : RAID 1+0, résiste à plusieurs pannes (1 par paire), coûteux",
          "Pour RAID logiciel Windows : Gestionnaire des disques ou Espaces de stockage",
          "Vérifier l'état RAID : Panneau Intel RST ou AMD RAIDXpert selon contrôleur",
          "Si disque RAID défaillant : remplacer physiquement, REBUILD automatique démarre",
          "Toujours sauvegarder externement même avec RAID — ce n'est PAS un backup",
        ],
      },
    ],
  }
];
