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
  /** Every optional column of the list, in its order. */
  columns: DetailColumn[];
  /** The comic catalogues of whole series (a file's volume and chapter stay). */
  seriesOnly: string[];
  /** The details windows' fields (labels apart), by rule. */
  editFields: Record<"music" | "book" | "comic" | "photo", Omit<EditField, "label">[]>;
  options: {
    /** Every setting at its default. */
    defaults: Record<string, unknown>;
    /** Settings kept in the system's password store, not in the file. */
    secrets: string[];
    tmdbLangs: string[];
    /** The setting holding each comic catalogue's key. */
    comicKeys: Record<string, string>;
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
  seriesOnly: [],
  editFields: { music: [], book: [], comic: [], photo: [] },
  options: { defaults: {}, secrets: [], tmdbLangs: [], comicKeys: {} },
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
  for (const p of ["renome", "kavita"] as const) {
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

/** The rules that name a whole kind of file (one at a time). */
export const isKindRule = (type: RuleType) => TABLE.kinds.includes(type);

/** Why a rule cannot join the rules there (the Rust catalog decides). */
export type Blocked = { kind: RuleType } | { twice: true } | { clash: RuleType };
