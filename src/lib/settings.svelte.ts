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

/** Whether the secrets came (the store may wait to be unlocked), and
 *  where they are kept: in the password store, or (when the system has
 *  none) in the settings file. */
export const secrets = $state<{ ready: boolean; stored: boolean | null }>({ ready: false, stored: null });

/** Fills the secrets from the system's password store, when it answers;
 *  without a store, those in the settings file hold. */
export async function loadSecrets() {
  try {
    const found = await invoke<Partial<Settings> | null>("load_secrets");
    if (found) {
      for (const k of SECRET_FIELDS) if (typeof found[k] === "string" && found[k]) settings[k] = found[k];
    }
    secrets.stored = found !== null;
  } catch {
    secrets.stored = false;
  }
  secrets.ready = true;
}

export async function saveSecrets() {
  if (!secrets.ready) return;
  const values = Object.fromEntries(SECRET_FIELDS.map((k) => [k, settings[k]]));
  secrets.stored = await invoke<boolean>("save_secrets", { secrets: values });
}

// Styles that differ on macOS (see app.css).
if (navigator.userAgent.includes("Mac OS X")) document.documentElement.classList.add("mac");

const dark = window.matchMedia("(prefers-color-scheme: dark)");

export function applyTheme() {
  const mode = settings.theme === "system" ? (dark.matches ? "dark" : "light") : settings.theme;
  document.documentElement.dataset.theme = mode;
  // Windows draws the title bar itself: in the same theme (null: the system's).
  if (navigator.userAgent.includes("Windows")) {
    getCurrentWindow()
      .setTheme(settings.theme === "system" ? null : settings.theme)
      .catch(() => {});
  }
}

dark.addEventListener("change", applyTheme);
