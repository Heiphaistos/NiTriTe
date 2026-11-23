<script setup lang="ts">
import { ref, computed } from "vue";
import { invoke } from "@/utils/invoke";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NModal from "@/components/ui/NModal.vue";
import NInput from "@/components/ui/NInput.vue";
import NBadge from "@/components/ui/NBadge.vue";
import { useNotificationStore } from "@/stores/notifications";
import { Plus, Pencil, Trash2, Copy, Play, Search, Check } from "lucide-vue-next";

const notify = useNotificationStore();
const STORAGE_KEY = "nitrite-snippets-v2";

interface Snippet {
  id: string; title: string; lang: "powershell" | "cmd" | "python" | "js" | "sql" | "bash" | "other";
  content: string; description: string; tags: string;
}

const LANGS = ["powershell","cmd","python","js","sql","bash","other"] as const;
const LANG_COLORS: Record<string, string> = { powershell:"#2563eb",cmd:"#6b7280",python:"#eab308",js:"#f59e0b",sql:"#06b6d4",bash:"#22c55e",other:"#6b7280" };

function load(): Snippet[] {
  try { return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]"); }
  catch { return []; }
}
function save(list: Snippet[]) { localStorage.setItem(STORAGE_KEY, JSON.stringify(list)); }

const snippets = ref<Snippet[]>(load());
const search = ref("");
const filterLang = ref<string>("all");
const showModal = ref(false);
const editTarget = ref<Snippet | null>(null);
const copied = ref<string | null>(null);
const runOutput = ref<Record<string, string>>({});

const form = ref({ title: "", lang: "powershell" as Snippet["lang"], content: "", description: "", tags: "" });

const filtered = computed(() => {
  let list = snippets.value;
  if (filterLang.value !== "all") list = list.filter(s => s.lang === filterLang.value);
  if (search.value) {
    const q = search.value.toLowerCase();
    list = list.filter(s => s.title.toLowerCase().includes(q) || s.tags.toLowerCase().includes(q) || s.description.toLowerCase().includes(q));
  }
  return list;
});

function openNew() {
  editTarget.value = null;
  form.value = { title: "", lang: "powershell", content: "", description: "", tags: "" };
  showModal.value = true;
}

function openEdit(s: Snippet) {
  editTarget.value = s;
  form.value = { title: s.title, lang: s.lang, content: s.content, description: s.description, tags: s.tags };
  showModal.value = true;
}

function saveSnippet() {
  if (!form.value.title.trim() || !form.value.content.trim()) { notify.warning("Titre et contenu requis"); return; }
  if (editTarget.value) {
    const idx = snippets.value.findIndex(s => s.id === editTarget.value!.id);
    if (idx >= 0) snippets.value[idx] = { ...editTarget.value, ...form.value };
  } else {
    snippets.value.push({ id: Date.now().toString(), ...form.value });
  }
  save(snippets.value);
  showModal.value = false;
  notify.success(editTarget.value ? "Snippet mis à jour" : "Snippet ajouté");
}

function deleteSnippet(s: Snippet) {
  snippets.value = snippets.value.filter(x => x.id !== s.id);
  save(snippets.value);
  notify.info("Snippet supprimé");
}

async function copySnippet(s: Snippet) {
  await navigator.clipboard.writeText(s.content).catch(() => {});
  copied.value = s.id;
  setTimeout(() => { copied.value = null; }, 1500);
}

async function runSnippet(s: Snippet) {
  if (s.lang !== "powershell" && s.lang !== "cmd") { notify.warning("Seuls PowerShell et CMD peuvent être exécutés"); return; }
  runOutput.value[s.id] = "⏳ Exécution...";
  try {
    const res = await invoke<{ output: string; success: boolean }>("execute_script", {
      content: s.content, scriptType: s.lang,
    }, 310_000); // 300 s cote Rust (executor.rs), pas les 30 s par defaut
    runOutput.value[s.id] = res.output || "(pas de sortie)";
    // Ne pas afficher « terminé » (succès) si le script a échoué (exit non-zéro).
    if (res.success) notify.success("Script terminé");
    else notify.warning("Script terminé avec erreurs");
  } catch (e: unknown) { runOutput.value[s.id] = String(e); notify.error("Erreur", String(e)); }
}
</script>

<template>
  <div style="display:flex;flex-direction:column;gap:12px">
    <!-- Toolbar -->
    <div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap">
      <div style="display:flex;align-items:center;gap:6px;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius-md);padding:4px 10px;flex:1;min-width:160px">
        <Search :size="13" style="color:var(--text-muted)" />
        <input v-model="search" placeholder="Rechercher..." style="background:none;border:none;outline:none;font-size:12px;color:var(--text-primary);width:100%" />
      </div>
      <select v-model="filterLang" style="padding:5px 8px;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius-md);color:var(--text-primary);font-size:12px">
        <option value="all">Tous les langages</option>
        <option v-for="l in LANGS" :key="l" :value="l">{{ l }}</option>
      </select>
      <NButton variant="primary" size="sm" @click="openNew"><Plus :size="14" /> Nouveau</NButton>
    </div>

    <!-- Empty -->
    <div v-if="!filtered.length" style="text-align:center;padding:30px;color:var(--text-muted);font-size:13px">
      {{ snippets.length ? 'Aucun snippet correspond à la recherche.' : 'Aucun snippet. Cliquez "Nouveau" pour en créer un.' }}
    </div>

    <!-- Liste -->
    <NCard v-for="s in filtered" :key="s.id">
      <div style="display:flex;align-items:flex-start;gap:10px">
        <div style="flex:1;min-width:0">
          <div style="display:flex;align-items:center;gap:8px;margin-bottom:4px;flex-wrap:wrap">
            <span style="font-size:13px;font-weight:700;color:var(--text-primary)">{{ s.title }}</span>
            <span style="font-size:10px;font-weight:700;padding:2px 7px;border-radius:4px;color:#fff" :style="{ background: LANG_COLORS[s.lang] }">{{ s.lang }}</span>
            <span v-if="s.tags" style="font-size:11px;color:var(--text-muted)">{{ s.tags }}</span>
          </div>
          <p v-if="s.description" style="font-size:12px;color:var(--text-muted);margin-bottom:6px">{{ s.description }}</p>
          <pre style="font-size:11px;font-family:'JetBrains Mono',monospace;background:var(--bg-tertiary);border:1px solid var(--border);border-radius:6px;padding:8px;overflow-x:auto;max-height:100px;white-space:pre-wrap;word-break:break-all;color:var(--text-primary)">{{ s.content }}</pre>
          <pre v-if="runOutput[s.id]" style="font-size:11px;font-family:'JetBrains Mono',monospace;background:var(--bg-tertiary);border:1px solid var(--border);border-radius:6px;padding:8px;overflow-x:auto;max-height:120px;margin-top:4px;color:var(--success)">{{ runOutput[s.id] }}</pre>
        </div>
        <div style="display:flex;flex-direction:column;gap:4px;flex-shrink:0">
          <NButton variant="ghost" size="sm" @click="copySnippet(s)" :title="'Copier'">
            <component :is="copied === s.id ? Check : Copy" :size="12" />
          </NButton>
          <NButton v-if="s.lang==='powershell'||s.lang==='cmd'" variant="ghost" size="sm" @click="runSnippet(s)" title="Exécuter">
            <Play :size="12" />
          </NButton>
          <NButton variant="ghost" size="sm" @click="openEdit(s)" title="Éditer"><Pencil :size="12" /></NButton>
          <NButton variant="danger" size="sm" @click="deleteSnippet(s)" title="Supprimer"><Trash2 :size="12" /></NButton>
        </div>
      </div>
    </NCard>

    <!-- Modal édition -->
    <NModal :open="showModal" :title="editTarget ? 'Modifier le snippet' : 'Nouveau snippet'" width="560px" @close="showModal = false">
      <div style="display:flex;flex-direction:column;gap:12px;padding:16px">
        <div style="display:flex;gap:10px">
          <div style="flex:1">
            <label style="font-size:12px;color:var(--text-muted);display:block;margin-bottom:4px">Titre *</label>
            <NInput v-model="form.title" placeholder="Nom du snippet" />
          </div>
          <div>
            <label style="font-size:12px;color:var(--text-muted);display:block;margin-bottom:4px">Langage</label>
            <select v-model="form.lang" style="padding:7px 10px;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius-md);color:var(--text-primary);font-size:12px">
              <option v-for="l in LANGS" :key="l" :value="l">{{ l }}</option>
            </select>
          </div>
        </div>
        <div>
          <label style="font-size:12px;color:var(--text-muted);display:block;margin-bottom:4px">Description</label>
          <NInput v-model="form.description" placeholder="Description courte (optionnel)" />
        </div>
        <div>
          <label style="font-size:12px;color:var(--text-muted);display:block;margin-bottom:4px">Contenu *</label>
          <textarea v-model="form.content" rows="6"
            style="width:100%;background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-md);padding:8px 12px;font-size:12px;font-family:'JetBrains Mono',monospace;color:var(--text-primary);resize:vertical;outline:none;box-sizing:border-box"
            placeholder="Contenu du snippet..." />
        </div>
        <div>
          <label style="font-size:12px;color:var(--text-muted);display:block;margin-bottom:4px">Tags (séparés par virgule)</label>
          <NInput v-model="form.tags" placeholder="réseau, maintenance, ..." />
        </div>
        <div style="display:flex;gap:8px;justify-content:flex-end">
          <NButton variant="ghost" @click="showModal = false">Annuler</NButton>
          <NButton variant="primary" @click="saveSnippet"><Check :size="13" /> Sauvegarder</NButton>
        </div>
      </div>
    </NModal>
  </div>
</template>
