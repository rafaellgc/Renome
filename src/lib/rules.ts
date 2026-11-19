import { settings } from "./settings.svelte";
import type { Rule, RuleType, Step } from "./types";

export const RULE_TYPES: RuleType[] = [
  "replace", "regex", "rearrange", "list", "case", "insert", "number", "remove", "convert", "removeRange", "charAt", "extension", "movie", "series", "music", "book", "comic", "photo", "subtitle",
];

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
  if (preset === "kavita") {
    const [template, folderTemplate] = KAVITA_TEMPLATE[settings.lang];
    return { template, folderTemplate, folders: true };
  }
  // Renome's own: the comic rule's defaults.
  const r = newRule("comic") as Extract<Rule, { type: "comic" }>;
  return { template: r.template, folderTemplate: r.folderTemplate, folders: true };
}

/** The preset a rule's templates match, if any. */
export function comicPresetOf(r: { template: string; folderTemplate: string; folders: boolean }): ComicPreset | "" {
  for (const p of ["renome", "kavita"] as const) {
    const t = comicPreset(p);
    if (r.folders && r.template === t.template && r.folderTemplate === t.folderTemplate) return p;
  }
  return "";
}



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



/** Frame rates the subtitle rule retimes between: subtitle's → video's. */
export const FPS_PAIRS: [number, number][] = [
  [23.976, 25], [25, 23.976], [24, 25], [25, 24], [23.976, 24], [24, 23.976], [29.97, 25], [25, 29.97],
];

/** A list rule's names: its lines without blank ones (as the Rust core). */
export const listNames = (text: string) => text.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);

/** Each rule new at its defaults (in the interface's language) and the
 *  rules that name a whole kind of file: the Rust catalog's, which the
 *  command line uses too; loaded at start and when the language changes. */
let TABLE: { defaults: Record<string, Rule>; kinds: RuleType[] } = { defaults: {}, kinds: [] };
export function setRuleTable(table: typeof TABLE) {
  TABLE = table;
}

export function newRule(type: RuleType): Rule {
  return structuredClone(TABLE.defaults[type] ?? ({ type } as Rule));
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
/** The rules that name a whole kind of file (one at a time). */
export const isKindRule = (type: RuleType) => TABLE.kinds.includes(type);

/** Why a rule cannot join the rules there (the Rust catalog decides). */
export type Blocked = { kind: RuleType } | { twice: true } | { clash: RuleType };

export function newStep(rule: Rule, enabled = true, only = ""): Step {
  const base = newRule(rule.type) as Record<string, unknown>;
  // Counting a video and its subtitles as one is new rules' default; a
  // rule saved before it counts them apart (as the core reads it).
  if (!("pairs" in rule)) delete base.pairs;
  const full = { ...base, ...rule } as Rule;
  return { id: `s${Date.now().toString(36)}${counter++}`, enabled, rule: full, ...(only ? { only } : {}) };
}

/** Catalogues of manga series, not of a publisher's issues: the file's
 *  volume and chapter stay. */
export function seriesOnly(source: string): boolean {
  return source === "mangadex" || source === "anilist" || source === "mangaupdates";
}
