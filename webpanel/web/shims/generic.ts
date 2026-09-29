/** Plugins sans usage dans le panneau web (os, notification, http…). */
export async function platform(): Promise<string> { return "windows"; }
export async function isPermissionGranted(): Promise<boolean> { return false; }
export async function requestPermission(): Promise<string> { return "denied"; }
export async function sendNotification(): Promise<void> {}
export const fetch = globalThis.fetch.bind(globalThis);
