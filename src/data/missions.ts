/**
 * Modèles de mission : enchaînements d'outils pour une intervention type.
 * Chaque étape ouvre une page existante de NiTriTe ; la mission ne fait que
 * guider (ordre, suivi, rapport), chaque outil garde ses propres boutons.
 */
export interface MissionStepTemplate {
  /** Route d'une page du menu (navigation.ts). */
  route: string;
  /** Ce qu'il faut faire dans l'outil à cette étape. */
  hint: string;
}

export interface MissionTemplate {
  id: string;
  name: string;
  description: string;
  steps: MissionStepTemplate[];
  /** Modèle créé par l'utilisateur (modifiable / supprimable). */
  custom?: boolean;
}

export const BUILTIN_MISSIONS: MissionTemplate[] = [
  {
    id: "remise-en-etat",
    name: "Remise en état PC",
    description: "Le grand classique de l'atelier : diagnostic, nettoyage, pilotes, mises à jour, antivirus, rapport.",
    steps: [
      { route: "/diagnostic", hint: "Lancer « Scan PC » et noter les problèmes relevés." },
      { route: "/cleaner", hint: "Analyser puis nettoyer fichiers temporaires et caches." },
      { route: "/drivers", hint: "Sauvegarder puis mettre à jour les pilotes obsolètes." },
      { route: "/updates", hint: "Installer les mises à jour Windows et logiciels." },
      { route: "/scanvirus", hint: "Lancer une analyse antivirus complète." },
      { route: "/stats-reports", hint: "Générer le rapport d'intervention pour le client." },
    ],
  },
  {
    id: "pc-lent",
    name: "PC lent",
    description: "Trouver ce qui ralentit la machine et la remettre en forme.",
    steps: [
      { route: "/monitoring", hint: "Repérer les processus qui saturent CPU, RAM ou disque." },
      { route: "/optimizations", hint: "Désactiver les programmes au démarrage inutiles, appliquer les réglages." },
      { route: "/cleaner", hint: "Libérer de l'espace disque." },
      { route: "/turbo-mode", hint: "Activer le Turbo Mode si besoin." },
      { route: "/benchmark", hint: "Mesurer le résultat avec un benchmark." },
    ],
  },
  {
    id: "pc-neuf",
    name: "Préparation PC neuf",
    description: "Installer les logiciels, les dépendances et sécuriser la machine avant livraison.",
    steps: [
      { route: "/master-install", hint: "Sélectionner et installer le pack de logiciels du client." },
      { route: "/dependencies", hint: "Installer .NET, Visual C++ et runtimes." },
      { route: "/updates", hint: "Appliquer toutes les mises à jour." },
      { route: "/optimizations", hint: "Appliquer les optimisations recommandées." },
      { route: "/restore-points", hint: "Créer un point de restauration « Livraison »." },
    ],
  },
  {
    id: "ecran-bleu",
    name: "Après un écran bleu",
    description: "Comprendre un BSOD et corriger sa cause probable.",
    steps: [
      { route: "/bsod-analyzer", hint: "Analyser les derniers écrans bleus et le pilote en cause." },
      { route: "/drivers", hint: "Mettre à jour ou restaurer le pilote fautif." },
      { route: "/diagnostic", hint: "Vérifier RAM, disques et températures." },
      { route: "/temperatures", hint: "Contrôler la chauffe en charge." },
      { route: "/restore-points", hint: "Créer un point de restauration une fois stable." },
    ],
  },
  {
    id: "reseau",
    name: "Problème réseau",
    description: "Internet lent ou coupé : du diagnostic à la correction.",
    steps: [
      { route: "/network", hint: "Tester la connexion, la passerelle et la résolution DNS." },
      { route: "/dns-switcher", hint: "Essayer un DNS public fiable." },
      { route: "/wifi-analyzer", hint: "Vérifier le signal et l'encombrement des canaux." },
      { route: "/hosts-editor", hint: "Contrôler le fichier hosts (redirections suspectes)." },
      { route: "/port-scanner", hint: "Vérifier les ports ouverts si nécessaire." },
    ],
  },
  {
    id: "sauvegarde",
    name: "Sauvegarde & migration",
    description: "Mettre les données du client à l'abri avant une intervention lourde.",
    steps: [
      { route: "/backup", hint: "Sauvegarder les profils et documents." },
      { route: "/hash-checker", hint: "Vérifier l'intégrité des fichiers copiés." },
      { route: "/clone", hint: "Cloner le disque si changement de matériel." },
    ],
  },
];
