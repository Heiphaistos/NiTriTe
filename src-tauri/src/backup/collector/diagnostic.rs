//! Scan PC complet (page Diagnostic) exécuté SANS fenêtre Tauri, pour l'inclure
//! dans le lot de sauvegarde.
//!
//! `run_total_scan` est `async` et a besoin d'une `tauri::Window` (events de
//! progression). `create_backup` tourne dans un `spawn_blocking` sans fenêtre :
//! on rejoue donc la même orchestration de façon synchrone en appelant
//! directement les fonctions de collecte `pub` de `total_scan`.
//!
//! DISM/CheckHealth et SFC/verifyonly (plusieurs minutes, contrôles de réparation
//! d'intégrité, pas de l'inventaire) sont volontairement exclus du lot pour ne pas
//! bloquer « Tout sauvegarder ». L'utilisateur qui les veut lance le scan complet
//! depuis la page Diagnostic.
//
// ponytail: réplique l'orchestration de run_total_scan sans window/async. Si les
// deux divergent souvent, extraire un noyau commun sync dans total_scan.

use crate::system::extended_info::{get_bios_info, get_battery_extended};
use crate::system::scan_extra::collect_scan_extra;
use crate::system::scan_supplement::collect_scan_supplement;
use crate::system::total_scan::hw_extended::collect_hw_extended;
use crate::system::total_scan::security_checks::{
    collect_advanced_security, scan_autoruns, scan_suspicious_services,
};
use crate::system::total_scan::system_checks::{
    check_security, check_startup_and_reboot, check_windows_activation, collect_system_summary,
    get_disk_usage, get_listening_ports, get_recent_errors, is_chocolatey_installed,
    is_scoop_installed, list_choco_upgradable, list_scoop_upgradable, list_winget_upgradable,
    scan_suspicious_processes, test_network,
};
use crate::system::total_scan::ScanResult;

/// Exécute le scan diagnostic complet de façon synchrone (sans events de progression,
/// sans DISM/SFC). Retourne le même `ScanResult` que la page Diagnostic.
pub fn run_scan_blocking() -> ScanResult {
    let mut r = ScanResult {
        // -1 = « non mesuré » (distinct de 0).
        battery_cycles: -1,
        defender_definition_age_days: -1,
        last_update_days: -1,
        pending_updates_cached: -1,
        dism_status: "Non vérifié (lancer le scan complet depuis la page Diagnostic)".into(),
        sfc_status: "Non vérifié (lancer le scan complet depuis la page Diagnostic)".into(),
        ..ScanResult::default()
    };

    // BIOS + batterie
    if let Ok(info) = get_bios_info() {
        r.bios_ok = true;
        r.bios_info = Some(format!("{} {} ({})", info.manufacturer, info.version, info.release_date));
    }
    if let Ok(Some(bat)) = get_battery_extended() {
        r.battery_present = true;
        r.battery_health = bat.health_percent;
    }

    // Système (CPU, RAM, OS, uptime)
    let sys = collect_system_summary();
    r.uptime_hours = sys.uptime_hours;
    r.cpu_name = sys.cpu_name;
    r.cpu_cores = sys.cpu_cores;
    r.cpu_usage_percent = sys.cpu_usage;
    r.ram_total_gb = sys.ram_total;
    r.ram_used_gb = sys.ram_used;
    r.ram_usage_percent = sys.ram_pct;
    r.windows_version = sys.win_version;

    // Activation
    r.windows_activation = check_windows_activation();

    // Sécurité : firewall + Defender
    let (fw, def) = check_security();
    r.firewall_enabled = fw;
    r.defender_enabled = def;

    // Processus, disque, démarrage, événements
    r.suspicious_processes = scan_suspicious_processes();
    r.disk_usage = get_disk_usage();
    let (sc, pr) = check_startup_and_reboot();
    r.startup_count = sc;
    r.pending_reboot = pr;
    r.recent_errors = get_recent_errors();

    // Gestionnaires de paquets
    r.winget_upgradable = list_winget_upgradable();
    if is_chocolatey_installed() {
        r.choco_upgradable = list_choco_upgradable();
    }
    if is_scoop_installed() {
        r.scoop_upgradable = list_scoop_upgradable();
    }

    // Réseau
    r.network_ok = test_network();
    r.open_ports = get_listening_ports();

    // Sécurité avancée
    let adv = collect_advanced_security();
    r.antivirus_installed = adv.antivirus;
    r.defender_definition_age_days = adv.def_age_days;
    r.last_bsod = adv.last_bsod;
    r.last_update_days = adv.last_update_days;
    r.temp_folder_size_mb = adv.temp_size_mb;
    r.virtual_memory_total_mb = adv.vmem_total_mb;
    r.virtual_memory_available_mb = adv.vmem_available_mb;
    r.suspicious_services = scan_suspicious_services();
    r.autorun_entries = scan_autoruns();

    // Matériel étendu
    let hw = collect_hw_extended();
    r.gpu_name = hw.gpu_name;
    r.gpu_vram_mb = hw.gpu_vram_mb;
    r.all_gpus = hw.all_gpus;
    r.screen_resolution = hw.screen_resolution;
    r.power_plan = hw.power_plan;
    r.installed_software_count = hw.installed_software_count;
    r.services_running = hw.services_running;
    r.services_stopped = hw.services_stopped;
    r.network_adapters_summary = hw.network_adapters_summary;
    r.cpu_temperature = hw.cpu_temperature;

    // Licences, BitLocker, composants
    let sup = collect_scan_supplement();
    r.windows_product_key = sup.windows_product_key;
    r.office_product_key = sup.office_product_key;
    r.office_name = sup.office_name;
    r.bitlocker_volumes = sup.bitlocker_volumes;
    r.motherboard = sup.motherboard;
    r.ram_detail = sup.ram_detail;
    r.ram_slots = sup.ram_slots;
    r.cpu_threads = sup.cpu_threads;
    r.cpu_frequency_ghz = sup.cpu_frequency_ghz;
    r.cpu_socket = sup.cpu_socket;
    r.cpu_l3_mb = sup.cpu_l3_mb;
    r.storage_items = sup.storage_items;
    r.monitors_detail = sup.monitors_detail;

    // TPM, Secure Boot, identité système, tâches, mises à jour
    let extra = collect_scan_extra();
    r.tpm_present = extra.tpm_present;
    r.tpm_enabled = extra.tpm_enabled;
    r.tpm_version = extra.tpm_version;
    r.secure_boot = extra.secure_boot;
    r.uac_level = extra.uac_level;
    r.rdp_enabled = extra.rdp_enabled;
    r.smbv1_enabled = extra.smbv1_enabled;
    r.wmi_subscriptions = extra.wmi_subscriptions;
    r.wmi_subscription_details = extra.wmi_subscription_details;
    r.local_admins = extra.local_admins;
    r.guest_enabled = extra.guest_enabled;
    r.system_manufacturer = extra.system_manufacturer;
    r.system_model = extra.system_model;
    r.system_serial = extra.system_serial;
    r.bios_manufacturer = extra.bios_manufacturer;
    r.bios_version = extra.bios_version;
    r.bios_date = extra.bios_date;
    r.license_type = extra.license_type;
    r.last_restore_point = extra.last_restore_point;
    r.pending_updates_cached = extra.pending_updates_cached;
    r.top_cpu = extra.top_cpu;
    r.top_ram = extra.top_ram;
    r.susp_tasks_count = extra.susp_tasks_count;
    r.susp_tasks = extra.susp_tasks;
    r.windows_updates_pending = extra.windows_updates_pending;
    r.activation_type = extra.activation_type;
    r.office_activation_type = extra.office_activation_type;

    r
}
