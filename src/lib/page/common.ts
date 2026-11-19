// What the main page's modules (books, comics, music…) share with it.

import { api } from "$lib/api";
import type { EditField, MovieStatus, PreviewItem, Step } from "$lib/types";
import { TABLE } from "$lib/rules";
import { t, type Key } from "$lib/i18n";

/** What a module reads from the main page and tells it. */
export interface PageCtx {
  /** The rows of the list, as the preview shows them. */
  readonly items: PreviewItem[];
  /** The paths chosen in the list. */
  readonly selected: Set<string>;
  readonly steps: Step[];
  /** A short message under the list. */
  say(text: string): void;
  /** Bumped by Stop: a search loop that sees a newer number stops. */
  readonly stopRun: number;
  /** Opens the options (a key or an account is missing). */
  openOptions(): void;
}

/** How many of the rows were found, are to be chosen or have nothing. */
export function counts(state: Record<string, { status: MovieStatus }>, items: PreviewItem[]) {
  let found = 0, choose = 0, none = 0;
  for (const i of items) {
    const st = state[i.path]?.status;
    if (st === "auto" || st === "chosen") found++;
    else if (st === "choose") choose++;
    else if (st === "none") none++;
  }
  return { found, choose, none };
}

/** The first enabled rule of a type. */
export const ruleOf = (steps: Step[], type: string) => steps.find((s) => s.enabled && s.rule.type === type);

/** A map with its keys taken through `follow` (files renamed). */
export function moved<T>(m: Record<string, T>, follow: (p: string) => string): Record<string, T> {
  return Object.fromEntries(Object.entries(m).map(([p, v]) => [follow(p), v]));
}

/** A map without the paths taken out of the list. */
export function without<T>(m: Record<string, T>, paths: Set<string>): Record<string, T> {
  return Object.fromEntries(Object.entries(m).filter(([p]) => !paths.has(p)));
}

/** Values as the details window shows them (nothing: empty). */
export function asText(m: object, keys: EditField[]): Record<string, string> {
  return Object.fromEntries(
    keys.map((f) => {
      const v = (m as Record<string, unknown>)[f.key];
      return [f.key, v == null || v === 0 ? "" : String(v)];
    }),
  );
}

/** A details window's fields (the Rust table's), with their labels:
 *  `songEditTitle`, `bookEditAuthors`… */
export function editFields(kind: "music" | "book" | "comic" | "photo"): EditField[] {
  const prefix = { music: "song", book: "book", comic: "comic", photo: "photo" }[kind];
  return TABLE.editFields[kind].map((f) => ({ ...f, label: t(`${prefix}Edit${f.key[0].toUpperCase()}${f.key.slice(1)}` as Key) }));
}

/** The user's pick for a file goes to the undecided files of its group too
 *  (the shared rule: `flow::same_answer`); the paths that take it. */
export function takers(path: string, state: Record<string, { group?: string; status: MovieStatus }>) {
  const files = Object.entries(state).map(([p, s]): [string, string, string] => [p, s.group ?? p, s.status]);
  return api.sameAnswer(path, files);
}
