import { isVideo } from "./rules";
import type { EpisodeMatch, PreviewItem, Show } from "./types";

/** An aired episode that no video in the list holds: a faded row that
 *  only informs, never selected, renamed or counted. */
export interface Ghost {
  ghost: true;
  key: string;
  /** The show's name, with its year. */
  show: string;
  season: number;
  number: number;
  title: string;
  airDate: string;
  /** The video it sits next to, and on which side. */
  anchor: string;
  after: boolean;
}

export type Row = PreviewItem | Ghost;

/** Today as YYYY-MM-DD, in local time (TMDB's air dates have no hour). */
export function today(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** The aired episodes missing from the seasons the listed videos cover
 *  (seasons with no video at all are left out: a whole show would flood
 *  the list). Each one goes next to the video holding the nearest later
 *  episode, or after the nearest earlier one. */
export function missingEpisodes(
  shows: Record<number, Show>,
  matches: Record<string, EpisodeMatch>,
  items: PreviewItem[],
  descending: boolean,
  day: string,
): Ghost[] {
  const seasons = new Map<string, { show: Show; season: number; files: { path: string; episodes: number[] }[] }>();
  for (const i of items) {
    const m = matches[i.path];
    const show = m && shows[m.id];
    if (!show || !m.episodes.length || !isVideo(i.original)) continue;
    const k = `${m.id}:${m.season}`;
    let s = seasons.get(k);
    if (!s) seasons.set(k, (s = { show, season: m.season, files: [] }));
    s.files.push({ path: i.path, episodes: m.episodes });
  }
  const out: Ghost[] = [];
  for (const { show, season, files } of seasons.values()) {
    const have = new Set(files.flatMap((f) => f.episodes));
    for (const e of show.episodes) {
      if (e.season !== season || have.has(e.number) || !e.airDate || e.airDate > day) continue;
      let later: { path: string; n: number } | null = null;
      let earlier: { path: string; n: number } | null = null;
      for (const f of files) {
        const first = Math.min(...f.episodes);
        const last = Math.max(...f.episodes);
        if (first > e.number && (!later || first < later.n)) later = { path: f.path, n: first };
        if (last < e.number && (!earlier || last > earlier.n)) earlier = { path: f.path, n: last };
      }
      const anchor = later ?? earlier!;
      out.push({
        ghost: true,
        key: `ghost:${show.id}:${season}:${e.number}`,
        show: show.year ? `${show.name} (${show.year})` : show.name,
        season,
        number: e.number,
        title: e.name,
        airDate: e.airDate,
        anchor: anchor.path,
        // Before the later episode, after the earlier one; flipped when
        // the list runs backwards.
        after: !later !== descending,
      });
    }
  }
  return out;
}

/** The list with each missing episode beside its video. */
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

/** Season and episode as in the file names, S01E16. */
export const code = (g: Ghost) => `S${String(g.season).padStart(2, "0")}E${String(g.number).padStart(2, "0")}`;
