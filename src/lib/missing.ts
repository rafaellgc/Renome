import { isVideo } from "./rules";
import type { ComicIssue, ComicMatch, ComicState, ComicVolume, EpisodeMatch, PreviewItem, Show } from "./types";

/** An aired episode or a published issue that no file in the list
 *  holds: a faded row that only informs, never selected, renamed or
 *  counted. */
export interface Ghost {
  ghost: true;
  key: string;
  kind: "episode" | "issue";
  /** The show or the series, with its year. */
  group: string;
  /** An episode's season and number; an issue's place in its series. */
  season: number;
  number: number;
  /** As in the file names: S01E16, #7. */
  code: string;
  title: string;
  /** Air date or cover date, YYYY-MM-DD. */
  date: string;
  /** The file it sits next to, and on which side. */
  anchor: string;
  after: boolean;
}

export type Row = PreviewItem | Ghost;

/** Today as YYYY-MM-DD, in local time (TMDB's air dates have no hour). */
export function today(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** Where a missing `n` goes among the files holding `from`..`to`: before
 *  the nearest later file, else after the nearest earlier one (swapped
 *  when the list runs backwards). */
function place(n: number, files: { path: string; from: number; to: number }[], descending: boolean) {
  let later: { path: string; n: number } | null = null;
  let earlier: { path: string; n: number } | null = null;
  for (const f of files) {
    if (f.from > n && (!later || f.from < later.n)) later = { path: f.path, n: f.from };
    if (f.to < n && (!earlier || f.to > earlier.n)) earlier = { path: f.path, n: f.to };
  }
  return { anchor: (later ?? earlier!).path, after: !later !== descending };
}

const named = (name: string, year: number | null | undefined) => (year ? `${name} (${year})` : name);

/** The aired episodes missing from the seasons the listed videos cover
 *  (seasons with no video at all are left out: a whole show would flood
 *  the list). */
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
    const spans = files.map((f) => ({ path: f.path, from: Math.min(...f.episodes), to: Math.max(...f.episodes) }));
    for (const e of show.episodes) {
      if (e.season !== season || have.has(e.number) || !e.airDate || e.airDate > day) continue;
      out.push({
        ghost: true,
        key: `ghost:${show.id}:${season}:${e.number}`,
        kind: "episode",
        group: named(show.name, show.year),
        season,
        number: e.number,
        code: `S${String(season).padStart(2, "0")}E${String(e.number).padStart(2, "0")}`,
        title: e.name,
        date: e.airDate,
        ...place(e.number, spans, descending),
      });
    }
  }
  return out;
}

/** A series' issues in reading order: by number, then as listed. */
function inOrder(issues: ComicIssue[]): ComicIssue[] {
  const n = (i: ComicIssue) => {
    const x = Number.parseFloat(i.number);
    return Number.isNaN(x) ? Infinity : x;
  };
  return issues.map((i, k) => ({ i, k })).sort((a, b) => n(a.i) - n(b.i) || a.k - b.k).map((x) => x.i);
}

/** The numbers missing between a series' own files, from 1 to the
 *  highest they have, for a catalogue without a list of issues
 *  (MangaDex): by volume when every file has one, else by issue
 *  number, else by chapter. */
function gaps(name: string, vkey: string, files: { path: string; own: ComicMatch }[], descending: boolean): Ghost[] {
  const by: [(o: ComicMatch) => number, (n: number) => string][] = [
    [(o) => o.volume ?? NaN, (n) => `v${String(n).padStart(2, "0")}`],
    [(o) => (o.number ? Number.parseFloat(o.number) : NaN), (n) => `#${n}`],
    [(o) => (o.chapter ? Number.parseFloat(o.chapter) : NaN), (n) => `c${String(n).padStart(3, "0")}`],
  ];
  const kind = by.find(([of]) => files.every((f) => !Number.isNaN(of(f.own))));
  if (!kind) return [];
  const [of, code] = kind;
  const spans = files.map((f) => ({ path: f.path, from: of(f.own), to: of(f.own) }));
  const have = new Set(spans.map((s) => s.from));
  const out: Ghost[] = [];
  for (let n = 1; n < Math.max(...have); n++) {
    if (have.has(n)) continue;
    out.push({ ghost: true, key: `ghost:${vkey}:${n}`, kind: "issue", group: name, season: 0, number: n, code: code(n), title: "", date: "", ...place(n, spans, descending) });
  }
  return out;
}

/** The issues missing from each series some listed comic has (issues
 *  with a cover date still to come are left out); for a series without
 *  a list of issues, the gaps between its files. */
export function missingIssues(
  volumes: Record<string, { volume: ComicVolume; issues: ComicIssue[] }>,
  comics: Record<string, ComicState>,
  items: PreviewItem[],
  descending: boolean,
  day: string,
): Ghost[] {
  const series = new Map<string, { order: ComicIssue[]; files: { path: string; from: number; to: number }[] }>();
  const unlisted = new Map<string, { path: string; own: ComicMatch }[]>();
  for (const i of items) {
    const c = comics[i.path];
    if (c?.choice === "" && volumes[c.volumeKey] && !volumes[c.volumeKey].issues.length) {
      const list = unlisted.get(c.volumeKey);
      if (list) list.push({ path: i.path, own: c.own });
      else unlisted.set(c.volumeKey, [{ path: i.path, own: c.own }]);
      continue;
    }
    const v = c?.choice && volumes[c.volumeKey];
    if (!v || !v.issues.length) continue;
    let s = series.get(c.volumeKey);
    if (!s) series.set(c.volumeKey, (s = { order: inOrder(v.issues), files: [] }));
    const at = s.order.findIndex((x) => x.id === c.choice);
    if (at >= 0) s.files.push({ path: i.path, from: at, to: at });
  }
  const out: Ghost[] = [];
  for (const [vkey, files] of unlisted) {
    const { volume } = volumes[vkey];
    out.push(...gaps(named(volume.name, volume.startYear), vkey, files, descending));
  }
  for (const [vkey, { order, files }] of series) {
    if (!files.length) continue;
    const { volume } = volumes[vkey];
    const have = new Set(files.map((f) => f.from));
    order.forEach((issue, at) => {
      if (have.has(at) || (issue.coverDate && issue.coverDate > day)) return;
      out.push({
        ghost: true,
        key: `ghost:${vkey}:${issue.id}`,
        kind: "issue",
        group: named(volume.name, volume.startYear),
        season: 0,
        number: at,
        code: /^\d/.test(issue.number) ? `#${issue.number}` : issue.number,
        title: issue.name,
        date: issue.coverDate,
        ...place(at, files, descending),
      });
    });
  }
  return out;
}

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

/** A season folder's name (as the Rust core): `Season 2`, `Temporada 02`, `S2`, `Specials`. */
const SEASON_DIR = /\b(?:season|temporada|temp|saison|staffel)[ ._-]*\d{1,3}\b|^[st]\d{1,3}$|^(?:specials?|especiais|especiales|extras)$/i;

/** The folder the missing items' files share, above any season folder:
 *  the show's or series' folder, where their list is saved by default. */
export function missingFolder(ghosts: Ghost[], dirOf: (path: string) => string | undefined): string | null {
  const dirs = [...new Set(ghosts.map((g) => dirOf(g.anchor)).filter((d) => d !== undefined))];
  if (!dirs.length) return null;
  const sep = dirs[0].includes("/") ? "/" : "\\";
  let common = dirs[0].split(sep);
  for (const d of dirs.slice(1)) {
    const parts = d.split(sep);
    let n = 0;
    while (n < common.length && n < parts.length && common[n] === parts[n]) n++;
    common = common.slice(0, n);
  }
  if (common.length > 1 && SEASON_DIR.test(common[common.length - 1].trim())) common.pop();
  const dir = common.join(sep);
  return dir || (common.length ? sep : null);
}
