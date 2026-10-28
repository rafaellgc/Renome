import { settings } from "./settings.svelte";
import type { Rule, RuleType, Step } from "./types";

export const RULE_TYPES: RuleType[] = [
  "replace", "regex", "rearrange", "case", "insert", "number", "remove", "removeRange", "charAt", "extension", "movie", "series", "music", "book", "comic",
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
const BOOK_TEMPLATE = {
  pt: ["{autor} - {título} ({ano})", "{autor}/{título}"],
  en: ["{author} - {title} ({year})", "{author}/{title}"],
  es: ["{autor} - {título} ({año})", "{autor}/{título}"],
} as const;
const COMIC_TEMPLATE = {
  pt: ["{série} v{volume} #{número} c{capítulo} - {título} ({ano})", "{série} ({ano da série})"],
  en: ["{series} v{volume} #{number} c{chapter} - {title} ({year})", "{series} ({series year})"],
  es: ["{serie} v{volumen} #{número} c{capítulo} - {título} ({año})", "{serie} ({año de la serie})"],
} as const;
/** Kavita reads `Series v01 #001 c001`; a title after " - " is not in its
 *  examples, so it is left out. */
const KAVITA_TEMPLATE = {
  pt: ["{série} v{volume} #{número} c{capítulo} ({ano})", "{série} ({ano da série})"],
  en: ["{series} v{volume} #{number} c{chapter} ({year})", "{series} ({series year})"],
  es: ["{serie} v{volumen} #{número} c{capítulo} ({año})", "{serie} ({año de la serie})"],
} as const;

export type ComicPreset = "renome" | "kavita";

/** A ready-made comic template and folder template, in the current
 *  language; both servers want each series in its own folder. */
export function comicPreset(preset: ComicPreset): { template: string; folderTemplate: string; folders: true } {
  const [template, folderTemplate] = (preset === "kavita" ? KAVITA_TEMPLATE : COMIC_TEMPLATE)[settings.lang];
  return { template, folderTemplate, folders: true };
}

/** The preset a rule's templates match, if any. */
export function comicPresetOf(r: { template: string; folderTemplate: string; folders: boolean }): ComicPreset | "" {
  for (const p of ["renome", "kavita"] as const) {
    const t = comicPreset(p);
    if (r.folders && r.template === t.template && r.folderTemplate === t.folderTemplate) return p;
  }
  return "";
}
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
/** Book formats (as the Rust core). */
const BOOK = new Set(["epub", "kepub", "mobi", "azw", "azw3", "prc", "pdf", "fb2", "djvu", "djv"]);
export const isBook = (name: string) => BOOK.has(splitExt(name)[1].toLowerCase());
/** Comic formats (as the Rust core). */
const COMIC = new Set(["cbz", "cbr", "cb7", "cbt", "pdf", "epub"]);
export const isComic = (name: string) => COMIC.has(splitExt(name)[1].toLowerCase());

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
        folders: false, folderTemplate: SHOW_FOLDER[settings.lang], sidecar: false, fanart: false, missing: true,
      };
    case "music": {
      const [template, folderTemplate] = MUSIC_TEMPLATE[settings.lang];
      return { type, template, colon: " - ", discPrefix: true, fromName: false, folders: false, folderTemplate, cover: false, playlist: false, listen: true, writeTags: false, embedCover: true, cleanTags: true };
    }
    case "book": {
      const [template, folderTemplate] = BOOK_TEMPLATE[settings.lang];
      return { type, template, colon: " - ", folders: false, folderTemplate, cover: false, opf: false };
    }
    case "comic": {
      const [template, folderTemplate] = COMIC_TEMPLATE[settings.lang];
      return { type, template, colon: " - ", series: "", source: "gcd", folders: false, folderTemplate, byFolder: false };
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
