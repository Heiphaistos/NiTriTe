// Types partagés pour la page diagnostic et ses composables

export interface OsInfo {
  name: string; version: string; architecture: string
  hostname: string; build_number: string
}
export interface CpuInfo {
  name: string; manufacturer: string; cores: number; threads: number
  base_speed_mhz: number; max_speed_mhz?: number; usage_percent: number
}
export interface CpuExtendedInfo {
  socket_designation: string; family: string; stepping: string; revision: string;
  processor_id: string; max_clock_speed_mhz: number; current_voltage: string;
  external_clock_mhz: number; l2_cache_size_kb: number; l3_cache_size_kb: number;
  number_of_physical_packages: number; virtualization_enabled: boolean;
  address_width: number; data_width: number; characteristics: string[];
}
export interface CpuCacheInfo {
  l1_instruction_kb: number; l1_data_kb: number;
  l2_kb: number; l3_kb: number; l4_kb: number;
}
export interface FolderEntry { label: string; path: string; size_mb: number; size_gb?: number; file_count: number; partial?: boolean; }
export interface RamInfo { total_gb: number; used_gb: number; usage_percent: number; modules?: { memory_type?: string }[] }

export interface SysGpu { name?: string; adapter_ram_mb?: number; driver_version?: string }

export interface SysInfo {
  os: OsInfo; cpu: CpuInfo; ram: RamInfo
  gpus: SysGpu[]; disks: SysDisk[]; motherboard: unknown
}
export interface BiosInfo {
  manufacturer: string; version: string; release_date: string
  serial_number: string; smbios_version: string
}
export interface MoboDetailed {
  manufacturer: string; product: string; serial_number: string; version: string; status: string
}
export interface GpuDetailed {
  name: string; adapter_ram_mb: number; driver_version: string; driver_date: string
  video_processor: string; video_mode: string; current_resolution: string
  current_refresh_rate: number; status: string; pnp_device_id: string; adapter_dac_type: string
}
export interface RamSlot {
  bank_label: string; device_locator: string; manufacturer: string; capacity_gb: number
  speed_mhz: number; configured_speed_mhz: number; memory_type: string; form_factor: string
  serial_number: string; part_number: string; data_width: number
  min_voltage?: number; configured_voltage?: number
}
export interface RamDetailed { total_slots: number; used_slots: number; total_capacity_gb: number; slots: RamSlot[] }
export interface StoragePhysical {
  model: string; serial_number: string; firmware_revision: string; size_bytes: number; size_gb: number
  interface_type: string; media_type: string; status: string; pnp_device_id: string; partitions: number
}
export interface NetworkAdapter {
  name: string; description: string; mac_address: string; ip_addresses: string[]
  subnet_masks: string[]; default_gateway: string[]; dns_servers: string[]
  dhcp_enabled: boolean; dhcp_server: string; speed_mbps: number
  net_connection_id: string; is_physical: boolean; status: string
}
export interface CpuCache { l1_instruction_kb: number; l1_data_kb: number; l2_kb: number; l3_kb: number; l4_kb: number }
export interface CommandResult { success: boolean; stdout: string; stderr: string; exit_code: number }
export interface ShellResult { success: boolean; stdout: string; stderr: string; exit_code: number; shell_used: string }
export interface AppConfig {
  theme: string; language: string; sidebar_collapsed: boolean
  ollama_url: string; ollama_model: string; ollama_temperature: number
  ai_backend: string; llamacpp_server_path: string; llamacpp_model_path: string; llamacpp_port: number
  monitor_interval_ms: number; show_animations: boolean; compact_mode: boolean
  notifications_enabled: boolean; process_count: number; font_size: string; export_format: string
  auto_save?: boolean; disk_cache_enabled?: boolean; gpu_acceleration?: boolean; background_tasks?: boolean
  max_history_entries?: number; auto_refresh_dashboard?: boolean; dashboard_refresh_ms?: number
  notif_sounds?: boolean; notif_desktop?: boolean; notif_errors?: boolean; notif_success?: boolean
  notif_position?: string
}
export interface ProblemDevice { name: string; device_id: string; error_code: number; error_description: string; class: string }
export interface PowerPlanResult { name: string; guid: string; success: boolean; message: string }
export interface NetPingResult { success: boolean; avg: number; host: string }
export interface NetworkExtended {
  ping_gateway: NetPingResult | null; ping_google: NetPingResult; ping_cloudflare: NetPingResult
  public_ip: string
  arp_table: { ip: string; mac: string; type: string }[]
  routes: { prefix: string; next_hop: string; metric: number; iface: string }[]
  proxy: { enabled: boolean; server: string; bypass: string }
  firewall: { domain: boolean; private: boolean; public: boolean }
  shares: { name: string; path: string; desc: string }[]
  stats: { name: string; recv_bytes: number; sent_bytes: number }[]
  hosts_entries: { ip: string; host: string }[]
  dns_test: { name: string; ip: string; type: string }[]
  wifi_networks: string[]
}
export interface SysDiskPartition { mount_point: string; fs_type: string; used_gb: number; total_gb: number; free_gb: number; usage_percent: number }
export interface SysDisk { partitions: SysDiskPartition[]; model?: string; name?: string }
export interface OsExtended {
  install_date: string; last_boot_time: string; registered_user?: string; organization?: string
  os_language: string; timezone: string; product_type: string; pending_reboot: boolean
  country_code: string; locale: string; windows_directory: string; system_directory: string
  system_drive: string; boot_device: string; page_file_path: string
  total_virtual_memory_gb: number; free_virtual_memory_gb: number
  total_swap_gb: number; free_physical_gb: number
  powershell_version: string; dotnet_versions: string[]
}
export interface BiosExtended {
  firmware_type?: string; secure_boot?: boolean; tpm_present?: boolean
  tpm_enabled?: boolean; tpm_spec_version?: string; chassis_type?: string
  wake_on_lan?: boolean; fast_boot?: boolean
}
export interface ExpansionSlot {
  name?: string; type?: string; status?: number | string; max_data_width?: number
}
export interface MoboExtended {
  model?: string; cpu_socket?: string; motherboard_temp_c?: number; slot_count?: number
  expansion_slots?: ExpansionSlot[]
}
export interface MonitorDetail {
  name: string; screen_width: number; screen_height: number
  pixels_per_inch: number; manufacturer: string; availability: string;
  refresh_rate_hz?: number;
}
export interface AudioDevice { name: string; manufacturer: string; status: string; device_id: string; audio_type?: string }
export interface UsbDevice   { name: string; device_id: string; manufacturer: string; status: string; pnp_class: string }
export interface BatteryDetailed {
  name: string; status: string; estimated_charge_remaining: number; estimated_run_time: string
  design_capacity: number; full_charge_capacity: number; battery_health_percent: number
  chemistry: string; cycle_count: number
}
export interface InstalledSoftware {
  name: string; version: string; publisher: string; install_date: string
  install_location: string; estimated_size_mb: number
}
export interface StartupProgram { name: string; command: string; location: string; user: string; category: string }
export interface PowerPlan  { name: string; is_active: boolean; guid: string }
export interface PrinterDetail {
  name: string; driver_name: string; port_name: string
  is_default: boolean; is_network: boolean; status: string; shared: boolean
}
export interface EnvVar     { name: string; value: string; var_type: string }
export interface WinLicense {
  product_name: string; activation_status: string; partial_product_key: string; full_product_key: string
  license_status: string; license_family: string; office_name: string; office_status: string
  office_key: string; office_full_key: string
}
export interface InstalledUpdate { title: string; hotfix_id: string; description: string; installed_on: string; installed_by: string }

export interface ProcessInfo {
  pid: number; name: string; cpu_percent: number; memory_mb: number
  status: string; path: string; parent_pid: number; user: string
  start_time: number; virtual_memory_mb: number; gpu_percent?: number
}
export interface ServiceInfo {
  name: string; display_name: string; state: string; start_mode: string
  path_name: string; description: string; process_id: number
  service_type: string; started: boolean; account: string
}
export interface ScheduledTask {
  name: string; path: string; state: string; last_run_time: string
  next_run_time: string; last_task_result: number; trigger: string
  author: string; description: string; run_as_user: string
}
export interface VolumeInfo {
  drive_letter: string; label: string; filesystem: string
  total_gb: number; free_gb: number; used_gb: number; used_percent: number
  drive_type: string; health_status: string; operational_status: string; serial_number: string
}
export interface SmartDiskInfo {
  device_id: string; name: string; media_type: string
  health_status: string; operational_status: string; size_gb: number
  temperature: number; wear_level: number; read_errors_uncorrected: number
  write_errors_uncorrected: number; power_on_hours: number
  start_stop_cycles: number; reallocated_sectors: number
}
export interface ConnectionInfo {
  protocol: string; local_addr: string; remote_addr: string
  state: string; pid: number; process_name: string
}
export interface TcpConnection {
  protocol: string; local_address: string; local_port: number
  remote_address: string; remote_port: number; state: string
  pid: number; process_name: string; owning_module: string
}
export interface WifiInfo {
  ssid: string; bssid: string; signal_percent: number; band: string; channel: number
  security: string; receive_rate_mbps: number; transmit_rate_mbps: number
  state: string; adapter_name: string; authentication: string; protocol: string
}
export interface BitLockerDrive {
  drive_letter: string; protection_status: string; encryption_method: string
  conversion_status?: string; lock_status?: string
}
export interface SecurityStatus {
  secure_boot: string; tpm_version: string; tpm_enabled: boolean; tpm_activated: boolean
  uac_level: string; uac_enabled: boolean; bitlocker_drives: BitLockerDrive[]
  windows_hello: boolean; smartscreen_enabled: boolean; credential_guard: boolean
  hvci_enabled: boolean; vbs_enabled: boolean; lsa_protection: boolean
  firewall_domain: boolean; firewall_private: boolean; firewall_public: boolean
  antivirus_name: string; antivirus_state: string; windows_defender_realtime: boolean
  defender_definitions_date: string; last_full_scan: string
}

export interface ScanResult {
  bios_ok: boolean; bios_info: string | null
  battery_present: boolean; battery_health: number; battery_cycles: number
  suspicious_processes: { name: string; pid: number; path: string; reason: string }[]
  disk_usage: { drive: string; total_gb: number; free_gb: number; used_percent: number }[]
  winget_upgradable: { name: string; id: string; current_version: string; available_version: string }[]
  choco_upgradable: string[]
  dism_status: string; sfc_status: string; scan_errors: string[]
  uptime_hours: number; cpu_name: string; cpu_cores: number; cpu_usage_percent: number
  ram_total_gb: number; ram_used_gb: number; ram_usage_percent: number
  windows_version: string; windows_activation: string
  firewall_enabled: boolean; defender_enabled: boolean
  startup_count: number; pending_reboot: boolean
  recent_errors: { time: string; source: string; message: string; level: string }[]
  network_ok: boolean; open_ports: number[]
  antivirus_installed: string; defender_definition_age_days: number
  last_bsod: string; last_update_days: number; temp_folder_size_mb: number
  suspicious_services: { name: string; display_name: string; state: string; path: string }[]
  autorun_entries: { name: string; path: string; location: string }[]
  virtual_memory_total_mb: number; virtual_memory_available_mb: number
  gpu_name: string; gpu_vram_mb: number; screen_resolution: string
  power_plan: string; installed_software_count: number
  services_running: number; services_stopped: number
  network_adapters_summary: string; cpu_temperature: string
  windows_product_key: string; office_product_key: string; office_name: string
  bitlocker_volumes: { drive: string; protection_status: string; encryption_percent: number; recovery_password: string; protectors: string[] }[]
  motherboard: string; ram_detail: string; cpu_threads: number; cpu_frequency_ghz: number
  storage_items: { model: string; size_gb: number; media_type: string; interface_type: string; health: string; power_on_hours?: number; power_on_count?: number; rpm?: number }[]
  monitors_detail: string
  tpm_present: boolean; tpm_enabled: boolean; tpm_version: string
  secure_boot: boolean; uac_level: string; rdp_enabled: boolean
  smbv1_enabled: boolean; wmi_subscriptions: number
  local_admins: string[]; guest_enabled: boolean
  system_manufacturer: string; system_model: string; system_serial: string
  bios_manufacturer: string; bios_version: string; bios_date: string
  license_type: string; last_restore_point: string; pending_updates_cached: number
  top_cpu: { name: string; pid: number; value: number }[]
  top_ram: { name: string; pid: number; value: number }[]
  susp_tasks_count: number
  susp_tasks: { name: string; path: string; exec: string }[]
  all_gpus?: { name: string; vram_mb: number; is_integrated: boolean }[]
  ram_slots?: string[]
  cpu_socket?: string; cpu_l3_mb?: number
  wmi_subscription_details?: { name: string; consumer_type: string; path: string }[]
  dism_details?: string; sfc_details?: string
  windows_updates_pending?: string[]
  scoop_upgradable?: string[]
  activation_type?: string
  office_activation_type?: string
}

export interface DriverInstallResult { inf_path: string; success: boolean; output: string; duration_secs: number }
export interface AppLogEntry { timestamp: string; level: string; message: string }
export interface ToolEntry { name: string; description: string; command: string; is_url: boolean; section: string; icon: string; category?: string; requires_admin?: boolean }
export interface RecentInstall { name: string; version: string; publisher: string; install_date: string }
export interface RecentEvent { time: string; source: string; message: string; id: number; level: string }
export interface SystemHistory {
  windows_install_date: string; last_boot_time: string; current_uptime_hours: number
  total_uptime_days_since_install: number; bsod_count_30d: number; bsod_list: string[]
  recent_installs: RecentInstall[]; recent_uninstalls: string[]
  critical_events_7d: number; error_events_7d: number; warning_events_7d: number
  last_logon_user: string; shutdown_count: number; hibernation_count: number
  crash_count_30d: number; recent_critical: RecentEvent[]
}

export interface SystemMonitorPayload {
  cpu_percent?: number; cpu_usage?: number
  ram_percent?: number; ram_used_gb?: number; ram_total_gb?: number
  disk_percent?: number; disk_read_kbs?: number; disk_write_kbs?: number
  disk_temps?: { name: string; temp_c: number }[]
  network_down_kbs?: number; network_up_kbs?: number
  net_download_kbs?: number; net_upload_kbs?: number
  cpu_name?: string; cpu_temp_c?: number; cpu_temp_source?: string | null; cpu_freq_mhz?: number
  cpu_per_core?: (number | { id?: number; usage: number })[]
  cpu_cores?: (number | { id?: number; usage: number })[]
  gpu_data?: { name: string; usage_percent: number; vram_used_mb: number; vram_total_mb: number; temperature_c: number }[]
  top_processes?: { name: string; pid: number; cpu_percent: number; memory_mb?: number; ram_percent?: number }[]
  process_count?: number
  alerts?: { level: string; message: string }[]
}
