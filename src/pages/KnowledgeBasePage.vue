<script setup lang="ts">
import { ref, computed, markRaw, onMounted, type Component } from "vue";
import { invoke, isTauriContext } from "@/utils/invoke";
import { useNotificationStore } from "@/stores/notifications";
import { confirm } from "@tauri-apps/plugin-dialog";
import NCard from "@/components/ui/NCard.vue";
import NSearchBar from "@/components/ui/NSearchBar.vue";
import { knowledgeBase } from "@/data/knowledgeBase";
import type { KBCategory, KBItem } from "@/data/knowledgeBase";
import {
  BookOpen, Wifi, Zap, Shield, HardDrive,
  Monitor, Volume2, Usb, ChevronDown, ChevronRight,
  Terminal, Copy, AlertTriangle, Settings, Cpu,
  CheckCircle, Heart, Printer, Link,
} from "lucide-vue-next";

// Map icon name → composant
const iconMap: Record<string, Component> = {
  Wifi:          markRaw(Wifi),
  Zap:           markRaw(Zap),
  Shield:        markRaw(Shield),
  HardDrive:     markRaw(HardDrive),
  Monitor:       markRaw(Monitor),
  Volume2:       markRaw(Volume2),
  Usb:           markRaw(Usb),
  AlertTriangle: markRaw(AlertTriangle),
  Settings:      markRaw(Settings),
  Cpu:           markRaw(Cpu),
  Terminal:      markRaw(Terminal),
  BookOpen:      markRaw(BookOpen),
  CheckCircle:   markRaw(CheckCircle),
};

// Enrichissement : ajouter les composants icône à la volée
const categories = computed<(KBCategory & { iconComponent: Component })[]>(() =>
  knowledgeBase.map((cat) => ({
    ...cat,
    iconComponent: iconMap[cat.icon] ?? iconMap.Settings,
  }))
);

const notify             = useNotificationStore();
const search             = ref("");
const expandedCategory   = ref<string | null>(null);
const expandedItem       = ref<string | null>(null);
const copiedCmd          = ref<string | null>(null);
const copiedLink         = ref<string | null>(null);

// ── Favoris ────────────────────────────────────────────────────────────────
const FAVORITES_KEY = "nitrite-kb-favorites";
const favorites = ref<Set<string>>(new Set());

function loadFavorites() {
  try {
    const raw = localStorage.getItem(FAVORITES_KEY);
    if (raw) favorites.value = new Set(JSON.parse(raw));
  } catch { /* ignore */ }
}

function saveFavorites() {
  localStorage.setItem(FAVORITES_KEY, JSON.stringify([...favorites.value]));
}

function isFavorite(title: string): boolean {
  return favorites.value.has(title);
}

function toggleFavorite(title: string) {
  if (favorites.value.has(title)) {
    favorites.value.delete(title);
  } else {
    favorites.value.add(title);
  }
  saveFavorites();
  // Trigger reactivity
  favorites.value = new Set(favorites.value);
}

// Tous les articles favoris (avec leur catégorie)
const favoriteItems = computed<Array<KBItem & { categoryId: string; categoryLabel: string }>>(() => {
  const result: Array<KBItem & { categoryId: string; categoryLabel: string }> = [];
  for (const cat of knowledgeBase) {
    for (const item of cat.items) {
      if (favorites.value.has(item.title)) {
        result.push({ ...item, categoryId: cat.id, categoryLabel: cat.label });
      }
    }
  }
  return result;
});

// ── Filtrage ────────────────────────────────────────────────────────────────
function toggleCategory(id: string) {
  expandedCategory.value = expandedCategory.value === id ? null : id;
  expandedItem.value = null;
}

function toggleItem(title: string) {
  expandedItem.value = expandedItem.value === title ? null : title;
}

// Normalise pour une recherche ACCENT-INSENSIBLE : « reseau » doit trouver
// « Réseau ». toLowerCase() seul laisse les accents → pas de correspondance.
function norm(s: string): string {
  return s.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "");
}

const filteredCategories = computed(() => {
  const cats = categories.value;
  if (!search.value.trim()) return cats;
  const q = norm(search.value);
  return cats
    .map((cat) => ({
      ...cat,
      items: cat.items.filter(
        (item) =>
          norm(item.title).includes(q) ||
          norm(item.symptoms ?? "").includes(q) ||
          (item.solution ?? []).some((s) => norm(s).includes(q)) ||
          norm(item.code ?? "").includes(q)
      ),
    }))
    .filter((cat) => cat.items.length > 0);
});

// ── Stats dynamiques ────────────────────────────────────────────────────────
const totalCategories  = computed(() => knowledgeBase.length);
const totalArticles    = computed(() => knowledgeBase.reduce((n, c) => n + c.items.length, 0));
const totalResults     = computed(() => filteredCategories.value.reduce((n, c) => n + c.items.length, 0));

// ── Articles connexes ───────────────────────────────────────────────────────
function relatedItems(item: KBItem, catId: string): KBItem[] {
  const cat = knowledgeBase.find(c => c.id === catId);
  if (!cat) return [];
  return cat.items
    .filter(i => i.title !== item.title)
    .slice(0, 3);
}

// ── Actions ─────────────────────────────────────────────────────────────────
// Commandes a copier seulement : exemples a adapter (proxy, IP, nom de
// logiciel, fichiers video) ou trop longues pour la limite de 60 s cote Rust
// (sfc, DISM, scan Defender, winget --all seraient tues en cours de route).
const COPY_ONLY = /corp\.com|192\.168\.|NomDu|input\.mp4|HandBrakeCLI|^vlc |\bsfc\b|\bdism\b|MpCmdRun|winget upgrade --all|winword/i;
function canRun(cmd: string): boolean { return !COPY_ONLY.test(cmd); }

const cmdOutput  = ref<Record<string, { ok: boolean; text: string }>>({});
const runningCmd = ref<string | null>(null);

async function runCommand(cmd: string) {
  // Plusieurs commandes de la base agissent (redemarrage vers le BIOS,
  // reset reseau, hibernation...) : jamais en un seul clic.
  if (!(await confirm(`Exécuter cette commande sur ce PC ?\n\n${cmd}`, { title: "Base de connaissances", kind: "warning" }))) return;
  runningCmd.value = cmd;
  try {
    // Le resultat etait jete : « Exécuter » ne montrait rien, succes ou echec.
    const r = await invoke<{ success: boolean; stdout: string; stderr: string }>(
      "run_system_command", { cmd: "cmd", args: ["/C", cmd] }, 70_000);
    const text = (r.stdout || r.stderr || "Commande lancée, aucune sortie.").trim();
    cmdOutput.value[cmd] = { ok: r.success, text: text.slice(0, 4000) };
    if (r.success) notify.success("Commande exécutée", cmd.slice(0, 80));
    else notify.warning("Commande en échec", "Voir la sortie sous la commande");
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    cmdOutput.value[cmd] = { ok: false, text: msg };
    if (isTauriContext()) notify.error("Commande", msg.slice(0, 120));
  } finally {
    runningCmd.value = null;
  }
}

async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    copiedCmd.value = text;
    setTimeout(() => { copiedCmd.value = null; }, 2000);
  } catch { /* dev */ }
}

function copyArticleLink(itemId: string) {
  const hash = `#article-${itemId}`;
  try {
    navigator.clipboard.writeText(hash);
  } catch { /* dev */ }
  copiedLink.value = itemId;
  setTimeout(() => { copiedLink.value = null; }, 2000);
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function printArticle(item: KBItem) {
  const content = escapeHtml([
    `# ${item.title}`,
    item.symptoms ? `Symptômes: ${item.symptoms}` : "",
    item.solution ? `Solution:\n${item.solution.map((s, i) => `${i + 1}. ${s}`).join("\n")}` : "",
    item.code ? `Code:\n${item.code}` : "",
    item.note ? `Note: ${item.note}` : "",
  ].filter(Boolean).join("\n\n"));

  const win = window.open("", "_blank");
  if (win) {
    win.document.write(`<pre style="font-family:monospace;white-space:pre-wrap;padding:20px">${content}</pre>`);
    win.document.close();
    win.print();
  }
}

/** Génère un id stable pour un article (slug du titre) */
function articleId(title: string): string {
  return title.toLowerCase().replace(/\s+/g, "-").replace(/[^a-z0-9-]/g, "");
}

onMounted(loadFavorites);
</script>

<template>
  <div class="kb-page">
    <!-- Header -->
    <div class="page-header">
      <div>
        <h1><BookOpen :size="22" /> Base de Connaissances</h1>
        <p class="page-subtitle">{{ totalCategories }} catégories · {{ totalArticles }} articles · Solutions aux problèmes courants</p>
      </div>
    </div>

    <!-- Recherche -->
    <NSearchBar v-model="search" placeholder="Rechercher un problème, une commande, un symptôme…" />

    <!-- Stats rapides -->
    <div class="stats-banner">
      <span class="stat-item"><strong>{{ totalArticles }}</strong> articles</span>
      <span class="stat-sep">·</span>
      <span class="stat-item"><strong>{{ totalCategories }}</strong> catégories</span>
      <span class="stat-sep">·</span>
      <span class="stat-item"><strong>{{ totalResults }}</strong> résultat{{ totalResults > 1 ? 's' : '' }}</span>
      <span v-if="favorites.size > 0" class="stat-sep">·</span>
      <span v-if="favorites.size > 0" class="stat-item stat-favs">
        <Heart :size="11" style="color:var(--danger)" /> {{ favorites.size }} favori{{ favorites.size > 1 ? 's' : '' }}
      </span>
    </div>

    <!-- Section Favoris -->
    <div v-if="favoriteItems.length > 0" class="favorites-section">
      <div class="favorites-header">
        <Heart :size="14" style="color:var(--danger)" />
        <span>Favoris ({{ favoriteItems.length }})</span>
      </div>
      <div class="favorites-list">
        <div
          v-for="fav in favoriteItems"
          :key="fav.title"
          class="fav-chip"
          @click="expandedCategory = fav.categoryId; expandedItem = fav.title"
        >
          <Heart :size="11" style="color:var(--danger); flex-shrink:0" />
          <span>{{ fav.title }}</span>
          <span class="fav-cat">{{ fav.categoryLabel }}</span>
        </div>
      </div>
    </div>

    <!-- Grille de catégories -->
    <div class="categories">
      <div v-for="cat in filteredCategories" :key="cat.id" class="category">
        <!-- En-tête catégorie -->
        <button class="cat-header" @click="toggleCategory(cat.id)">
          <component :is="cat.iconComponent" :size="18" class="cat-icon" />
          <span class="cat-label">{{ cat.label }}</span>
          <span class="cat-count">{{ cat.items.length }} article{{ cat.items.length > 1 ? 's' : '' }}</span>
          <ChevronDown v-if="expandedCategory === cat.id" :size="16" class="chevron" />
          <ChevronRight v-else :size="16" class="chevron" />
        </button>

        <!-- Liste des articles -->
        <div v-if="expandedCategory === cat.id" class="cat-items">
          <div
            v-for="item in cat.items"
            :key="item.title"
            class="kb-item"
            :id="`article-${articleId(item.title)}`"
          >
            <!-- En-tête article -->
            <button class="item-header" @click="toggleItem(item.title)">
              <span class="item-title">{{ item.title }}</span>
              <div class="item-header-actions" @click.stop>
                <!-- Favori -->
                <button
                  class="icon-action-btn"
                  :class="{ active: isFavorite(item.title) }"
                  @click="toggleFavorite(item.title)"
                  :title="isFavorite(item.title) ? 'Retirer des favoris' : 'Ajouter aux favoris'"
                >
                  <Heart :size="12" />
                </button>
                <!-- Copier lien -->
                <button
                  class="icon-action-btn"
                  :class="{ active: copiedLink === articleId(item.title) }"
                  @click="copyArticleLink(articleId(item.title))"
                  title="Copier le lien de l'article"
                >
                  <Link :size="12" />
                </button>
                <!-- Imprimer -->
                <button
                  class="icon-action-btn"
                  @click="printArticle(item)"
                  title="Imprimer l'article"
                >
                  <Printer :size="12" />
                </button>
              </div>
              <ChevronDown v-if="expandedItem === item.title" :size="14" />
              <ChevronRight v-else :size="14" />
            </button>

            <!-- Contenu article -->
            <div v-if="expandedItem === item.title" class="item-content">
              <!-- Symptômes -->
              <div v-if="item.symptoms" class="symptoms">
                <AlertTriangle :size="13" style="color: var(--warning); flex-shrink:0" />
                <span><strong>Symptômes :</strong> {{ item.symptoms }}</span>
              </div>

              <!-- Solutions -->
              <div class="solution">
                <strong>Solution :</strong>
                <ol>
                  <li v-for="(step, i) in item.solution" :key="i">{{ step }}</li>
                </ol>
              </div>

              <!-- Bloc code -->
              <div v-if="item.code" class="code-block">
                <div class="code-header">
                  <Terminal :size="12" />
                  <span>Script / Commandes</span>
                  <button class="copy-btn-sm" @click="copyToClipboard(item.code!)" title="Copier le code">
                    <Copy :size="11" />
                    {{ copiedCmd === item.code ? 'Copié !' : 'Copier' }}
                  </button>
                </div>
                <pre class="code-content">{{ item.code }}</pre>
              </div>

              <!-- Commande rapide -->
              <div v-if="item.command" class="command-block">
                <code>{{ item.command }}</code>
                <button
                  class="copy-btn"
                  :class="{ copied: copiedCmd === item.command }"
                  @click="copyToClipboard(item.command!)"
                  title="Copier"
                >
                  <Copy :size="12" />
                </button>
                <button v-if="canRun(item.command)" class="run-btn" :disabled="runningCmd === item.command" @click="runCommand(item.command!)">
                  <Terminal :size="12" /> {{ runningCmd === item.command ? "En cours…" : "Exécuter" }}
                </button>
              </div>
              <pre v-if="item.command && cmdOutput[item.command]" class="cmd-output" :class="{ failed: !cmdOutput[item.command].ok }">{{ cmdOutput[item.command].text }}</pre>

              <!-- Note -->
              <div v-if="item.note" class="note-block">
                💡 {{ item.note }}
              </div>

              <!-- Articles connexes -->
              <div v-if="relatedItems(item, cat.id).length > 0" class="related-section">
                <span class="related-label">Voir aussi :</span>
                <div class="related-chips">
                  <button
                    v-for="rel in relatedItems(item, cat.id)"
                    :key="rel.title"
                    class="related-chip"
                    @click="toggleItem(rel.title)"
                  >
                    <ChevronRight :size="11" />
                    {{ rel.title }}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Aucun résultat -->
      <div v-if="filteredCategories.length === 0" class="empty-state">
        <BookOpen :size="36" style="opacity: 0.2" />
        <p>Aucun résultat pour <strong>"{{ search }}"</strong></p>
        <p class="empty-hint">Essayez un terme plus général (ex: "réseau", "lent", "BSOD")</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.kb-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.page-header h1 {
  font-size: 22px;
  font-weight: 700;
  display: flex;
  align-items: center;
  gap: 10px;
}

.page-subtitle {
  color: var(--text-muted);
  font-size: 13px;
  margin-top: 4px;
}

/* ── Stats banner ── */
.stats-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  font-size: 12px;
  color: var(--text-secondary);
  flex-wrap: wrap;
}
.stat-item { display: flex; align-items: center; gap: 4px; }
.stat-sep { color: var(--border); }
.stat-favs { color: var(--text-secondary); }

/* ── Favoris ── */
.favorites-section {
  border: 1px solid color-mix(in srgb, var(--danger) 30%, transparent);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--bg-secondary);
}
.favorites-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  background: color-mix(in srgb, var(--danger) 6%, transparent);
  border-bottom: 1px solid color-mix(in srgb, var(--danger) 20%, transparent);
}
.favorites-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 10px 14px;
}
.fav-chip {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border);
  border-radius: 99px;
  font-size: 12px;
  color: var(--text-secondary);
  cursor: pointer;
  transition: border-color .15s, color .15s;
}
.fav-chip:hover { border-color: var(--danger); color: var(--text-primary); }
.fav-cat { font-size: 10px; color: var(--text-muted); margin-left: 2px; }

/* ── Catégories ── */
.categories { display: flex; flex-direction: column; gap: 6px; }

.category {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--bg-secondary);
  transition: border-color 0.15s;
}
.category:has(.cat-header:hover) { border-color: var(--border-hover); }

.cat-header {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 14px 16px;
  border: none;
  background: transparent;
  cursor: pointer;
  font-family: inherit;
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  transition: background 0.15s;
}
.cat-header:hover { background: var(--bg-tertiary); }

.cat-icon { color: var(--accent-primary); flex-shrink: 0; }
.cat-count {
  margin-left: auto;
  font-size: 11px;
  color: var(--text-muted);
  font-weight: 400;
  background: var(--bg-tertiary);
  padding: 2px 8px;
  border-radius: 99px;
  border: 1px solid var(--border);
}
.chevron { color: var(--text-muted); flex-shrink: 0; }

/* ── Articles ── */
.cat-items { border-top: 1px solid var(--border); }

.kb-item { border-bottom: 1px solid var(--border); }
.kb-item:last-child { border-bottom: none; }

.item-header {
  display: flex;
  align-items: center;
  width: 100%;
  padding: 11px 16px 11px 44px;
  border: none;
  background: transparent;
  cursor: pointer;
  font-family: inherit;
  font-size: 13px;
  color: var(--text-secondary);
  transition: background 0.15s, color 0.15s;
  text-align: left;
  gap: 8px;
}
.item-header:hover { background: var(--bg-tertiary); color: var(--text-primary); }

.item-title { flex: 1; text-align: left; }

.item-header-actions {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
}

.icon-action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  cursor: pointer;
  transition: color .15s, background .15s;
}
.icon-action-btn:hover { background: var(--bg-elevated); color: var(--text-primary); }
.icon-action-btn.active { color: var(--danger); }

/* ── Contenu ── */
.item-content {
  padding: 0 16px 16px 44px;
  font-size: 13px;
  color: var(--text-secondary);
  display: flex;
  flex-direction: column;
  gap: 10px;
  animation: fade-in-up 200ms ease forwards;
}

@keyframes fade-in-up {
  from { opacity: 0; transform: translateY(4px); }
  to   { opacity: 1; transform: translateY(0); }
}

.symptoms {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 8px 12px;
  background: rgba(234, 179, 8, 0.07);
  border-radius: var(--radius-sm);
  border-left: 3px solid var(--warning);
  font-size: 12px;
  color: var(--text-secondary);
}

.solution ol {
  margin: 6px 0 0 18px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  line-height: 1.5;
}

/* ── Bloc code ── */
.code-block {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--bg-primary);
}
.code-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: var(--bg-tertiary);
  border-bottom: 1px solid var(--border);
  font-size: 11px;
  color: var(--text-muted);
  font-weight: 500;
}
.copy-btn-sm {
  margin-left: auto;
  display: flex; align-items: center; gap: 4px;
  padding: 3px 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-secondary);
  color: var(--text-muted);
  cursor: pointer;
  font-family: inherit;
  font-size: 10px;
  transition: all 0.15s;
}
.copy-btn-sm:hover { border-color: var(--accent-primary); color: var(--accent-primary); }
.code-content {
  margin: 0;
  padding: 12px 14px;
  font-family: "JetBrains Mono", "Courier New", monospace;
  font-size: 11px;
  color: var(--text-secondary);
  white-space: pre;
  overflow-x: auto;
  line-height: 1.6;
  scrollbar-width: thin;
  scrollbar-color: var(--border) transparent;
  max-height: 320px;
  overflow-y: auto;
}

/* ── Commande rapide ── */
.cmd-output {
  margin: 6px 0 0;
  padding: 8px 12px;
  max-height: 240px;
  overflow: auto;
  white-space: pre-wrap;
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  background: var(--bg-primary);
  border: 1px solid var(--border);
  border-left: 3px solid var(--success, #22c55e);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
}
.cmd-output.failed { border-left-color: var(--danger, #ef4444); }
.command-block {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--bg-primary);
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  flex-wrap: wrap;
}
.command-block code {
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  color: var(--accent-primary);
  flex: 1;
  min-width: 0;
  word-break: break-all;
}
.copy-btn {
  display: flex; align-items: center; justify-content: center;
  padding: 4px 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-tertiary);
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.15s;
  flex-shrink: 0;
}
.copy-btn:hover, .copy-btn.copied { border-color: var(--accent-primary); color: var(--accent-primary); }

.run-btn {
  display: flex; align-items: center; gap: 4px;
  padding: 4px 10px;
  border: 1px solid var(--accent-primary);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--accent-primary);
  cursor: pointer;
  font-family: inherit;
  font-size: 11px;
  transition: all 0.15s;
  flex-shrink: 0;
}
.run-btn:hover { background: var(--accent-primary); color: #fff; }

/* ── Note ── */
.note-block {
  padding: 8px 12px;
  background: rgba(249, 115, 22, 0.07);
  border-left: 3px solid var(--accent-primary);
  border-radius: var(--radius-sm);
  font-size: 12px;
  color: var(--text-secondary);
}

/* ── Articles connexes ── */
.related-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 4px;
  border-top: 1px solid var(--border);
  margin-top: 4px;
}
.related-label {
  font-size: 11px;
  color: var(--text-muted);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.related-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.related-chip {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 10px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border);
  border-radius: 99px;
  font-size: 11px;
  color: var(--text-secondary);
  cursor: pointer;
  font-family: inherit;
  transition: border-color .15s, color .15s;
}
.related-chip:hover { border-color: var(--accent-primary); color: var(--accent-primary); }

/* ── État vide ── */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 48px;
  color: var(--text-muted);
  font-size: 14px;
  text-align: center;
}
.empty-hint { font-size: 12px; color: var(--text-muted); opacity: 0.7; }
</style>
