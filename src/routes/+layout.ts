// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
export const ssr = false;

import { loadSettings } from "$lib/settings.svelte";

// The settings and the rules' table (from Rust) before the page shows:
// everything on it reads them.
export const load = async () => {
  await loadSettings();
};
