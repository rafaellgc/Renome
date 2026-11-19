import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { DetailColumn, MediaServer, Step } from "./types";
import { TABLE, setRuleTable, type RuleTable } from "./rules";

export type Lang = "pt" | "en" | "es";
export type Theme = "light" | "dark" | "system";
export type RuleSet = { name: string; steps: Step[] };
/** How big the window's text (and all else, in proportion) is. */
export type TextSize = "normal" | "large" | "larger";
export const TEXT_SCALE: Record<TextSize, number> = { normal: 1, large: 1.15, larger: 1.3 };

export interface Settings {
  lang: Lang;
  theme: Theme;
  textSize: TextSize;
  includeHidden: boolean;
  confirmApply: boolean;
  disableRulesAfterApply: boolean;
  warnWindows: boolean;
  rememberRules: boolean;
  /** Two files given the same name: number the later ones. */
  numberClashes: boolean;
  /** TMDB API key or read access token (empty: not set up). */
  tmdbKey: string;
  /** Language of TMDB titles; empty follows the interface. */
  tmdbLang: string;
  /** Comic Vine API key (empty: not set up). */
  comicVineKey: string;
  /** Discogs personal token (empty: not set up). */
  discogsToken: string;
  /** Metron API token (empty: not set up). */
  metronToken: string;
  /** Google Books API key (empty: not used). */
  googleBooksKey: string;
  /** Grand Comics Database account (empty: searched without one). */
  gcdEmail: string;
  gcdPassword: string;
  /** OpenSubtitles.com API key and account (empty: not used). */
  osKey: string;
  osUser: string;
  osPassword: string;
  /** SubDL API key (empty: not used). */
  subdlKey: string;
  subsourceKey: string;
  /** Media servers told about the folders where files were renamed. */
  servers: MediaServer[];
  /** Their keys, by server id, as JSON (a secret). */
  serverKeys: string;
  /** Look for a new version on GitHub when the program opens. */
  checkUpdates: boolean;
  /** Rules from the last session (when `rememberRules`). */
  rules: Step[];
  /** Rule sets saved by name, in the order they were saved. */
  ruleSets: RuleSet[];
  /** The optional columns shown in the list, in their order. */
  columns: DetailColumn[];
  /** The faded rows of what is missing are shown (they are still counted). */
  showMissing: boolean;
  /** Days the catalogues' answers are kept on disk; 0: not kept. */
  cacheDays: number;
}

/** Filled by {@link loadSettings} before the page shows (see
 *  `routes/+layout.ts`): every setting comes from the Rust options table
 *  (`options::whole`, the command line's too), at its default when not
 *  saved, the remembered rules brought up to date. */
export const settings = $state<Settings>({} as Settings);

/** Loads the settings (on first run, in the system's language), then
 *  the rules' table in that language. */
export async function loadSettings() {
  const saved = await invoke<Settings>("load_settings", { fallbackLang: navigator.language });
  Object.assign(settings, saved);
  setRuleTable(await invoke<RuleTable>("rule_table", { lang: settings.lang }));
  await refreshLangs();
  applyTheme();
  applyTextSize();
}

/** The languages the catalogues are asked in (Rust's `catalog_langs`):
 *  asked again when the interface's or TMDB's language changes. */
export const langs = $state<{ tmdb: string; comic: string }>({ tmdb: "en-US", comic: "en" });
export async function refreshLangs() {
  Object.assign(langs, await invoke<{ tmdb: string; comic: string }>("catalog_langs", { tmdbLang: settings.tmdbLang ?? "", lang: settings.lang }));
}

/** The window zoomed by the text size chosen. */
export function applyTextSize() {
  getCurrentWebview()
    .setZoom(TEXT_SCALE[settings.textSize] ?? 1)
    .catch(() => {});
}

/** The key a comic catalogue is asked with: Comic Vine's, Metron's token. */
export function comicKey(source: string): string {
  const name = (TABLE.options.comicKeys[source] ?? "comicVineKey") as keyof Settings;
  return String(settings[name] ?? "").trim();
}

/** Language for TMDB titles and overviews. */
export function tmdbLang() {
  return langs.tmdb;
}

export function saveSettings() {
  return invoke<void>("save_settings", { settings: $state.snapshot(settings) });
}

/** The settings kept in the system's password store, not in the file
 *  (the Rust table's). */
export const secretFields = () => TABLE.options.secrets as (keyof Settings & string)[];

/** Whether the secrets came (the store may wait to be unlocked or
 *  allowed), and where they are kept: `stored` in the password store,
 *  `refused` when the store did not answer (what it holds stays there,
 *  untouched; keys typed meanwhile go to the settings file), `missing`
 *  when the system has none (the settings file). */
export const secrets = $state<{ ready: boolean; status: "stored" | "refused" | "missing" | null }>({ ready: false, status: null });

/** The secrets as last loaded or saved: only a change is saved, so a
 *  store that did not answer is never written over with empty fields. */
let saved = "";
const current = () => JSON.stringify(secretFields().map((k) => settings[k]));

/** Fills the secrets from the system's password store, when it answers;
 *  without a store, those in the settings file hold. */
export async function loadSecrets() {
  secrets.ready = false;
  // What the fields held before the store answered: one typed meanwhile
  // (a slow or locked store) wins over the store's, and is saved below.
  const before = Object.fromEntries(secretFields().map((k) => [k, settings[k]]));
  let typed = false;
  try {
    const r = await invoke<{ status: "stored" | "refused" | "missing"; secrets?: Partial<Settings> }>("load_secrets");
    const got = r.secrets ?? {};
    for (const k of secretFields()) {
      if (settings[k] !== before[k]) typed = true;
      else if (typeof got[k] === "string" && got[k]) (settings as unknown as Record<string, unknown>)[k] = got[k];
    }
    secrets.status = r.status;
    // The baseline is what the store holds, so what was typed differs.
    saved = JSON.stringify(secretFields().map((k) => (typeof got[k] === "string" && got[k] ? got[k] : before[k])));
  } catch {
    secrets.status = "refused";
    saved = current();
  }
  secrets.ready = true;
  if (typed) await saveSecrets().catch(() => {});
}

export async function saveSecrets() {
  if (!secrets.ready || current() === saved) return;
  const values = Object.fromEntries(secretFields().map((k) => [k, settings[k]]));
  saved = current();
  const stored = await invoke<boolean>("save_secrets", { secrets: values });
  if (secrets.status === "stored" && !stored) secrets.status = "refused";
}

// Styles that differ on macOS (see app.css).
if (navigator.userAgent.includes("Mac OS X")) document.documentElement.classList.add("mac");

const dark = window.matchMedia("(prefers-color-scheme: dark)");

export function applyTheme() {
  const mode = settings.theme === "system" ? (dark.matches ? "dark" : "light") : settings.theme;
  document.documentElement.dataset.theme = mode;
  // Windows and macOS draw the title bar themselves: in the same theme
  // (null: the system's).
  if (navigator.userAgent.includes("Windows") || navigator.userAgent.includes("Mac OS X")) {
    getCurrentWindow()
      .setTheme(settings.theme === "system" ? null : settings.theme)
      .catch(() => {});
  }
}

dark.addEventListener("change", applyTheme);
