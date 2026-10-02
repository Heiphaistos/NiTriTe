<script setup lang="ts">
/**
 * En-tête du tableau de bord propre à chaque modèle d'interface. Il reçoit les
 * mêmes mesures que la page (aucune requête en plus) et n'ajoute que de la
 * navigation : le reste du tableau de bord est identique dans tous les modèles.
 * Le modèle Classique n'a pas d'en-tête (interface historique inchangée).
 */
import { computed } from "vue";
import { useRouter } from "vue-router";
import { useLocalStorage } from "@vueuse/core";
import { Cpu, MemoryStick, HardDrive, Wifi, Flag, Play, Star, History } from "lucide-vue-next";
import { useUiModelStore } from "@/stores/uiModel";
import { useMissionStore, toolLabel } from "@/stores/mission";
import { BUILTIN_MISSIONS } from "@/data/missions";
import { useShellNav } from "@/composables/useShellNav";

const props = defineProps<{
  cpu: number;
  ram: number;
  disk: number;
  /** Débit descendant en Ko/s. */
  netDown: number;
  health: number;
  healthLabel: string;
}>();

const ui = useUiModelStore();
const mission = useMissionStore();
const nav = useShellNav();
const router = useRouter();
const recent = useLocalStorage<string[]>("nitrite-recent", []);

const hour = new Date().getHours();
const greeting = hour < 6 ? "Bonne nuit" : hour < 12 ? "Bonjour" : hour < 18 ? "Bon après-midi" : "Bonsoir";
const mood = computed(() => props.health >= 80 ? "l'atelier tourne bien." : props.health >= 50 ? "quelques points à surveiller." : "le PC a besoin d'attention.");
const tone = computed(() => props.health >= 80 ? "ok" : props.health >= 50 ? "warn" : "bad");

function fmtNet(kbs: number) { return kbs >= 1024 ? `${(kbs / 1024).toFixed(1)} Mo/s` : `${Math.round(kbs)} Ko/s`; }
const metrics = computed(() => [
  { key: "cpu", label: "CPU", value: `${props.cpu}%`, pct: props.cpu, icon: Cpu },
  { key: "ram", label: "RAM", value: `${props.ram}%`, pct: props.ram, icon: MemoryStick },
  { key: "disk", label: "Disque", value: `${props.disk}%`, pct: props.disk, icon: HardDrive },
  { key: "net", label: "Réseau", value: fmtNet(props.netDown), pct: Math.min(100, props.netDown / 100), icon: Wifi },
]);
function level(pct: number) { return pct > 90 ? "bad" : pct > 75 ? "warn" : "ok"; }

// Favoris : épinglés, sinon une sélection d'atelier.
const DEFAULT_FAVS = ["/diagnostic", "/cleaner", "/drivers", "/updates", "/master-install"];
const favorites = computed(() => nav.pinnedItems.value.length
  ? nav.pinnedItems.value.slice(0, 6)
  : nav.allItems.filter(i => DEFAULT_FAVS.includes(i.route)));
const recentItems = computed(() => recent.value
  .map(r => nav.allItems.find(i => i.route === r)).filter(Boolean).slice(0, 6) as typeof nav.allItems);

// Anneau de santé (r = 52 → périmètre ≈ 326,7).
const ring = computed(() => `${(props.health / 100) * 326.7} 326.7`);
const asciiBar = (pct: number) => "#".repeat(Math.round(pct / 10)).padEnd(10, "·");

function startMission(id: string) {
  const first = mission.start(id);
  if (first) router.push(first.route);
}
</script>

<template>
  <!-- ── 01 Forge : salutation, anneau de santé, favoris ── -->
  <section v-if="ui.model === 'forge'" class="mdh mdh-forge" aria-label="Résumé">
    <div class="mdh-forge__health">
      <svg viewBox="0 0 120 120" class="mdh-ring" aria-hidden="true">
        <circle cx="60" cy="60" r="52" class="mdh-ring__track" />
        <circle cx="60" cy="60" r="52" class="mdh-ring__fill" :class="`is-${tone}`" :stroke-dasharray="ring" transform="rotate(-90 60 60)" />
      </svg>
      <div class="mdh-ring__label"><strong>{{ health }}</strong><span>santé</span></div>
    </div>
    <div class="mdh-forge__text">
      <span class="mdh-forge__hello">{{ greeting }}, {{ mood }}</span>
      <span class="mdh-muted">{{ healthLabel }} · CPU {{ cpu }}% · RAM {{ ram }}% · Disque {{ disk }}%</span>
      <div class="mdh-chips">
        <button v-for="f in favorites" :key="f.id" class="mdh-chip" @click="router.push(f.route)">
          <component :is="nav.getNavIcon(f.icon)" :size="13" /> {{ f.label }}
        </button>
      </div>
    </div>
  </section>

  <!-- ── 02 Command Deck : sections au clavier ── -->
  <section v-else-if="ui.model === 'command-deck'" class="mdh mdh-deck" aria-label="Commandes rapides">
    <div class="mdh-deck__head">
      <span class="mdh-kicker">Commandes rapides</span>
      <button class="mdh-deck__cmd" @click="nav.openSearch">Tapez une commande… <kbd class="sh-kbd">Ctrl K</kbd></button>
      <span class="mdh-badge" :class="`is-${tone}`">Santé {{ health }}/100</span>
    </div>
    <div class="mdh-deck__grid">
      <button v-for="(s, i) in nav.sections" :key="s.title" class="mdh-deck__key" @click="router.push(s.items[0].route)">
        <kbd class="sh-kbd">Ctrl {{ i === 9 ? 0 : i + 1 }}</kbd>
        <component :is="nav.getSectionIcon(s.title)" :size="15" />
        <span>{{ nav.sectionShortLabel(s.title) }}</span>
      </button>
    </div>
  </section>

  <!-- ── 04 Bento : tuiles ── -->
  <section v-else-if="ui.model === 'bento'" class="mdh mdh-bento" aria-label="Résumé">
    <div class="mdh-bento__tile mdh-bento__health" :class="`is-${tone}`">
      <span class="mdh-kicker">Santé du PC</span>
      <strong>{{ health }}</strong>
      <span>{{ healthLabel }}</span>
    </div>
    <div v-for="m in metrics" :key="m.key" class="mdh-bento__tile">
      <span class="mdh-bento__label"><component :is="m.icon" :size="14" /> {{ m.label }}</span>
      <strong>{{ m.value }}</strong>
      <span class="mdh-meter"><span :class="`is-${level(m.pct)}`" :style="{ width: `${m.pct}%` }" /></span>
    </div>
    <button
      v-for="s in nav.sections.slice(1, 5)"
      :key="s.title"
      class="mdh-bento__tile mdh-bento__sec"
      @click="router.push(s.items[0].route)"
    >
      <component :is="nav.getSectionIcon(s.title)" :size="18" />
      <span>{{ nav.sectionShortLabel(s.title) }}</span>
      <small>{{ s.items.length }} outils</small>
    </button>
  </section>

  <!-- ── 05 Console : état en texte ── -->
  <section v-else-if="ui.model === 'console'" class="mdh mdh-console" aria-label="État du système">
    <div><span class="c-prompt">nitrite@pc:~$</span> status --live</div>
    <div v-for="m in metrics.slice(0, 3)" :key="m.key">
      <span class="c-key">{{ m.label.toLowerCase().padEnd(7, " ") }}</span>
      <span :class="`c-${level(m.pct)}`">[{{ asciiBar(m.pct) }}]</span> {{ m.value }}
    </div>
    <div><span class="c-key">{{ "net".padEnd(7, " ") }}</span> ↓ {{ metrics[3].value }}</div>
    <div><span class="c-key">{{ "santé".padEnd(7, " ") }}</span> <span :class="`c-${tone}`">{{ health }}/100 {{ healthLabel.toUpperCase() }}</span></div>
    <div class="c-hint"># Ctrl+1…0 : sections · Ctrl+K : rechercher · ? : raccourcis</div>
  </section>

  <!-- ── 06 Orbital : santé au centre, mesures en orbite ── -->
  <section v-else-if="ui.model === 'orbital'" class="mdh mdh-orbit" aria-label="Santé du système">
    <div class="mdh-orbit__stage">
      <svg viewBox="0 0 120 120" class="mdh-ring mdh-orbit__ring" aria-hidden="true">
        <circle cx="60" cy="60" r="52" class="mdh-ring__track" />
        <circle cx="60" cy="60" r="52" class="mdh-ring__fill" :class="`is-${tone}`" :stroke-dasharray="ring" transform="rotate(-90 60 60)" />
      </svg>
      <div class="mdh-ring__label mdh-orbit__label"><strong>{{ health }}</strong><span>{{ healthLabel }}</span></div>
      <div v-for="(m, i) in metrics" :key="m.key" class="mdh-orbit__sat" :class="`sat-${i}`">
        <component :is="m.icon" :size="14" />
        <strong>{{ m.value }}</strong>
        <span>{{ m.label }}</span>
      </div>
    </div>
  </section>

  <!-- ── 08 Colonnes : favoris et récents ── -->
  <section v-else-if="ui.model === 'columns'" class="mdh mdh-cols" aria-label="Accès rapides">
    <div class="mdh-cols__col">
      <span class="mdh-kicker"><Star :size="12" /> Favoris</span>
      <button v-for="f in favorites" :key="f.id" class="mdh-cols__row" @click="router.push(f.route)">
        <component :is="nav.getNavIcon(f.icon)" :size="14" /> {{ f.label }}
        <small>{{ nav.navDescription(f.id) }}</small>
      </button>
    </div>
    <div class="mdh-cols__col">
      <span class="mdh-kicker"><History :size="12" /> Récemment ouverts</span>
      <p v-if="!recentItems.length" class="mdh-muted">Les outils ouverts apparaîtront ici.</p>
      <button v-for="r in recentItems" :key="r.id" class="mdh-cols__row" @click="router.push(r.route)">
        <component :is="nav.getNavIcon(r.icon)" :size="14" /> {{ r.label }}
        <small>{{ nav.navDescription(r.id) }}</small>
      </button>
    </div>
    <div class="mdh-cols__col mdh-cols__state">
      <span class="mdh-kicker">État</span>
      <div v-for="m in metrics" :key="m.key" class="mdh-cols__metric">
        <span>{{ m.label }}</span><strong>{{ m.value }}</strong>
      </div>
      <div class="mdh-cols__metric"><span>Santé</span><strong :class="`t-${tone}`">{{ health }}/100</strong></div>
    </div>
  </section>

  <!-- ── 09 Verre & dock : widgets ── -->
  <section v-else-if="ui.model === 'glass-dock'" class="mdh mdh-glass" aria-label="Widgets">
    <div class="mdh-glass__w mdh-glass__hello">
      <span class="mdh-muted">{{ greeting }}</span>
      <strong>{{ healthLabel }}</strong>
      <span class="mdh-muted">Santé {{ health }}/100</span>
    </div>
    <div v-for="m in metrics" :key="m.key" class="mdh-glass__w">
      <svg viewBox="0 0 120 120" class="mdh-ring mdh-glass__ring" aria-hidden="true">
        <circle cx="60" cy="60" r="52" class="mdh-ring__track" />
        <circle cx="60" cy="60" r="52" class="mdh-ring__fill" :class="`is-${level(m.pct)}`" :stroke-dasharray="`${(m.pct / 100) * 326.7} 326.7`" transform="rotate(-90 60 60)" />
      </svg>
      <div class="mdh-glass__val"><strong>{{ m.value }}</strong><span>{{ m.label }}</span></div>
    </div>
  </section>

  <!-- ── 10 Mission : reprendre ou démarrer une mission ── -->
  <section v-else-if="ui.model === 'mission'" class="mdh mdh-mission" aria-label="Missions">
    <template v-if="mission.active">
      <span class="mdh-kicker"><Flag :size="12" /> Mission en cours</span>
      <strong class="mdh-mission__name">{{ mission.active.name }}<template v-if="mission.active.client"> · {{ mission.active.client }}</template></strong>
      <span class="mdh-meter mdh-mission__meter"><span class="is-ok" :style="{ width: `${mission.progress}%` }" /></span>
      <span class="mdh-muted">{{ mission.doneCount }}/{{ mission.active.steps.length }} étapes<template v-if="mission.currentStep && !mission.isFinished"> · suivante : {{ toolLabel(mission.currentStep.route) }}</template></span>
      <button class="mdh-btn" @click="router.push(mission.isFinished || !mission.currentStep ? '/missions' : mission.currentStep.route)">
        <Play :size="13" /> {{ mission.isFinished ? "Voir le rapport" : "Reprendre" }}
      </button>
    </template>
    <template v-else>
      <span class="mdh-kicker"><Flag :size="12" /> Démarrer une mission</span>
      <div class="mdh-mission__list">
        <button v-for="t in BUILTIN_MISSIONS.slice(0, 4)" :key="t.id" class="mdh-mission__tpl" @click="startMission(t.id)">
          <strong>{{ t.name }}</strong>
          <small>{{ t.steps.length }} étapes · {{ t.steps.map(s => toolLabel(s.route)).slice(0, 3).join(" → ") }}…</small>
        </button>
        <button class="mdh-mission__tpl mdh-mission__more" @click="router.push('/missions')">Toutes les missions →</button>
      </div>
    </template>
  </section>
</template>

<style scoped>
.mdh { margin-bottom: 18px; }
.mdh-muted { color: var(--text-muted); font-size: 12px; }
.mdh-kicker { display: inline-flex; align-items: center; gap: 6px; font-size: 10.5px; font-weight: 800; letter-spacing: 0.12em; text-transform: uppercase; color: var(--accent-primary); }
.is-ok { --tone: var(--success); } .is-warn { --tone: var(--warning); } .is-bad { --tone: var(--danger); }
.t-ok { color: var(--success); } .t-warn { color: var(--warning); } .t-bad { color: var(--danger); }
.mdh-meter { display: block; height: 5px; border-radius: 99px; background: var(--bg-tertiary); overflow: hidden; }
.mdh-meter span { display: block; height: 100%; background: var(--tone, var(--accent-primary)); transition: width var(--transition-normal); }
.mdh-ring { width: 100%; height: 100%; }
.mdh-ring__track { fill: none; stroke: var(--bg-tertiary); stroke-width: 9; }
.mdh-ring__fill { fill: none; stroke: var(--tone, var(--accent-primary)); stroke-width: 9; stroke-linecap: round; transition: stroke-dasharray var(--transition-slow); }
.mdh-ring__label { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; }
.mdh-ring__label strong { font-size: 30px; font-weight: 800; line-height: 1; }
.mdh-ring__label span { font-size: 10.5px; color: var(--text-muted); }
.mdh-btn {
  display: inline-flex; align-items: center; gap: 6px; height: 32px; padding: 0 14px; border-radius: var(--radius-md);
  border: none; background: var(--accent-primary); color: var(--bg-primary); font-family: inherit; font-weight: 700; cursor: pointer;
}

/* Forge */
.mdh-forge {
  display: flex; align-items: center; gap: 22px; padding: 18px 22px; border-radius: var(--radius-xl);
  border: 1px solid color-mix(in srgb, var(--accent-primary) 22%, var(--border));
  background: linear-gradient(160deg, color-mix(in srgb, var(--accent-primary) 9%, var(--bg-secondary)), var(--bg-secondary) 60%);
}
.mdh-forge__health { position: relative; width: 104px; height: 104px; flex-shrink: 0; }
.mdh-forge__text { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.mdh-forge__hello { font-size: 18px; font-weight: 800; }
.mdh-chips { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 6px; }
.mdh-chip {
  display: inline-flex; align-items: center; gap: 6px; padding: 5px 11px; border-radius: 999px; cursor: pointer;
  border: 1px solid var(--border); background: var(--bg-primary); color: var(--text-secondary); font-family: inherit; font-size: 12px;
}
.mdh-chip:hover { color: var(--accent-hover); border-color: var(--accent-primary); }
.mdh-chip :deep(svg) { color: var(--accent-primary); }

/* Command Deck */
.mdh-deck { padding: 14px; border: 1px solid var(--border); border-radius: var(--radius-lg); background: var(--bg-secondary); }
.mdh-deck__head { display: flex; align-items: center; gap: 14px; margin-bottom: 12px; }
.mdh-deck__cmd {
  flex: 1; max-width: 420px; display: flex; align-items: center; justify-content: space-between; height: 32px; padding: 0 10px;
  border-radius: var(--radius-md); border: 1px solid var(--border); background: var(--bg-primary); color: var(--text-muted);
  font-family: inherit; font-size: 12.5px; cursor: pointer;
}
.mdh-deck__cmd:hover { border-color: var(--accent-primary); }
.mdh-badge { margin-left: auto; padding: 4px 10px; border-radius: 999px; font-size: 11.5px; font-weight: 700; color: var(--tone); background: color-mix(in srgb, var(--tone) 12%, transparent); }
.mdh-deck__grid { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 6px; }
.mdh-deck__key {
  display: flex; align-items: center; gap: 8px; padding: 9px 10px; border-radius: var(--radius-md); cursor: pointer;
  border: 1px solid var(--border); background: var(--bg-primary); color: var(--text-primary); font-family: inherit; font-size: 12.5px; font-weight: 600;
}
.mdh-deck__key:hover { border-color: var(--accent-primary); }
.mdh-deck__key :deep(svg) { color: var(--accent-primary); flex-shrink: 0; }
.mdh-deck__key span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

/* Bento */
.mdh-bento { display: grid; grid-template-columns: 1.4fr repeat(4, minmax(0, 1fr)); grid-auto-rows: minmax(92px, auto); gap: 12px; }
.mdh-bento__tile {
  display: flex; flex-direction: column; justify-content: space-between; gap: 6px; padding: 16px;
  border-radius: 22px; background: var(--bg-tertiary); border: none; color: var(--text-primary); font-family: inherit; text-align: left;
}
.mdh-bento__tile strong { font-size: 26px; font-weight: 800; letter-spacing: -0.02em; }
.mdh-bento__health { grid-row: span 2; background: linear-gradient(160deg, color-mix(in srgb, var(--tone) 22%, var(--bg-tertiary)), var(--bg-tertiary)); }
.mdh-bento__health strong { font-size: 54px; color: var(--tone); }
.mdh-bento__label { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-secondary); }
.mdh-bento__sec { cursor: pointer; justify-content: flex-start; }
.mdh-bento__sec :deep(svg) { color: var(--accent-primary); }
.mdh-bento__sec span { font-weight: 700; font-size: 13.5px; }
.mdh-bento__sec small { color: var(--text-muted); font-size: 11px; }
.mdh-bento__sec:hover { outline: 1px solid var(--accent-primary); }

/* Console */
.mdh-console {
  font-family: "JetBrains Mono", Consolas, monospace; font-size: 12.5px; line-height: 1.75; white-space: pre;
  padding: 14px 16px; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--bg-primary); overflow-x: auto;
}
.c-prompt { color: var(--success); }
.c-key { color: var(--info); }
.c-ok { color: var(--success); } .c-warn { color: var(--warning); } .c-bad { color: var(--danger); }
.c-hint { color: var(--text-muted); }

/* Orbital */
.mdh-orbit { display: flex; justify-content: center; }
.mdh-orbit__stage { position: relative; width: 420px; height: 220px; }
.mdh-orbit__ring { position: absolute; left: 50%; top: 50%; width: 170px; height: 170px; transform: translate(-50%, -50%); }
.mdh-orbit__label { inset: auto; left: 50%; top: 50%; transform: translate(-50%, -50%); }
.mdh-orbit__label strong { font-size: 40px; }
.mdh-orbit__sat {
  position: absolute; width: 92px; height: 92px; border-radius: 50%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 1px;
  border: 1px solid color-mix(in srgb, var(--accent-primary) 30%, var(--border)); background: var(--bg-secondary);
}
.mdh-orbit__sat :deep(svg) { color: var(--accent-primary); }
.mdh-orbit__sat strong { font-size: 14px; }
.mdh-orbit__sat span { font-size: 10px; color: var(--text-muted); }
.sat-0 { left: 0; top: 0; } .sat-1 { right: 0; top: 0; } .sat-2 { left: 0; bottom: 0; } .sat-3 { right: 0; bottom: 0; }

/* Colonnes */
.mdh-cols { display: grid; grid-template-columns: 1fr 1fr 220px; border: 1px solid var(--border); border-radius: var(--radius-lg); background: var(--bg-secondary); }
.mdh-cols__col { display: flex; flex-direction: column; gap: 2px; padding: 12px; min-width: 0; }
.mdh-cols__col + .mdh-cols__col { border-left: 1px solid var(--border); }
.mdh-cols__col .mdh-kicker { margin-bottom: 6px; }
.mdh-cols__row {
  display: grid; grid-template-columns: 18px 1fr; align-items: center; column-gap: 8px; padding: 6px 8px; border-radius: var(--radius-sm);
  border: none; background: transparent; color: var(--text-primary); font-family: inherit; font-size: 12.5px; font-weight: 600; text-align: left; cursor: pointer;
}
.mdh-cols__row :deep(svg) { color: var(--accent-primary); }
.mdh-cols__row small { grid-column: 2; font-size: 11px; font-weight: 400; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.mdh-cols__row:hover { background: var(--bg-tertiary); }
.mdh-cols__metric { display: flex; justify-content: space-between; padding: 5px 4px; font-size: 12.5px; border-bottom: 1px dashed var(--border); }
.mdh-cols__metric span { color: var(--text-muted); }

/* Verre & dock */
.mdh-glass { display: grid; grid-template-columns: 1.3fr repeat(4, minmax(0, 1fr)); gap: 12px; }
.mdh-glass__w {
  position: relative; display: flex; align-items: center; justify-content: center; min-height: 120px; padding: 12px; border-radius: 22px;
  background: color-mix(in srgb, var(--bg-tertiary) 45%, transparent); border: 1px solid color-mix(in srgb, var(--text-primary) 9%, transparent);
  backdrop-filter: blur(14px);
}
html[data-perf="light"] .mdh-glass__w { backdrop-filter: none; background: var(--bg-tertiary); }
.mdh-glass__hello { flex-direction: column; align-items: flex-start; gap: 4px; }
.mdh-glass__hello strong { font-size: 24px; font-weight: 800; }
.mdh-glass__ring { position: absolute; width: 96px; height: 96px; }
.mdh-glass__val { position: relative; display: flex; flex-direction: column; align-items: center; }
.mdh-glass__val strong { font-size: 15px; }
.mdh-glass__val span { font-size: 10.5px; color: var(--text-muted); }

/* Mission */
.mdh-mission {
  display: flex; align-items: center; gap: 14px; flex-wrap: wrap; padding: 14px 16px; border-radius: var(--radius-lg);
  border: 1px solid var(--border); border-top: 2px solid var(--accent-primary); background: var(--bg-secondary);
}
.mdh-mission__name { font-size: 15px; }
.mdh-mission__meter { width: 160px; }
.mdh-mission__list { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 8px; width: 100%; }
.mdh-mission__tpl {
  display: flex; flex-direction: column; gap: 4px; padding: 10px 12px; border-radius: var(--radius-md); cursor: pointer; text-align: left;
  border: 1px solid var(--border); background: var(--bg-primary); color: var(--text-primary); font-family: inherit;
}
.mdh-mission__tpl:hover { border-color: var(--accent-primary); }
.mdh-mission__tpl small { color: var(--text-muted); font-size: 11px; line-height: 1.35; }
.mdh-mission__more { align-items: center; justify-content: center; color: var(--accent-primary); font-weight: 700; font-size: 12.5px; }

@media (max-width: 1200px) {
  .mdh-bento, .mdh-glass { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  .mdh-deck__grid, .mdh-mission__list { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  .mdh-cols { grid-template-columns: 1fr 1fr; }
  .mdh-cols__state { display: none; }
}
</style>
