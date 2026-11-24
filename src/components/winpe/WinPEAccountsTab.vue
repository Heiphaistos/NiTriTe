<script setup lang="ts">
import { ref, watch } from "vue";
import { invoke } from "@/utils/invoke";
import { confirm } from "@tauri-apps/plugin-dialog";
import NButton from "@/components/ui/NButton.vue";
import { Users, ShieldCheck, ShieldOff, KeyRound, Lock, Unlock, Trash2, Power, UserPlus } from "lucide-vue-next";

interface RepairResult { success: boolean; output: string; command: string; }
interface SamUser {
  username: string; rid: number; enabled: boolean; locked: boolean;
  is_admin: boolean; password_blank: boolean; is_builtin: boolean;
}
interface SamActionResult { success: boolean; message: string; backup_path: string; }

const props = defineProps<{ windowsDir: string }>();
const emit = defineEmits<{ (e: "result", r: RepairResult): void }>();

const dir = ref(props.windowsDir || "C:\\Windows");
watch(() => props.windowsDir, (v) => { dir.value = v || "C:\\Windows"; });

const users = ref<SamUser[]>([]);
const loadingList = ref(false);
const busyRid = ref<number | null>(null);

async function loadUsers() {
  loadingList.value = true;
  try {
    users.value = await invoke<SamUser[]>("sam_list_users", { windowsDir: dir.value });
    if (!users.value.length) {
      emit("result", { success: false, output: "Aucun compte trouvé (ruche non chargée ou installation invalide).", command: "sam_list_users" });
    }
  } catch (e) {
    emit("result", { success: false, output: String(e), command: "sam_list_users" });
  } finally {
    loadingList.value = false;
  }
}

/** Mappe un SamActionResult vers le journal partagé, en y joignant le chemin de sauvegarde. */
function report(cmd: string, r: SamActionResult) {
  const bk = r.backup_path ? `\nSauvegarde SAM : ${r.backup_path}` : "";
  emit("result", { success: r.success, output: r.message + bk, command: cmd });
}

async function act(u: SamUser, cmd: string, question: string, call: () => Promise<SamActionResult>) {
  if (busyRid.value !== null) return;
  const ok = await confirm(`${question}\n\nCompte « ${u.username} » (RID ${u.rid}) — installation ${dir.value}.`, {
    title: "Comptes & mots de passe hors ligne", kind: "warning",
  });
  if (!ok) return;
  busyRid.value = u.rid;
  try {
    report(cmd, await call());
  } catch (e) {
    emit("result", { success: false, output: String(e), command: cmd });
  } finally {
    busyRid.value = null;
    await loadUsers();
  }
}

const clearPwd = (u: SamUser) =>
  act(u, "sam_clear_password", "Faire SAUTER (blanchir) le mot de passe ? Le compte se connectera sans mot de passe.",
    () => invoke<SamActionResult>("sam_clear_password", { windowsDir: dir.value, rid: u.rid }));

const toggleEnabled = (u: SamUser) =>
  act(u, "sam_set_enabled", u.enabled ? "Désactiver ce compte ?" : "Activer ce compte ?",
    () => invoke<SamActionResult>("sam_set_enabled", { windowsDir: dir.value, rid: u.rid, enabled: !u.enabled }));

const unlock = (u: SamUser) =>
  act(u, "sam_unlock", "Déverrouiller ce compte (effacer verrouillage + désactivation) ?",
    () => invoke<SamActionResult>("sam_unlock", { windowsDir: dir.value, rid: u.rid }));

const toggleAdmin = (u: SamUser) =>
  act(u, "sam_set_admin", u.is_admin ? "Retirer ce compte des administrateurs ?" : "Promouvoir ce compte administrateur ?",
    () => invoke<SamActionResult>("sam_set_admin", { windowsDir: dir.value, rid: u.rid, admin: !u.is_admin }));

const removeUser = (u: SamUser) =>
  act(u, "sam_delete_user", "SUPPRIMER définitivement ce compte de la SAM ? (irréversible hors restauration de la sauvegarde)",
    () => invoke<SamActionResult>("sam_delete_user", { windowsDir: dir.value, rid: u.rid }));

async function tryCreate() {
  // Création volontairement non disponible (blocage structurel reg.exe) : on affiche la raison.
  try {
    report("sam_create_user", await invoke<SamActionResult>("sam_create_user", { windowsDir: dir.value, username: "" }));
  } catch (e) {
    emit("result", { success: false, output: String(e), command: "sam_create_user" });
  }
}
</script>

<template>
  <div class="accounts-tab">
    <div class="intro">
      <div class="tool-header"><Users :size="18" /><h3>Comptes &amp; mots de passe (hors ligne)</h3></div>
      <p class="tool-desc">
        Édition directe de la ruche SAM d'une installation Windows <strong>hors ligne</strong>
        (jamais le Windows en cours). Blanchir un mot de passe, (dés)activer, déverrouiller,
        promouvoir administrateur ou supprimer un compte. Une sauvegarde horodatée de la SAM
        est créée avant chaque écriture.
      </p>
      <div class="form-group">
        <label class="form-label">Répertoire Windows cible :</label>
        <input v-model="dir" class="form-input" placeholder="Ex : C:\Windows" />
      </div>
      <div class="tool-actions">
        <NButton size="sm" variant="primary" :loading="loadingList" :disabled="!dir" @click="loadUsers">
          Charger les comptes
        </NButton>
        <NButton size="sm" variant="ghost" @click="tryCreate">
          <UserPlus :size="14" /> Créer un compte
        </NButton>
      </div>
    </div>

    <div v-if="users.length" class="user-list">
      <div v-for="u in users" :key="u.rid" class="user-card" :class="{ disabled: !u.enabled }">
        <div class="user-main">
          <div class="user-name">
            {{ u.username }}
            <span class="user-rid">RID {{ u.rid }}</span>
          </div>
          <div class="badges">
            <span class="badge" :class="u.enabled ? 'ok' : 'off'">{{ u.enabled ? 'Activé' : 'Désactivé' }}</span>
            <span v-if="u.is_admin" class="badge admin">Administrateur</span>
            <span v-if="u.locked" class="badge warn">Verrouillé</span>
            <span v-if="u.password_blank" class="badge warn">Sans mot de passe</span>
            <span v-if="u.is_builtin" class="badge builtin">Intégré</span>
          </div>
        </div>
        <div class="user-actions">
          <NButton size="sm" variant="secondary" :loading="busyRid === u.rid" title="Blanchir le mot de passe" @click="clearPwd(u)">
            <KeyRound :size="14" /> MDP
          </NButton>
          <NButton size="sm" variant="ghost" :loading="busyRid === u.rid" :title="u.enabled ? 'Désactiver' : 'Activer'" @click="toggleEnabled(u)">
            <Power :size="14" /> {{ u.enabled ? 'Désactiver' : 'Activer' }}
          </NButton>
          <NButton size="sm" variant="ghost" :loading="busyRid === u.rid" title="Déverrouiller" @click="unlock(u)">
            <Unlock :size="14" /> Déverr.
          </NButton>
          <NButton size="sm" variant="ghost" :loading="busyRid === u.rid" :title="u.is_admin ? 'Rétrograder' : 'Promouvoir admin'" @click="toggleAdmin(u)">
            <component :is="u.is_admin ? ShieldOff : ShieldCheck" :size="14" /> {{ u.is_admin ? 'Rétrograder' : 'Admin' }}
          </NButton>
          <NButton size="sm" variant="danger" :loading="busyRid === u.rid" :disabled="u.is_builtin" title="Supprimer le compte" @click="removeUser(u)">
            <Trash2 :size="14" /> Supprimer
          </NButton>
        </div>
      </div>
    </div>

    <div v-else-if="!loadingList" class="empty">
      <Lock :size="26" style="opacity:.25" />
      <p>Chargez les comptes d'une installation Windows hors ligne pour commencer.</p>
    </div>
  </div>
</template>

<style scoped>
.accounts-tab { display: flex; flex-direction: column; gap: 16px; }
.intro { background: var(--bg-secondary); border: 1px solid var(--border); border-radius: var(--radius-xl); padding: 14px; display: flex; flex-direction: column; gap: 10px; }
.tool-header { display: flex; align-items: center; gap: 10px; color: var(--accent-primary); }
.tool-header h3 { font-size: 13px; font-weight: 700; color: var(--text-primary); }
.tool-desc { font-size: 12px; color: var(--text-muted); line-height: 1.5; }
.form-group { display: flex; flex-direction: column; gap: 6px; }
.form-label { font-size: 12px; font-weight: 600; color: var(--text-secondary); }
.form-input { background: var(--bg-primary); border: 1px solid var(--border); border-radius: var(--radius-md); padding: 6px 10px; font-size: 12px; color: var(--text-primary); width: 100%; font-family: monospace; }
.tool-actions { display: flex; flex-wrap: wrap; gap: 6px; }

.user-list { display: flex; flex-direction: column; gap: 10px; }
.user-card { background: var(--bg-secondary); border: 1px solid var(--border); border-radius: var(--radius-xl); padding: 12px 14px; display: flex; flex-direction: column; gap: 10px; }
.user-card.disabled { opacity: .72; }
.user-main { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; }
.user-name { font-size: 14px; font-weight: 700; color: var(--text-primary); display: flex; align-items: center; gap: 8px; }
.user-rid { font-size: 11px; font-weight: 500; color: var(--text-muted); font-family: monospace; }
.badges { display: flex; flex-wrap: wrap; gap: 6px; }
.badge { font-size: 10.5px; font-weight: 700; padding: 2px 8px; border-radius: 999px; border: 1px solid; }
.badge.ok { color: var(--success); border-color: rgba(34,197,94,.4); background: rgba(34,197,94,.08); }
.badge.off { color: var(--text-muted); border-color: var(--border); background: var(--bg-primary); }
.badge.admin { color: var(--accent-primary); border-color: rgba(255,140,0,.4); background: rgba(255,140,0,.08); }
.badge.warn { color: var(--danger); border-color: rgba(239,68,68,.4); background: rgba(239,68,68,.08); }
.badge.builtin { color: var(--text-secondary); border-color: var(--border); background: var(--bg-tertiary); }
.user-actions { display: flex; flex-wrap: wrap; gap: 6px; }

.empty { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 30px; color: var(--text-muted); font-size: 13px; }
</style>
