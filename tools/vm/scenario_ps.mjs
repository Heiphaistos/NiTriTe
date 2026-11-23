// Lance une commande PowerShell dans la VM (lecture) : SCRIPT=... node vm_cdp.mjs <ip> scenario_ps.mjs
export default async ({ invoke, log }) => {
  const r = await invoke("run_system_command", { cmd: "powershell", args: ["-NoProfile", "-Command", process.env.SCRIPT], timeoutSecs: 300 });
  log(r.stdout, r.stderr);
};
