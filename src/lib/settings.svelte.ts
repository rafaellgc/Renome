import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { DetailColumn, Step } from "./types";

export type Lang = "pt" | "en" | "es";
export type Theme = "light" | "dark" | "system";

export interface Settings {
  lang: Lang;
  theme: Theme;
  includeHidden: boolean;
  confirmApply: boolean;
  disableRulesAfterApply: boolean;
  warnWindows: boolean;
  rememberRules: boolean;
  /** TMDB API key or read access token (empty: not set up). */
  tmdbKey: string;
  /** Language of TMDB titles; empty follows the interface. */
  tmdbLang: string;
  /** Comic Vine API key (empty: not set up). */
  comicVineKey: string;
  /** Discogs personal token (empty: not set up). */
  discogsToken: string;
  /** Grand Comics Database account (empty: searched without one). */
  gcdEmail: string;
  gcdPassword: string;
  /** Rules from the last session (when `rememberRules`). */
  rules: Step[];
  /** The optional columns shown in the list, in their order. */
  columns: DetailColumn[];
  /** The faded rows of what is missing are shown (they are still counted). */
  showMissing: boolean;
}

const DEFAULTS: Settings = {
  lang: "en",
  theme: "system",
  includeHidden: false,
  confirmApply: true,
  disableRulesAfterApply: true,
  warnWindows: true,
  rememberRules: true,
  tmdbKey: "",
  tmdbLang: "",
  comicVineKey: "",
  discogsToken: "",
  gcdEmail: "",
  gcdPassword: "",
  rules: [],
  columns: [],
  showMissing: true,
};

export const settings = $state<Settings>({ ...DEFAULTS });

function toLang(code: string): Lang {
  return code === "pt" || code === "es" ? code : "en";
}

/** Old default templates, replaced by the current ones when a saved rule
 *  still has them (the user did not change them). */
const OLD_COMIC_TEMPLATES: Record<string, string> = {
  "{série} v{volume} #{número} c{capítulo} ({ano})": "{série} v{volume} #{número} c{capítulo} - {título} ({ano})",
  "{series} v{volume} #{number} c{chapter} ({year})": "{series} v{volume} #{number} c{chapter} - {title} ({year})",
  "{serie} v{volumen} #{número} c{capítulo} ({año})": "{serie} v{volumen} #{número} c{capítulo} - {título} ({año})",
};

function upgradeRules(steps: Step[]) {
  for (const s of steps ?? []) {
    // Rules from before the missing episodes, issues, movies, tracks and
    // books were shown.
    const r = s.rule as { type?: string; missing?: boolean } | undefined;
    if (r && ["series", "comic", "movie", "music", "book"].includes(r.type ?? "")) r.missing ??= true;
    if (s.rule?.type !== "comic") continue;
    if (OLD_COMIC_TEMPLATES[s.rule.template]) s.rule.template = OLD_COMIC_TEMPLATES[s.rule.template];
    // Rules from before there was a choice of catalogue.
    s.rule.source ??= "comicvine";
    s.rule.byFolder ??= false;
  }
}

/** Loads the settings; on first run, uses the system language. */
export async function loadSettings() {
  const saved = await invoke<Partial<Settings> | null>("load_settings");
  if (saved) {
    Object.assign(settings, DEFAULTS, saved);
    upgradeRules(settings.rules);
  } else {
    const sys = await invoke<string>("system_language");
    settings.lang = toLang(sys || navigator.language.slice(0, 2));
    await saveSettings();
  }
  applyTheme();
}

/** Language for TMDB titles and overviews. */
export function tmdbLang() {
  return settings.tmdbLang || { pt: "pt-BR", en: "en-US", es: "es-ES" }[settings.lang];
}

export function saveSettings() {
  return invoke<void>("save_settings", { settings: $state.snapshot(settings) });
}

/** The settings kept in the system's password store, not in the file. */
export const SECRET_FIELDS = ["tmdbKey", "comicVineKey", "discogsToken", "gcdEmail", "gcdPassword"] as const;

/** Whether the secrets came (the store may wait to be unlocked or
 *  allowed), and where they are kept: `stored` in the password store,
 *  `refused` when the store did not answer (what it holds stays there,
 *  untouched; keys typed meanwhile go to the settings file), `missing`
 *  when the system has none (the settings file). */
export const secrets = $state<{ ready: boolean; status: "stored" | "refused" | "missing" | null }>({ ready: false, status: null });

/** The secrets as last loaded or saved: only a change is saved, so a
 *  store that did not answer is never written over with empty fields. */
let saved = "";
const current = () => JSON.stringify(SECRET_FIELDS.map((k) => settings[k]));

/** Fills the secrets from the system's password store, when it answers;
 *  without a store, those in the settings file hold. */
export async function loadSecrets() {
  secrets.ready = false;
  try {
    const r = await invoke<{ status: "stored" | "refused" | "missing"; secrets?: Partial<Settings> }>("load_secrets");
    if (r.secrets) {
      for (const k of SECRET_FIELDS) if (typeof r.secrets[k] === "string" && r.secrets[k]) settings[k] = r.secrets[k];
    }
    secrets.status = r.status;
  } catch {
    secrets.status = "refused";
  }
  saved = current();
  secrets.ready = true;
}

export async function saveSecrets() {
  if (!secrets.ready || current() === saved) return;
  const values = Object.fromEntries(SECRET_FIELDS.map((k) => [k, settings[k]]));
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
