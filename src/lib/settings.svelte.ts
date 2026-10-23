import { invoke } from "@tauri-apps/api/core";
import type { Step } from "./types";

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
  /** Rules from the last session (when `rememberRules`). */
  rules: Step[];
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
  rules: [],
};

export const settings = $state<Settings>({ ...DEFAULTS });

function toLang(code: string): Lang {
  return code === "pt" || code === "es" ? code : "en";
}

/** Loads the settings; on first run, uses the system language. */
export async function loadSettings() {
  const saved = await invoke<Partial<Settings> | null>("load_settings");
  if (saved) {
    Object.assign(settings, DEFAULTS, saved);
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

const dark = window.matchMedia("(prefers-color-scheme: dark)");

export function applyTheme() {
  const mode = settings.theme === "system" ? (dark.matches ? "dark" : "light") : settings.theme;
  document.documentElement.dataset.theme = mode;
}

dark.addEventListener("change", applyTheme);
