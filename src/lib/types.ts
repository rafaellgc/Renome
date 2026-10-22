// Mirrors the types serialized by the Rust core (renome-core).

export type Target = "name" | "ext" | "full";
export type CaseMode = "lower" | "upper" | "sentence" | "title" | "titleSmart";
export type Anchor = "start" | "end" | "index" | "fromEnd";
export type Bound = "start" | "end" | "position" | "text";

export type Rule =
  | { type: "replace"; find: string; replace: string; caseSensitive: boolean; firstOnly: boolean; target: Target }
  | { type: "regex"; pattern: string; replace: string; caseSensitive: boolean; firstOnly: boolean; target: Target }
  | { type: "case"; mode: CaseMode; target: Target }
  | {
      type: "insert";
      source: "text" | "now" | "modified" | "taken" | "folder";
      text: string; dateFormat: string; fallback: boolean;
      at: Anchor; position: number;
    }
  | { type: "number"; template: string; start: number; step: number; digits: number; perFolder: boolean }
  | { type: "remove"; allSpaces: boolean; extraSpaces: boolean; accents: boolean; digits: boolean; chars: string; target: Target }
  | {
      type: "removeRange";
      from: Bound; fromPosition: number; fromText: string;
      to: Bound; toPosition: number; toText: string;
      inclusive: boolean;
    }
  | { type: "charAt"; position: number; fromEnd: boolean; onlyIf: string; caseSensitive: boolean; replace: string }
  | { type: "extension"; mode: "set" | "lower" | "upper" | "remove"; value: string }
  | { type: "rearrange"; separator: string; order: string; target: Target };

export type RuleType = Rule["type"];

export interface Step {
  id: string;
  enabled: boolean;
  rule: Rule;
}

export type Status = "unchanged" | "ok" | "conflict";
export type Problem = "empty" | "slash" | "tooLong" | "windowsChars" | "duplicate" | "exists";

export interface PreviewItem {
  path: string;
  dir: string;
  size: number;
  modified: number | null;
  isDir: boolean;
  original: string;
  newName: string;
  status: Status;
  problem: Problem | null;
  windowsUnsafe: boolean;
  manual: boolean;
}

export interface Preview {
  items: PreviewItem[];
  ruleErrors: { index: number; code: "invalidRegex" | "invalidDateFormat"; detail: string }[];
  filterError: string | null;
  total: number;
  changed: number;
  conflicts: number;
}

export interface ScanOptions {
  recursive: boolean;
  includeHidden: boolean;
  /** Adds folders themselves (to rename them) instead of their contents. */
  foldersAsItems: boolean;
}

export interface View {
  filter: string;
  sortByNewName: boolean;
  descending: boolean;
}

export type ErrorKind = "tempExists" | "targetExists" | "notFound" | "permissionDenied" | "io" | "restoreFailed" | "history";
export interface OpError { from: string; to: string; kind: ErrorKind; detail: string }

export interface ApplyReport {
  done: { from: string; to: string }[];
  errors: OpError[];
  historyId: string | null;
}

export interface HistoryEntry {
  id: string;
  created: string;
  count: number;
  undone: boolean;
  /** Undone so far, when an undo was partial. */
  reverted: number;
  /** Failed on the last undo attempt. */
  failed: number;
}

/** File list sort orders (done in Rust). */
export type SortKey = "name" | "ext" | "dir" | "modified" | "size";
/** Displayed order: a column, the new name or the manual order. */
export type ViewSort = SortKey | "newName" | "manual";
