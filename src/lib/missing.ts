import type { PreviewItem } from "./types";

/** An aired episode, a published issue, a track of an album, a released
 *  movie of a collection or a volume of a book series that no file in
 *  the list holds (worked out by the shared flow, `flow::missing`): a
 *  faded row that only informs, never selected, renamed or counted. */
export interface Ghost {
  ghost: true;
  key: string;
  kind: "episode" | "issue" | "track" | "movie" | "book";
  /** The show, series, album or collection, with its year. */
  group: string;
  /** An episode's season, a track's disc; 0 for the others. */
  season: number;
  /** An episode's number; an issue's, track's or movie's place in its
   *  series, album or collection; a book's volume. */
  number: number;
  /** A track's number on its disc. */
  position?: number;
  /** As in the file names: S01E16, #7, 1-05, 1999, 3. */
  code: string;
  title: string;
  /** Air date or cover date, YYYY-MM-DD. */
  date: string;
  /** The file it sits next to, and on which side. */
  anchor: string;
  after: boolean;
}

export type Row = PreviewItem | Ghost;

/** The list with each missing item beside its file. */
export function interleave(items: PreviewItem[], ghosts: Ghost[], descending: boolean): Row[] {
  if (!ghosts.length) return items;
  const before = new Map<string, Ghost[]>();
  const after = new Map<string, Ghost[]>();
  for (const g of ghosts) {
    const side = g.after ? after : before;
    const list = side.get(g.anchor);
    if (list) list.push(g);
    else side.set(g.anchor, [g]);
  }
  const order = (list: Ghost[] = []) => list.sort((a, b) => (descending ? b.number - a.number : a.number - b.number));
  const rows: Row[] = [];
  for (const i of items) rows.push(...order(before.get(i.path)), i, ...order(after.get(i.path)));
  return rows;
}
