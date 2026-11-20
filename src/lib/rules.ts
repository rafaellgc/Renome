import type { DetailColumn, EditField, Rule, RuleType, Step } from "./types";

/** Everything rules, their menus and the options are built from, in the
 *  interface's language: the Rust catalog's (`catalog::table`,
 *  `options::table`), which the command line uses too; loaded at start
 *  and when the language changes. */
export type RuleTable = {
  /** Each rule new, at its defaults. */
  defaults: Record<string, Rule>;
  /** The rules that name a whole kind of file (one at a time). */
  kinds: RuleType[];
  /** The rules that look files up in a catalogue. */
  lookups: RuleType[];
  /** The "+ Rule" menu's groups: by kind of file, then on the name's text. */
  media: RuleType[];
  text: RuleType[];
  comicPresets: Record<ComicPreset, { template: string; folderTemplate: string }>;
  photoTemplates: { names: string[]; folders: string[] };
  /** Frame rates the subtitle rule retimes between: subtitle's → video's. */
  fpsPairs: [number, number][];
  /** Every optional column of the list, in its order, and each one's
   *  title (its key in the texts, as the CSV's). */
  columns: DetailColumn[];
  columnLabels: Record<DetailColumn, string>;
  /** The bounds the core takes for each rule's whole-number options
   *  (null: none), and the numbers its number menus offer. */
  limits: Record<string, Record<string, [number | null, number | null]>>;
  choices: Record<string, Record<string, number[]>>;
  /** The choices of each rule's word options, with their labels' keys. */
  menus: Record<string, Record<string, { value: string; label: string }[]>>;
  /** Each rule's options' labels and hints (keys of the texts). */
  labels: Record<string, Record<string, [string, string | null]>>;
  /** The comic catalogues of whole series (a file's volume and chapter stay). */
  seriesOnly: string[];
  /** The details windows' fields (labels apart), by rule. */
  editFields: Record<"music" | "book" | "comic" | "photo", Omit<EditField, "label">[]>;
  options: {
    /** Every setting at its default. */
    defaults: Record<string, unknown>;
    /** Settings kept in the system's password store, not in the file. */
    secrets: string[];
    /** Each option's menu: words with their labels' keys, or numbers. */
    menus: Record<string, { value: string; label: string }[] | number[]>;
    /** The kinds of media server, with their names. */
    serverKinds: [string, string][];
    /** The setting holding each comic catalogue's key. */
    comicKeys: Record<string, string>;
    /** The errors a key or account in the options fixes. */
    keyErrors: string[];
  };
};

export let TABLE: RuleTable = {
  defaults: {},
  kinds: [],
  lookups: [],
  media: [],
  text: [],
  comicPresets: { renome: { template: "", folderTemplate: "" }, kavita: { template: "", folderTemplate: "" } },
  photoTemplates: { names: [], folders: [] },
  fpsPairs: [],
  columns: [],
  columnLabels: {} as Record<DetailColumn, string>,
  limits: {},
  choices: {},
  menus: {},
  labels: {},
  seriesOnly: [],
  editFields: { music: [], book: [], comic: [], photo: [] },
  options: { defaults: {}, secrets: [], menus: {}, serverKinds: [], comicKeys: {}, keyErrors: [] },
};
export function setRuleTable(table: RuleTable) {
  TABLE = table;
}

export type ComicPreset = "renome" | "kavita";

/** A ready-made comic template and folder template, in the current
 *  language; both servers want each series in its own folder. */
export function comicPreset(preset: ComicPreset): { template: string; folderTemplate: string; folders: true } {
  return { ...TABLE.comicPresets[preset], folders: true };
}

/** The preset a rule's templates match, if any. */
export function comicPresetOf(r: { template: string; folderTemplate: string; folders: boolean }): ComicPreset | "" {
  for (const p of Object.keys(TABLE.comicPresets) as ComicPreset[]) {
    const t = TABLE.comicPresets[p];
    if (r.folders && r.template === t.template && r.folderTemplate === t.folderTemplate) return p;
  }
  return "";
}

export function newRule(type: RuleType): Rule {
  return structuredClone(TABLE.defaults[type] ?? ({ type } as Rule));
}

let counter = 0;
/** A new step for a new rule (saved ones come whole from the Rust
 *  catalog: `api.completeSteps`). */
export function newStep(rule: Rule): Step {
  return { id: `s${Date.now().toString(36)}${counter++}`, enabled: true, rule };
}

/** A whole-number option's bounds, as a field's `min` and `max`. */
export function limitsOf(type: RuleType, name: string): { min?: number; max?: number } {
  const [min, max] = TABLE.limits[type]?.[name] ?? [null, null];
  return { min: min ?? undefined, max: max ?? undefined };
}

/** The numbers a rule's number menu offers. */
export const choicesOf = (type: RuleType, name: string): number[] => TABLE.choices[type]?.[name] ?? [];

/** A rule's menu of words: each value with its label's key. */
export const menuOf = (type: RuleType, name: string) => TABLE.menus[type]?.[name] ?? [];

/** The rules that name a whole kind of file (one at a time). */
export const isKindRule = (type: RuleType) => TABLE.kinds.includes(type);

/** Whether an error is fixed by a key or account in the options (the
 *  Rust table's list). */
export const needsKey = (e: unknown) => TABLE.options.keyErrors.some((k) => String(e).startsWith(k));

/** Why a rule cannot join the rules there (the Rust catalog decides). */
export type Blocked = { kind: RuleType } | { twice: true } | { clash: RuleType };
