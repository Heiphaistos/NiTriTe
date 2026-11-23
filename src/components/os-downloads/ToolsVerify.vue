<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) — vérification d'un fichier (SHA-256,
// signature Authenticode) et d'un manifeste SHA-256 signé OpenPGP (GnuPG requis).
import { ref } from "vue";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import { FileCheck, KeyRound } from "lucide-vue-next";
import { api, displayBytes, errorText } from "./useIsoCatalog";

interface Verified { name: string; size: number; sha256: string; expected: string; matches: string[]; match: boolean | null; signature: { status: string; signer: string; thumbprint: string } | null }
interface Manifest { name: string; fingerprint: string; sha256: string; expected: string; match: boolean }

const SIGNATURES: Record<string, string> = {
  Valid: "Signature valide", NotSigned: "Fichier non signé", HashMismatch: "Fichier modifié après signature",
  NotTrusted: "Signature non approuvée", Unavailable: "Signature non vérifiable",
};

const path = ref("");
const expected = ref("");
const result = ref<Verified | null>(null);
const error = ref("");
const busy = ref(false);

const pgp = ref({ media: "", manifest: "", signature: "", key: "", fingerprint: "" });
const pgpResult = ref<Manifest | null>(null);
const pgpError = ref("");
const pgpBusy = ref(false);

async function pick(): Promise<string> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const file = await open({ title: "Choisir un fichier" });
  return typeof file === "string" ? file : "";
}

async function verify() {
  busy.value = true;
  error.value = "";
  result.value = null;
  try { result.value = await api<Verified>("media-verify", { path: path.value, sha256: expected.value.trim() }, 3_600_000); }
  catch (e) { error.value = errorText(e); }
  finally { busy.value = false; }
}

async function verifyManifest() {
  pgpBusy.value = true;
  pgpError.value = "";
  pgpResult.value = null;
  try { pgpResult.value = await api<Manifest>("media-manifest", { ...pgp.value }, 3_600_000); }
  catch (e) { pgpError.value = errorText(e); }
  finally { pgpBusy.value = false; }
}
</script>

<template>
  <div class="iso-stack">
    <NCard>
      <div class="iso-stack">
        <span class="iso-section-title"><FileCheck :size="16" /> Vérifier un fichier</span>
        <div class="iso-row">
          <input v-model="path" class="iso-field iso-field--grow" placeholder="Chemin du fichier" readonly />
          <NButton size="sm" variant="secondary" @click="pick().then(p => p && (path = p))">Parcourir</NButton>
        </div>
        <input v-model="expected" class="iso-field" placeholder="Empreinte SHA-256 attendue (facultatif)" maxlength="64" />
        <div><NButton size="sm" :disabled="!path" :loading="busy" @click="verify">Calculer et comparer</NButton></div>
        <p v-if="error" class="iso-error">{{ error }}</p>
        <div v-if="result" class="iso-stack">
          <span class="iso-small">{{ result.name }} · {{ displayBytes(result.size) }}</span>
          <code class="iso-hash">SHA-256 {{ result.sha256 }}</code>
          <div class="iso-row">
            <NBadge v-if="result.match === true" variant="success">Empreinte conforme</NBadge>
            <NBadge v-else-if="result.match === false" variant="danger">Empreinte différente</NBadge>
            <NBadge v-if="result.matches.length" variant="success">Reconnu dans le catalogue</NBadge>
            <NBadge v-if="result.signature" :variant="result.signature.status === 'Valid' ? 'success' : 'warning'">
              {{ SIGNATURES[result.signature.status] ?? result.signature.status }}
            </NBadge>
          </div>
          <span v-if="result.matches.length" class="iso-muted">{{ result.matches.join(", ") }}</span>
          <span v-if="result.signature?.signer" class="iso-muted">Signataire : {{ result.signature.signer }} · {{ result.signature.thumbprint }}</span>
        </div>
      </div>
    </NCard>

    <NCard>
      <div class="iso-stack">
        <span class="iso-section-title"><KeyRound :size="16" /> Manifeste signé (OpenPGP)</span>
        <p class="iso-small">Vérifie la signature d’un fichier SHA256SUMS publié par l’éditeur, puis compare l’empreinte de ton image. Nécessite Gpg4win.</p>
        <div v-for="[field, label] in ([['media', 'Image'], ['manifest', 'Manifeste (SHA256SUMS)'], ['signature', 'Signature (.sig / .asc / .gpg)'], ['key', 'Clé publique de l’éditeur']] as const)" :key="field" class="iso-row">
          <span class="iso-small" style="width: 200px">{{ label }}</span>
          <input v-model="pgp[field]" class="iso-field iso-field--grow" readonly />
          <NButton size="sm" variant="secondary" @click="pick().then(p => p && (pgp[field] = p))">Parcourir</NButton>
        </div>
        <input v-model="pgp.fingerprint" class="iso-field" placeholder="Empreinte complète de la clé publiée par l’éditeur (40 ou 64 caractères)" />
        <div><NButton size="sm" :disabled="!pgp.media || !pgp.manifest || !pgp.signature || !pgp.key" :loading="pgpBusy" @click="verifyManifest">Vérifier</NButton></div>
        <p v-if="pgpError" class="iso-error">{{ pgpError }}</p>
        <div v-if="pgpResult" class="iso-stack">
          <NBadge :variant="pgpResult.match ? 'success' : 'danger'">{{ pgpResult.match ? "Signature valide et empreinte conforme" : "Signature valide mais empreinte différente" }}</NBadge>
          <code class="iso-hash">Attendue {{ pgpResult.expected }}</code>
          <code class="iso-hash">Calculée {{ pgpResult.sha256 }}</code>
        </div>
      </div>
    </NCard>
  </div>
</template>
