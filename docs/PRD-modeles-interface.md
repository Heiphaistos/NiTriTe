# PRD — Modèles d'interface NiTriTe

## 1. Contexte et objectif

NiTriTe a une seule disposition (barre latérale + en-tête + barre d'état) que l'on
peut ajuster (presets de mise en page, 62 thèmes de couleurs). L'objectif est une
**refonte graphique de toute l'application sous forme de modèles d'interface**
choisis par l'utilisateur, **sans rien changer au fonctionnement** : chaque page,
chaque bouton, chaque catégorie et chaque sous-onglet doit faire exactement la même
chose qu'avant, dans tous les modèles.

## 2. Modèles livrés

| # | Modèle | Navigation | Caractère visuel |
|---|--------|-----------|------------------|
| 01 | **Forge** (par défaut) | Barre latérale riche : recherche, favoris en pastilles, catégories dépliables avec icône et compteur, carte d'état (profil de performance) | Cartes sombres arrondies, accent du thème en dégradé, titres forts |
| — | **Classique** | L'interface historique, inchangée (respecte tous les presets de mise en page) | Identique à la version précédente |
| 02 | **Command Deck** | En-tête avec palette de commandes (Ctrl K), barre de catégories horizontale, menu déroulant détaillé par catégorie | Plein écran, contenu large, menus en panneau |
| 04 | **Bento** | Rail d'icônes des catégories + panneau d'outils avec descriptions | Grandes cartes très arrondies, espacement généreux |
| 05 | **Console** | Arborescence monospace (sections → outils, numérotés) + onglets des outils ouverts | Police mono, bordures fines, look terminal |
| 06 | **Orbital** | Barre supérieure + navigation circulaire (catégories en orbite, outils au centre) ouverte d'un bouton ou du raccourci | Anneaux, lueurs, formes rondes |
| 08 | **Colonnes** | Navigation en colonnes : sections │ outils de la section (avec descriptions) │ contenu, fil d'Ariane avec précédent/suivant | Panneaux nets séparés, style explorateur |
| 09 | **Verre & dock** | Barre supérieure discrète + dock de catégories en bas ; chaque catégorie ouvre un panneau d'outils | Verre dépoli, halos colorés du thème |
| 10 | **Mission** | Onglets des outils ouverts en haut + barre de sections à menus déroulants | Bandeau d'onglets, cartes « étapes » |

Le modèle 03 (HUD) et le modèle 07 (Néo-brutal) ne sont pas retenus.

## 3. Exigences fonctionnelles

1. **Aucune régression fonctionnelle** : les pages ne sont pas modifiées. La zone de
   page (router-view, keep-alive des pages longues, overlay d'erreur, remise à zéro du
   défilement) reste rendue par `App.vue`, identique pour tous les modèles ; seules
   la navigation et l'habillage changent.
2. **Toutes les catégories et tous les outils** sont accessibles dans chaque modèle :
   toutes les coques lisent la même source `src/data/navigation.ts` (10 sections,
   47 outils). Un test vérifie que chaque coque rend chaque entrée.
3. **Mêmes préférences partagées** : favoris (`nitrite-pinned`), sections repliées
   (`nitrite-sections`), recherche globale (Ctrl K), raccourcis (`?`, Ctrl B),
   barre d'état (CPU/RAM/disque/réseau en direct) présente dans chaque modèle.
4. **Choix du modèle** : Paramètres › Interface et Éditeur de thème › Mise en page,
   avec aperçu schématique ; appliqué immédiatement, mémorisé (`nitrite-ui-model`).
   Valeur absente ou inconnue → Forge.
5. **Thèmes** : chaque modèle n'utilise que les variables du thème (`--bg-*`,
   `--accent-*`, `--text-*`, `--border`, `--success`…) : les 62 thèmes et l'éditeur
   de thème fonctionnent dans tous les modèles.
6. **Profil de performance** : en profil léger (`data-perf="light"`), les flous,
   halos et animations des modèles sont coupés.
7. **Sous-catégories** : l'habillage (`ui-models.css`, sélecteurs
   `html[data-ui-model="…"]`) restyle les composants communs utilisés par toutes les
   pages : cartes (`NCard`), boutons (`NButton`), onglets (`NTabs` et onglets
   de diagnostic), champs (`NInput`, `NSearchBar`), badges, tableaux, barres de
   progression, en-têtes de page.

## 4. Architecture

- `src/stores/uiModel.ts` — store Pinia : modèle courant, liste des modèles,
  persistance, attribut `data-ui-model` sur `<html>`.
- `src/data/navIcons.ts` — icônes des outils et des sections (partagées).
- `src/data/navDescriptions.ts` — sous-titre de chaque outil.
- `src/composables/useShellNav.ts` — logique commune : page courante, section,
  navigation, favoris, sections repliées, onglets ouverts (`useOpenTabs`).
- `src/components/shells/*Shell.vue` — une coque par modèle, avec un emplacement
  `<PageSlot>` qui reçoit la zone de page.
- `App.vue` téléporte (`<Teleport defer>`) la zone de page dans le `<PageSlot>` du
  modèle actif : changer de modèle **déplace** la page sans la recréer, donc les
  opérations en cours et l'état des pages (keep-alive compris) sont conservés.
- `src/assets/styles/ui-models.css` — habillage des composants par modèle.
- `src/components/shared/UiModelPicker.vue` — sélecteur avec aperçus.

## 5. Critères d'acceptation

- `vue-tsc` et `vite build` passent ; tous les tests existants passent.
- Nouveaux tests : défaut = Forge, persistance et valeur invalide ; chaque route du
  menu existe ; chaque coque se monte et affiche les 47 entrées (ou les 10
  sections et leurs outils au survol/clic) ; un clic sur une entrée navigue vers la
  bonne route.
- Basculer d'un modèle à l'autre ne recharge pas l'application et ne perd ni les
  opérations en cours ni l'état des pages.

## 6. Hors périmètre

- Modification du contenu ou de la logique des pages et du backend Rust.
- Nouvelles polices web (l'application doit rester utilisable hors ligne).
