// What the main page's modules (books, comics, music…) share with it.

import { splitExt } from "$lib/rules";
import type { EditField, MovieStatus, PreviewItem, Step } from "$lib/types";

/** What a module reads from the main page and tells it. */
export interface PageCtx {
  /** The rows of the list, as the preview shows them. */
  readonly items: PreviewItem[];
  /** The paths chosen in the list. */
  readonly selected: Set<string>;
  readonly steps: Step[];
  /** The name a row is looked up by. */
  readName(i: PreviewItem): string;
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

/** A path's extension, lower case. */
export const extOf = (p: string) => splitExt(p.split(/[\\/]/).pop() ?? p)[1].toLowerCase();
export const isEpub = (p: string) => ["epub", "kepub"].includes(extOf(p));
/** Books whose details can be written inside: EPUB, FB2 and Kindle's. */
export const isWritableBook = (p: string) => ["epub", "kepub", "fb2", "mobi", "azw3", "azw", "prc"].includes(extOf(p));

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
