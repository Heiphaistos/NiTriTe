import type { KBCategory } from "../knowledgeBase";

export const kbHardwareCore: KBCategory[] = [
  {
    id: "hardware",
    label: "Matériel & Hardware",
    icon: "Cpu",
    items: [
      {
        title: "CPU - Températures et refroidissement",
        symptoms: "PC qui chauffe, throttling, performances réduites sous charge, ventilateurs à fond",
        solution: [
          "Températures normales : < 70°C en gaming, < 50°C au repos (AMD/Intel récent)",
          "Températures critiques : > 95°C → throttling automatique, > 100°C → BSOD possible",
          "Surveiller avec HWiNFO64 (le plus précis) ou HWMonitor",
          "Nettoyer les ventilateurs et radiateurs à l'air comprimé tous les 6-12 mois",
          "Remplacer la pâte thermique si CPU > 5 ans ou température anormale (Thermal Grizzly Kryonaut)",
          "Vérifier que les ventilateurs du boîtier tournent dans le bon sens (entrée avant/bas, sortie arrière/haut)",
          "Améliorer le flux d'air : câbles bien rangés, filtres à poussière propres",
          "Undervoltage CPU : réduire la tension avec Intel XTU ou AMD Ryzen Master (-50mV à -100mV stable)",
        ],
        command: "powershell -c \"Get-CimInstance Win32_Processor | Select Name, CurrentClockSpeed, MaxClockSpeed, LoadPercentage\"",
      },
      {
        title: "GPU - Diagnostic et optimisation",
        symptoms: "FPS bas, artefacts graphiques, crash jeux, GPU non détecté ou utilisation < 50% anormale",
        solution: [
          "Vérifier utilisation GPU en gaming : doit être proche de 100% (si < 50% → CPU bottleneck)",
          "Températures GPU normales : < 83°C en gaming (certaines cartes jusqu'à 90°C par design)",
          "Mettre à jour pilotes NVIDIA via GeForce Experience ou AMD via Radeon Software",
          "Désinstaller proprement avec DDU (Display Driver Uninstaller) en mode sans échec avant réinstallation",
          "Activer Resizable BAR/SAM dans le BIOS pour +5% FPS gratuits",
          "NVIDIA : activer DLSS dans les jeux supportés (qualité/performance)",
          "AMD : activer FSR dans les jeux supportés",
          "Vérifier alimentation GPU : câble PCIe 8-pin/16-pin bien connecté, PSU suffisant (RTX 4090 = 450W)",
        ],
        command: "dxdiag",
      },
      {
        title: "RAM - Configuration et problèmes",
        symptoms: "PC instable, crashs aléatoires, BSOD, performances inférieures à ce qui est attendu",
        solution: [
          "Vérifier dual-channel : RAM dans les slots A2+B2 (pas A1+B1) selon manuel carte mère",
          "Activer XMP/EXPO dans le BIOS pour atteindre la fréquence annoncée (sinon fonctionne à 2133 MHz par défaut)",
          "Fréquences actuelles avec CPU-Z : onglet Memory, vérifier Frequency et Type (DDR4/DDR5)",
          "Test RAM complet : MemTest86 depuis USB bootable (minimum 2 passages = 4-8h)",
          "Si barrette défaillante : tester une par une pour identifier",
          "Compatibilité : vérifier QVL (Qualified Vendor List) de la carte mère pour les kits validés",
          "DDR5 vs DDR4 : DDR5 obligatoire sur Intel LGA1700 12th-14th gen et AMD AM5",
          "Ordre installation : A2 puis B2 pour dual-channel chez la plupart des fabricants",
        ],
        command: "powershell -c \"Get-CimInstance Win32_PhysicalMemory | Select BankLabel, Capacity, Speed, SMBIOSMemoryType\"",
      },
      {
        title: "Alimentation (PSU) - problèmes et diagnostic",
        symptoms: "PC qui s'éteint sous charge, instabilité, composants non alimentés, redémarrages aléatoires",
        solution: [
          "Puissances recommandées 2024 : RTX 4070=650W, RTX 4080=850W, RTX 4090=1000W minimum",
          "Vérifier le PSU avec OCCT (test PSU pendant 30 min sous charge maximale)",
          "Extinction sous charge = PSU sous-dimensionné ou en fin de vie",
          "Vérifier les câbles d'alimentation PCIe du GPU (bien connectés, bons connecteurs)",
          "Nettoyer ventilateur du PSU (poussière = surchauffe et protection thermique)",
          "Mesurer la qualité du courant avec un multimètre sur les rails +12V/+5V/+3.3V",
          "PSU à remplacer si > 7-8 ans ou s'il ne garantit plus 80% de sa puissance",
          "Bon budget PSU : 15-20% du budget total PC pour une alimentation de qualité",
        ],
      },
      {
        title: "Overclocking - bases et stabilité",
        symptoms: "Vouloir améliorer les performances sans achat de matériel, PC instable après OC",
        solution: [
          "CPU OC : Intel (séries K sur carte mère Z) avec Intel XTU ou via BIOS Ratio Multiplier",
          "AMD OC : Ryzen Master (logiciel) ou BIOS (Precision Boost Overdrive 2 = OC automatique)",
          "RAM OC : activer XMP/EXPO dans le BIOS en premier (OC officiel du fabricant)",
          "GPU OC : MSI Afterburner, augmenter Core Clock +100-200MHz et Memory Clock +200-400MHz progressivement",
          "Tester la stabilité : AIDA64 (CPU/RAM), FurMark (GPU), 30 min minimum sans crash ni artefact",
          "Surveiller les températures pendant les tests : HWiNFO64",
          "Si instable : augmenter la tension (Core Voltage) légèrement ou réduire la fréquence",
          "Règle d'or : stabilité > performance. Un crash OC peut corrompre des données",
        ],
      },
    ],
  }
];
