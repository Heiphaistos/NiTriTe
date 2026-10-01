import { createApp } from "vue";
import { createPinia } from "pinia";
import piniaPluginPersistedstate from "pinia-plugin-persistedstate";
import App from "./App.vue";
import router from "./router";
import "./assets/styles/main.css";
import "./assets/styles/tab-styles.css";
import "./assets/diagnostic.css";
import "./assets/styles/shells.css";
import "./assets/styles/ui-models.css";
import { logger, setupGlobalErrorHandlers } from "./utils/logger";
import { useAppStore } from "./stores/app";

// ── Logger : intercepteurs globaux (window.onerror, unhandledrejection, console) ──
setupGlobalErrorHandlers();

const pinia = createPinia();
pinia.use(piniaPluginPersistedstate);

const app = createApp(App);

// Capture globale des erreurs Vue → logger + console
app.config.errorHandler = (err, _instance, info) => {
  console.error(`[Nitrite][Vue error][${info}]`, err);
  logger.vue(info, err);
};

app.use(pinia);
app.use(router);

// Profil de performance appliqué AVANT le premier rendu (splash compris) :
// un PC modeste ne doit jamais payer les effets, même quelques secondes.
useAppStore(pinia).loadPerfMode();

app.mount("#app");
