// The comic rule's side of the main page: the series and issues found in
// the catalogue chosen (GCD, Metron, Comic Vine or a manga one), the
// choices, the details typed in and what converting and writing need.

import { api } from "$lib/api";
import { confirm, notify } from "$lib/dialogs.svelte";
import { t, tError } from "$lib/i18n";
import { byFit, isComic, seriesOnly, splitExt } from "$lib/rules";
import { comicKey, settings } from "$lib/settings.svelte";
import type { ComicIssue, ComicMatch, ComicSource, ComicState, ComicVolume, EditField, Rule, BookTarget } from "$lib/types";
import { counts, extOf, moved, ruleOf, without, type PageCtx } from "./common";

/** What a file's series has after the catalogue's (as the Rust core's `story`). */
function story(own: string, series: string): string {
  const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
  const name = series.trim();
  if (!name || own.length === name.length || !fold(own).startsWith(fold(name))) return "";
  const rest = own.slice(name.length);
  return /^[: \-–]/.test(rest) ? rest.replace(/^[\s:\-–—,.#]+|[\s\-–—:,.#]+$/g, "") : "";
}

function withIssue(own: ComicMatch, volume: ComicVolume, issue: ComicIssue | undefined): ComicMatch {
  const year = Number.parseInt((issue?.coverDate ?? "").slice(0, 4), 10);
  return {
    series: volume.name,
    seriesYear: volume.startYear ?? own.seriesYear,
    // A book found by its volume (`v03`, a manga's) stays a volume, and a
    // one-shot gets no number.
    number: own.number && issue?.number ? issue.number : own.number,
    volume: own.volume,
    chapter: own.chapter,
    // Many issues have no title there: the file's stays, or what its
    // series had after the catalogue's (`Series: Story`).
    title: issue?.name.trim() ? issue.name : own.title || story(own.series, volume.name),
    year: Number.isNaN(year) ? own.year : year,
    count: volume.count > 0 ? volume.count : own.count,
    publisher: volume.publisher || own.publisher,
    writer: own.writer || volume.writer,
    artist: own.artist,
    id: issue ? issue.id : volume.id,
    extra: withCoverDate(own.extra ?? {}, issue?.coverDate ?? ""),
  };
}

/** The catalogue's cover date over the file's (its parts come from it). */
function withCoverDate(extra: Record<string, string>, date: string): Record<string, string> {
  if (!date) return extra;
  const out = Object.fromEntries(Object.entries(extra).filter(([k]) => !["date", "month", "day"].includes(k) && !k.startsWith("month name")));
  out.date = date;
  return out;
}

/** A CBZ whose ComicInfo.xml the comic rule takes out: with "clear", the
 *  ones it does not write (without "write", every one; with it, none:
 *  an unmatched comic's details are what name it). */
export function clearsInfo(r: Rule, junk: boolean) {
  return r.type === "comic" && !!r.cleanInfo && junk && !r.writeInfo;
}

const EMPTY: ComicMatch = { series: "", seriesYear: null, number: "", volume: null, chapter: "", title: "", year: null, count: null, publisher: "", writer: "", artist: "", id: "" };

/** What the series picker asks, while open. */
export type ComicPick = { source: ComicSource; skip: string; query: string; files: number; highest: number; candidates: ComicVolume[]; path?: string; comic?: ComicMatch; resolve: (v: ComicVolume | null) => void };

export class Comics {
  #ctx: PageCtx;
  /** The series and issue per file. */
  state = $state<Record<string, ComicState>>({});
  /** Series fetched with their issues, by `source:id`. */
  volumes = $state<Record<string, { volume: ComicVolume; issues: ComicIssue[] }>>({});
  /** The series picked for each search text (typed, or the files' series). */
  picked = $state<Record<string, ComicVolume>>({});
  /** Files in the review window; null when closed. */
  review = $state<string[] | null>(null);
  /** The series picker, while open. */
  pick = $state<ComicPick | null>(null);
  /** Whether this session was told that the GCD works better with an account. */
  #gcdWarned = false;
  progress = $state<string | null>(null);
  /** Comics whose details were typed in (over what the catalogue said). */
  edits = $state<Record<string, ComicMatch>>({});
  /** The details window, while open. */
  edit = $state<{ paths: string[]; comics: ComicMatch[] } | null>(null);

  constructor(ctx: PageCtx) {
    this.#ctx = ctx;
  }

  rule = $derived.by(() => ruleOf(this.#ctx.steps, "comic"));
  /** Each comic's details: its issue's (or its series'), or typed in. */
  matches = $derived.by(() => ({
    ...Object.fromEntries(
      Object.entries(this.state).flatMap(([p, c]): [string, ComicMatch][] => {
        const v = this.volumes[c.volumeKey];
        if (!v || c.choice == null) return [];
        // "": the series alone (MangaDex).
        const issue = c.choice === "" ? undefined : v.issues.find((i) => i.id === c.choice);
        return c.choice === "" || issue ? [[p, withIssue(c.own, v.volume, issue)]] : [];
      }),
    ),
    ...this.edits,
  }));
  /** The comics among the rows chosen in the list. */
  selected = $derived.by(() => this.#ctx.items.filter((i) => this.#ctx.selected.has(i.path) && (i.isDir ? i.convert : isComic(i.original))));
  counts = $derived.by(() => counts(this.state, this.#ctx.items));
  /** Comics whose names are already right but that get converted to CBZ
   *  (a RAR named `.cbz`), or whose ComicInfo.xml says something else. */
  infoOnly = $derived.by(() => {
    const r = this.rule?.rule;
    if (r?.type !== "comic") return [];
    return this.#ctx.items.filter(
      (i) =>
        i.status === "unchanged" &&
        ((r.convert && i.convert) ||
          (extOf(i.original) === "cbz" && ((r.writeInfo && (i.retag || (r.cleanInfo && i.junk)) && this.matches[i.path]) || clearsInfo(r, i.junk)))),
    );
  });
  /** The details window's fields. */
  fields = $derived<EditField[]>([
    { key: "series", label: t("comicEditSeries"), wide: true },
    { key: "seriesYear", label: t("comicEditSeriesYear"), numeric: true },
    { key: "volume", label: t("comicEditVolume"), numeric: true },
    { key: "number", label: t("comicEditNumber") },
    { key: "title", label: t("comicEditTitle"), wide: true },
    { key: "year", label: t("comicEditYear"), numeric: true },
    { key: "publisher", label: t("comicEditPublisher") },
    { key: "writer", label: t("comicEditWriter") },
    { key: "artist", label: t("comicEditArtist") },
  ]);

  /** What converting a comic to CBZ and writing its ComicInfo.xml need: the
   *  original's fate, its match and its page in the catalogue. */
  insideItem(key: string, path: string, convert: boolean, details?: boolean) {
    const r = this.rule?.rule;
    /** The issue's details go along (to write them, or for an e-book). */
    const want = details ?? (r?.type === "comic" && !!r.writeInfo);
    const original = r?.type === "comic" && r.convert && convert ? (r.original ?? "keep") : undefined;
    const pdf = r?.type === "comic" ? { draw: r.drawPages !== false, size: r.pageSize ?? 2400, info: !!r.writeInfo } : undefined;
    const fresh = r?.type === "comic" && !!r.cleanInfo;
    // Typed in: no page in a catalogue.
    if (r?.type === "comic" && want && this.edits[key]) return { path, comic: this.edits[key], web: "", manga: false, convert: original, pdf, fresh };
    if (!(r?.type === "comic" && want && this.matches[key] && this.state[key])) return { path, convert: original, pdf, fresh };
    const c = this.state[key];
    const [source, volume] = [c.volumeKey.slice(0, c.volumeKey.indexOf(":")), c.volumeKey.slice(c.volumeKey.indexOf(":") + 1)];
    const issue = c.choice || "";
    const web =
      source === "comicvine"
        ? issue ? `https://comicvine.gamespot.com/issue/4000-${issue}/` : `https://comicvine.gamespot.com/volume/4050-${volume}/`
        : source === "gcd"
          ? issue ? `https://www.comics.org/issue/${issue}/` : `https://www.comics.org/series/${volume}/`
          : source === "mangadex"
            ? `https://mangadex.org/title/${volume}`
            : source === "anilist"
              ? `https://anilist.co/manga/${volume}`
              : source === "mangaupdates" && /^\d+$/.test(volume)
                ? `https://www.mangaupdates.com/series/${BigInt(volume).toString(36)}`
                : "";
    return { path, comic: this.matches[key], web, manga: seriesOnly(source), convert: original, pdf, fresh };
  }
  /** What making a comic an e-book needs: the format, the reader, the
   *  original's fate and, when it has some, its details. */
  ebookItem(key: string, path: string, to: BookTarget) {
    const r = this.rule?.rule;
    if (r?.type !== "comic") return { path };
    const base = this.insideItem(key, path, false, true);
    return {
      ...base,
      fresh: false,
      comicTo: to,
      ebook: { device: r.device ?? "kindle-pw", crop: r.crop ?? true, split: r.split ?? true },
      convert: r.original ?? "keep",
    };
  }


  /** The chosen catalogue (GCD, Metron, Comic Vine, a manga one) for the
   *  visible comics: on Metron, first each file's barcode (recorded in it,
   *  or read in its pages when asked); then one series per group of files
   *  (the series typed, else each series the files say), picked by itself
   *  when only one has that name (and year), else asked; then each file's
   *  issue by its number. */
  async search(rule: Extract<Rule, { type: "comic" }>) {
    const source = rule.source ?? "gcd";
    const skip = rule.skipPublishers?.trim() ?? "";
    const key = comicKey(source);
    if ((source === "comicvine" || source === "metron") && !key) {
      const ok = await confirm({ title: t("rule.comic"), text: t(source === "metron" ? "mtNeedsKey" : "cvNeedsKey"), ok: t("options"), cancel: t("cancel") });
      if (ok) this.#ctx.openOptions();
      return;
    }
    // Without an account the GCD takes few searches: said once per session.
    if (source === "gcd" && !this.#gcdWarned && !(settings.gcdEmail.trim() && settings.gcdPassword)) {
      this.#gcdWarned = true;
      const ok = await confirm({ title: t("rule.comic"), text: t("gcdNoAccount"), ok: t("options"), cancel: t("gcdAnonymous") });
      if (ok) {
        this.#ctx.openOptions();
        return;
      }
    }
    // A folder of pictures too, when it becomes a CBZ.
    const files = this.#ctx.items.filter((i) => (i.isDir ? i.convert : isComic(i.original)));
    if (!files.length) {
      this.#ctx.say(t("comicNoFiles"));
      return;
    }
    this.progress = t("comicReading");
    const lang = settings.lang === "pt" ? "pt-br" : settings.lang;
    try {
      const owns = await api.comicGuesses(
        files.map((f) => f.path),
        files.map((f) => (f.lookupName ? (f.isDir ? f.lookupName : splitExt(f.lookupName)[0]) : null)),
        !!rule.byFolder,
      );
      const run = this.#ctx.stopRun;
      // On Metron, a barcode tells the issue itself.
      const coded = new Set<string>();
      if (source === "metron") {
        let codes = owns.map((o) => o?.extra?.gtin?.replace(/\D/g, "") || null);
        if (rule.barcode && codes.some((c) => !c)) {
          this.progress = t("comicBarcodeReading");
          const unread = files.flatMap((f, i) => (!codes[i] && owns[i] ? [i] : []));
          const read = await api.comicBarcodes(unread.map((i) => files[i].path));
          if (this.#ctx.stopRun !== run) return;
          unread.forEach((i, n) => {
            const code = read[n];
            const own = owns[i];
            if (!code || !own) return;
            codes[i] = code;
            // Written into the ComicInfo.xml with the rest.
            own.extra = { ...(own.extra ?? {}), gtin: code };
          });
        }
        const withCode = files.flatMap((f, i) => (codes[i] && owns[i] ? [i] : []));
        for (const [n, i] of withCode.entries()) {
          if (this.#ctx.stopRun !== run) return;
          this.progress = t("comicBarcodeLooking", { n: `${n + 1}/${withCode.length}` });
          const found = await api.comicByCode(key, codes[i]!);
          if (!found) continue;
          const vkey = `${source}:${found.volume.id}`;
          this.volumes[vkey] = { volume: found.volume, issues: found.issues };
          // The code tells the issue: its number too, when the name has none.
          const own = owns[i]!;
          const number = found.issues.find((x) => x.id === found.issue)?.number ?? "";
          const exact = own.number || !number ? own : { ...own, number };
          this.state[files[i].path] = { volumeKey: vkey, own: exact, choice: found.issue, status: "auto" };
          coded.add(files[i].path);
        }
      }
      const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();
      const groups = new Map<string, { query: string; paths: string[]; owns: ComicMatch[] }>();
      files.forEach((f, i) => {
        const own = owns[i];
        if (!own || coded.has(f.path)) return;
        const query = rule.series.trim() || own.series;
        const g = groups.get(fold(query)) ?? { query, paths: [], owns: [] };
        g.paths.push(f.path);
        g.owns.push(own);
        groups.set(fold(query), g);
      });
      if (!groups.size) {
        if (!coded.size) this.#ctx.say(t("comicNoFiles"));
        return;
      }
      for (const [k, g] of groups) {
        if (this.#ctx.stopRun !== run) break;
        // Loose chapters are no issues in the catalogues of issues.
        if (!seriesOnly(source) && g.owns.every((o) => !o.number && o.volume == null && !!o.chapter)) continue;
        const picked = `${source}:${k}`;
        let volume: ComicVolume | null = this.picked[picked] ?? null;
        // A `ComicInfo.xml` link to a Comic Vine issue tells the series.
        const linked = source === "comicvine" ? g.owns.find((o) => o.id) : undefined;
        if (!volume && linked) {
          try {
            const id = await api.comicVolumeOf(key, linked.id);
            volume = this.volumes[`${source}:${id}`]?.volume ?? { id, name: g.query, startYear: null, publisher: "", count: 0, cover: "", country: "", language: "", format: "", writer: "" };
          } catch {
            volume = null;
          }
        }
        if (!volume) {
          this.progress = t("comicSearching", { series: g.query });
          let found = await api.comicSearch(source, key, g.query, lang, null, "", skip);
          // `Series: Story` may be a story in `Series`: the series alone.
          const head = g.query.split(/\s*[:–]\s*|\s+-\s+/)[0].trim();
          if (!found.length && head && head !== g.query.trim()) found = await api.comicSearch(source, key, head, lang, null, "", skip);
          if (this.#ctx.stopRun !== run) break;
          const year = g.owns.find((o) => o.seriesYear)?.seriesYear ?? null;
          // A common name (`Saga`) may leave the series out of what the
          // catalogue answers: asked again with the files' year (theirs, or
          // their issues'), whose series come first.
          const dated = year ?? g.owns.find((o) => o.year)?.year ?? null;
          if (dated && !found.some((v) => fold(v.name) === k)) {
            const more = await api.comicSearch(source, key, g.query, lang, dated, "", skip).catch(() => []);
            if (this.#ctx.stopRun !== run) break;
            found = [...more, ...found.filter((v) => !more.some((m) => m.id === v.id))];
          }
          // The highest number among the files: a series with fewer issues
          // is not theirs.
          const highest = Math.max(
            0,
            ...g.owns.map((o) => Number.parseFloat(o.number || (o.chapter ? "" : String(o.volume ?? ""))) || 0),
          );
          const fits = (v: ComicVolume) => !v.count || v.count >= Math.floor(highest);
          const sorted = byFit(found, highest);
          const same = sorted.filter((v) => fold(v.name) === k && (year == null || v.startYear === year) && fits(v));
          // The same name in another language (a French edition for a
          // Brazilian reader, when there is one in Portuguese) is not
          // picked by itself.
          const mine = (v: ComicVolume) => !v.language || v.language.toLowerCase().startsWith(settings.lang);
          const foreign = same.length === 1 && !mine(same[0]) && found.some(mine);
          // Without the files' year, the name alone does not tell one
          // run from another (a translation of a newer series under an
          // older one's name): asked when there is anything else to pick.
          const sure = year != null || found.length === 1;
          volume =
            same.length === 1 && !foreign && sure
              ? same[0]
              : await new Promise<ComicVolume | null>((resolve) => {
                  // The file whose cover is compared: the first with a number.
                  const at = Math.max(0, g.owns.findIndex((o) => o.number || o.volume != null));
                  this.pick = { source, skip, query: g.query, files: g.paths.length, highest, candidates: sorted, path: g.paths[at], comic: g.owns[at], resolve };
                });
          this.pick = null;
          if (!volume) continue;
        }
        this.picked[picked] = volume;
        // The manga catalogues have series, not a publisher's volumes: the
        // file's numbers stay. MangaUpdates tells a series' authors and
        // volumes only when asked for it.
        if (seriesOnly(source)) {
          const vkey = `${source}:${volume.id}`;
          if (source === "mangaupdates") {
            const full = await api.comicIssues(source, key, volume.id, g.owns).catch(() => null);
            if (this.#ctx.stopRun !== run) break;
            if (full) volume = { ...volume, ...Object.fromEntries(Object.entries(full.volume).filter(([, x]) => x !== "" && x !== null && x !== 0)) };
          }
          this.volumes[vkey] = { volume, issues: [] };
          for (const [i, p] of g.paths.entries()) this.state[p] = { volumeKey: vkey, own: g.owns[i], choice: "", status: "auto" };
          continue;
        }
        if (this.#ctx.stopRun !== run) break;
        this.progress = t("comicLoading", { series: volume.name });
        const res = await api.comicIssues(source, key, volume.id, g.owns);
        const vkey = `${source}:${res.volume.id}`;
        // The search's own details where the series' lack them (a cover).
        this.volumes[vkey] = { volume: { ...volume, ...Object.fromEntries(Object.entries(res.volume).filter(([, x]) => x !== "" && x !== null && x !== 0)) }, issues: res.issues };
        g.paths.forEach((p, i) => {
          const found = res.found[i];
          const own = g.owns[i];
          // A loose chapter is no issue there: the series is all there is.
          const chapterOnly = !own.number && !!own.chapter;
          // A file without its issue still takes the series ("": the series
          // alone, with its own numbers), so a series' files stay together;
          // the review can still pick its issue.
          this.state[p] = {
            volumeKey: vkey,
            own,
            choice: found ?? "",
            status: found != null || chapterOnly ? "auto" : "choose",
          };
        });
      }
      if (Object.values(this.state).some((c) => c.status === "choose")) this.openReview();
    } catch (e) {
      const err = String(e);
      // A refused account is fixed in the options.
      if (err.startsWith("gcdBadLogin") || err.startsWith("mtBadKey")) {
        if (await confirm({ title: t("rule.comic"), text: tError(e), ok: t("options"), cancel: t("close") })) this.#ctx.openOptions();
      } else {
        await notify({ title: t("rule.comic"), text: tError(e) });
      }
    } finally {
      this.progress = null;
      this.pick = null;
    }
  }

  /** Opens the review on the comics still without an issue (or on one). */
  openReview(start?: string) {
    const paths = this.#ctx.items.map((i) => i.path);
    const pending = [
      ...paths.filter((p) => this.state[p]?.status === "choose"),
      ...paths.filter((p) => this.state[p]?.status === "none"),
    ];
    const queue = start ? [start, ...pending.filter((p) => p !== start)] : pending;
    this.review = queue.length ? queue : null;
  }

  choose(path: string, issue: string | null) {
    const c = this.state[path];
    if (c) this.state[path] = { ...c, choice: issue, status: issue == null ? "none" : "chosen" };
  }

  /** Opens the details window on the comics chosen in the list. */
  async openEdit() {
    const rows = this.selected;
    if (!rows.length) return;
    const r = this.rule?.rule;
    const missing = rows.filter((i) => !this.matches[i.path] && !this.state[i.path]);
    const guessed: Record<string, ComicMatch | null> = {};
    if (missing.length) {
      const got = await api.comicGuesses(
        missing.map((i) => i.path),
        missing.map((i) => (i.lookupName ? (i.isDir ? i.lookupName : splitExt(i.lookupName)[0]) : null)),
        r?.type === "comic" && !!r.byFolder,
      );
      missing.forEach((i, n) => (guessed[i.path] = got[n]));
    }
    this.edit = {
      paths: rows.map((i) => i.path),
      comics: rows.map((i) => $state.snapshot(this.matches[i.path] ?? this.state[i.path]?.own ?? guessed[i.path] ?? EMPTY)),
    };
  }

  /** The fields changed in the details window, for each of its comics. */
  saveEdit(changed: Record<string, string>) {
    const edit = this.edit;
    if (!edit) return;
    const num = (v: string) => (v ? Number.parseInt(v, 10) : null);
    const next = { ...this.edits };
    edit.paths.forEach((p, i) => {
      const m: ComicMatch = { ...edit.comics[i] };
      for (const [k, v] of Object.entries(changed)) {
        if (k === "seriesYear" || k === "volume" || k === "year") m[k] = num(v);
        else (m as unknown as Record<string, string>)[k] = v;
      }
      // The catalogue's issue no longer tells it.
      if (changed.series !== undefined || changed.number !== undefined) m.id = "";
      next[p] = m;
    });
    this.edits = next;
    this.edit = null;
  }

  /** Forgets the files taken out of the list (all, with the series
   *  picked, with null). */
  forget(paths: Set<string> | null) {
    if (!paths) {
      this.state = {};
      this.edits = {};
      this.picked = {};
      return;
    }
    this.state = without(this.state, paths);
    this.edits = without(this.edits, paths);
  }

  /** The files renamed: what was found and typed follows them. */
  follow(to: (p: string) => string) {
    this.state = moved(this.state, to);
    this.edits = moved(this.edits, to);
  }

  /** What was found, to put back with Ctrl+Z. */
  snapshot() {
    return { state: this.state, edits: this.edits, picked: this.picked };
  }
  restore(s: ReturnType<Comics["snapshot"]>) {
    this.state = s.state;
    this.edits = s.edits;
    this.picked = s.picked;
  }
}
