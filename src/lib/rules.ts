import { settings } from "./settings.svelte";
import type { Rule, RuleType, Step } from "./types";

export const RULE_TYPES: RuleType[] = [
  "replace", "regex", "rearrange", "case", "insert", "number", "remove", "convert", "removeRange", "charAt", "extension", "movie", "series", "music", "book", "comic", "photo", "subtitle",
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
/** The subtitle languages a new rule asks for: the interface's. */
export const defaultSubLangs = () => ({ pt: "pt-BR", en: "en", es: "es" })[settings.lang] ?? "en";

export const isVideo = (name: string) => VIDEO.has(splitExt(name)[1].toLowerCase());
const SUBTITLE = new Set(["srt", "ass", "ssa", "sub", "idx", "vtt", "sup", "smi"]);
/** What the movie and TV show rules name: videos and their subtitles (as the Rust core). */
export const isScreen = (name: string) => isVideo(name) || SUBTITLE.has(splitExt(name)[1].toLowerCase());
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
const COMIC = new Set(["cbz", "cbr", "cb7", "cbt", "pdf", "epub", "mobi", "azw3", "azw"]);
export const isComic = (name: string) => COMIC.has(splitExt(name)[1].toLowerCase());

/** The photo rule's ready templates: names and folders, in each language. */
export const PHOTO_TEMPLATES = {
  pt: {
    names: ["{ano}-{mês}-{dia} {hora}.{minuto}.{segundo}", "{ano}{mês}{dia}_{hora}{minuto}{segundo}", "{ano}-{mês}-{dia} {hora}.{minuto}.{segundo} ({câmera})", "{ano}-{mês}-{dia} - {nome}"],
    folders: ["{ano}/{ano}-{mês}", "{ano}/{mês} - {nome do mês}", "{ano}/{ano}-{mês}-{dia}", "{ano}"],
  },
  en: {
    names: ["{year}-{month}-{day} {hour}.{minute}.{second}", "{year}{month}{day}_{hour}{minute}{second}", "{year}-{month}-{day} {hour}.{minute}.{second} ({camera})", "{year}-{month}-{day} - {name}"],
    folders: ["{year}/{year}-{month}", "{year}/{month} - {month name}", "{year}/{year}-{month}-{day}", "{year}"],
  },
  es: {
    names: ["{año}-{mes}-{día} {hora}.{minuto}.{segundo}", "{año}{mes}{día}_{hora}{minuto}{segundo}", "{año}-{mes}-{día} {hora}.{minuto}.{segundo} ({cámara})", "{año}-{mes}-{día} - {nombre}"],
    folders: ["{año}/{año}-{mes}", "{año}/{mes} - {nombre del mes}", "{año}/{año}-{mes}-{día}", "{año}"],
  },
} as const;

/** What the photo rule names: photos, raw files, phone videos and edits (as the Rust core). */
const SHOT = new Set([
  "jpg", "jpeg", "jpe", "heic", "heif", "avif", "png", "webp", "tif", "tiff", "gif",
  "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "srw", "x3f", "3fr", "erf", "kdc", "mrw", "rwl", "iiq", "raw",
  "mp4", "mov", "m4v", "3gp", "mts", "m2ts", "avi", "xmp", "aae",
]);
export const isShot = (name: string) => SHOT.has(splitExt(name)[1].toLowerCase());

/** Files the subtitle rule works on: subtitles, and MKV videos (their tracks). */
export const isSubtitleFile = (name: string) => /\.(srt|ass|ssa|vtt|sub|smi|sami)$/i.test(name);
export const isMatroska = (name: string) => /\.(mkv|webm|mk3d)$/i.test(name);
/** Videos whose details can be written inside (MKV and MP4). */
export const isTaggableVideo = (name: string) => /\.(mkv|webm|mk3d|mp4|m4v|mov)$/i.test(name);

/** Frame rates the subtitle rule retimes between: subtitle's → video's. */
export const FPS_PAIRS: [number, number][] = [
  [23.976, 25], [25, 23.976], [24, 25], [25, 24], [23.976, 24], [24, 23.976], [29.97, 25], [25, 29.97],
];

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
      return { type, allSpaces: false, extraSpaces: true, accents: false, digits: false, chars: "", leadingNumber: false, maxLength: 0, target: "name" };
    case "convert":
      return { type, mojibake: true, latin: false, web: "", target: "name" };
    case "removeRange":
      return { type, from: "text", fromPosition: 1, fromText: "", to: "text", toPosition: 1, toText: "", inclusive: true };
    case "charAt":
      return { type, position: 1, fromEnd: false, onlyIf: "", caseSensitive: false, replace: "" };
    case "extension":
      return { type, mode: "lower", value: "" };
    case "rearrange":
      return { type, separator: " - ", order: "", target: "name" };
    case "movie":
      return { type, template: MOVIE_TEMPLATE[settings.lang], colon: " - ", sidecar: false, fanart: false, folder: false, missing: true };
    case "series":
      return {
        type, template: SERIES_TEMPLATE[settings.lang], colon: " - ", show: "",
        folders: false, folderTemplate: SHOW_FOLDER[settings.lang], sidecar: false, fanart: false, missing: true, source: "tmdb",
      };
    case "music": {
      const [template, folderTemplate] = MUSIC_TEMPLATE[settings.lang];
      return { type, template, colon: " - ", discPrefix: true, fromName: false, folders: false, folderTemplate, cover: false, playlist: false, listen: true, writeTags: false, embedCover: true, coverSize: 1200, cleanTags: true, missing: true };
    }
    case "book": {
      const [template, folderTemplate] = BOOK_TEMPLATE[settings.lang];
      return { type, template, colon: " - ", folders: false, folderTemplate, cover: false, opf: false, writeEpub: false, missing: true, convertTo: "", original: "keep" };
    }
    case "photo": {
      const p = PHOTO_TEMPLATES[settings.lang];
      return { type, template: p.names[0], fromName: true, modified: false, shiftHours: 0, folders: false, folderTemplate: p.folders[0], setModified: false };
    }
    case "subtitle":
      return { type, toSrt: false, utf8: true, ads: false, hearing: false, shiftMs: 0, fpsFrom: 0, fpsTo: 0, extract: false, extractLangs: "", original: "keep" };
    case "comic": {
      const [template, folderTemplate] = COMIC_TEMPLATE[settings.lang];
      return { type, template, colon: " - ", series: "", source: "gcd", folders: false, folderTemplate, byFolder: false, writeInfo: false, cleanInfo: false, convert: false, original: "keep", drawPages: true, pageSize: 2400, missing: true, barcode: false, skipPublishers: "", ebook: "", device: "kindle-pw", crop: true, split: true };
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
/** The rules that look the files up in a catalogue: one at a time. */
export const LOOKUP_TYPES: RuleType[] = ["movie", "series", "music", "book", "comic"];

export const isLookup = (type: RuleType) => LOOKUP_TYPES.includes(type);

/** Steps with only their first lookup rule; the others are left out and
 *  returned apart (saved before they were one at a time). */
export function oneLookup(steps: Step[]): { steps: Step[]; dropped: RuleType[] } {
  let seen = false;
  const dropped: RuleType[] = [];
  const kept = steps.filter((s) => {
    if (!isLookup(s.rule.type)) return true;
    if (seen) {
      dropped.push(s.rule.type);
      return false;
    }
    seen = true;
    return true;
  });
  return { steps: kept, dropped };
}

export function newStep(rule: Rule, enabled = true): Step {
  const full = { ...newRule(rule.type), ...rule } as Rule;
  return { id: `s${Date.now().toString(36)}${counter++}`, enabled, rule: full };
}

/** Series that could hold issue `highest` first: one with fewer issues
 *  (when the catalogue says how many) is not the files' series. */
export function byFit<T extends { count: number }>(found: T[], highest: number): T[] {
  const fits = (v: T) => !v.count || v.count >= Math.floor(highest);
  return [...found.filter(fits), ...found.filter((v) => !fits(v))];
}

/** Catalogues of manga series, not of a publisher's issues: the file's
 *  volume and chapter stay. */
export function seriesOnly(source: string): boolean {
  return source === "mangadex" || source === "anilist" || source === "mangaupdates";
}
