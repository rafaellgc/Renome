import { settings } from "./settings.svelte";
import type { Rule, RuleType, Step } from "./types";

export const RULE_TYPES: RuleType[] = [
  "replace", "regex", "rearrange", "case", "insert", "number", "remove", "removeRange", "charAt", "extension", "movie", "series", "music",
];

const NAME_VAR = { pt: "{nome}", en: "{name}", es: "{nombre}" } as const;
const MOVIE_TEMPLATE = {
  pt: "{título} ({ano}) [tmdbid-{id}]",
  en: "{title} ({year}) [tmdbid-{id}]",
  es: "{título} ({año}) [tmdbid-{id}]",
} as const;
const SHOW_FOLDER = {
  pt: "{série} ({ano}) [tmdbid-{id}]",
  en: "{show} ({year}) [tmdbid-{id}]",
  es: "{serie} ({año}) [tmdbid-{id}]",
} as const;
const MUSIC_TEMPLATE = {
  pt: ["{faixa} - {título}", "{artista do álbum}/{álbum} ({ano})"],
  en: ["{track} - {title}", "{album artist}/{album} ({year})"],
  es: ["{pista} - {título}", "{artista del álbum}/{álbum} ({año})"],
} as const;
const SERIES_TEMPLATE = {
  pt: "{série} ({ano}) - S{temporada}E{episódio} - {título}",
  en: "{show} ({year}) - S{season}E{episode} - {title}",
  es: "{serie} ({año}) - S{temporada}E{episodio} - {título}",
} as const;

/** Extensions that get .nfo and images (subtitles share the movie's name but not those). */
const VIDEO = new Set(["mkv", "mp4", "m4v", "avi", "mov", "wmv", "ts", "m2ts", "mts", "webm", "mpg", "mpeg", "flv", "ogv", "iso", "vob", "divx", "rmvb"]);
export const isVideo = (name: string) => VIDEO.has(splitExt(name)[1].toLowerCase());
/** Audio formats the music rule reads tags from (as the Rust core). */
const AUDIO = new Set([
  "mp3", "mp2", "mp1", "flac", "ogg", "opus", "spx", "m4a", "m4b", "m4p", "m4r", "aac",
  "aiff", "aif", "afc", "aifc", "wav", "wave", "wv", "ape", "mpc", "mp+", "mpp",
]);
export const isAudio = (name: string) => AUDIO.has(splitExt(name)[1].toLowerCase());

export function newRule(type: RuleType): Rule {
  switch (type) {
    case "replace":
      return { type, find: "", replace: "", caseSensitive: false, firstOnly: false, target: "name" };
    case "regex":
      return { type, pattern: "", replace: "", caseSensitive: false, firstOnly: false, target: "name" };
    case "case":
      return { type, mode: "titleSmart", target: "name" };
    case "insert":
      return { type, source: "text", text: "", dateFormat: "%Y-%m-%d", fallback: true, at: "start", position: 1 };
    case "number":
      return { type, template: `${NAME_VAR[settings.lang]} {n}`, start: 1, step: 1, digits: 2, perFolder: false };
    case "remove":
      return { type, allSpaces: false, extraSpaces: true, accents: false, digits: false, chars: "", target: "name" };
    case "removeRange":
      return { type, from: "text", fromPosition: 1, fromText: "", to: "text", toPosition: 1, toText: "", inclusive: true };
    case "charAt":
      return { type, position: 1, fromEnd: false, onlyIf: "", caseSensitive: false, replace: "" };
    case "extension":
      return { type, mode: "lower", value: "" };
    case "rearrange":
      return { type, separator: " - ", order: "", target: "name" };
    case "movie":
      return { type, template: MOVIE_TEMPLATE[settings.lang], colon: " - ", sidecar: false, fanart: false, folder: false };
    case "series":
      return {
        type, template: SERIES_TEMPLATE[settings.lang], colon: " - ", show: "",
        folders: false, folderTemplate: SHOW_FOLDER[settings.lang], sidecar: false, fanart: false,
      };
    case "music": {
      const [template, folderTemplate] = MUSIC_TEMPLATE[settings.lang];
      return { type, template, colon: " - ", discPrefix: true, fromName: false, folders: false, folderTemplate, cover: false, playlist: false, listen: true, writeTags: false, embedCover: true, cleanTags: true };
    }
  }
}

/** Same split as the Rust rule: trimmed parts, empty ones dropped. */
export function splitParts(name: string, separator: string): string[] {
  if (!separator) return [];
  return name.split(separator).map((p) => p.trim()).filter(Boolean);
}

/** Splits `name.ext` like the Rust core (hidden files have no extension). */
export function splitExt(name: string): [string, string] {
  const dot = name.lastIndexOf(".");
  return dot > 0 && dot < name.length - 1 ? [name.slice(0, dot), name.slice(dot + 1)] : [name, ""];
}

let counter = 0;
/** Options added in later versions take their defaults in saved rules. */
export function newStep(rule: Rule, enabled = true): Step {
  const full = { ...newRule(rule.type), ...rule } as Rule;
  return { id: `s${Date.now().toString(36)}${counter++}`, enabled, rule: full };
}
