// Subtitles for the videos the movie and TV show rules identified: what
// OpenSubtitles and SubDL found, the ones chosen and what downloading them
// with the rename needs.

import { api } from "$lib/api";
import { confirm, notify } from "$lib/dialogs.svelte";
import { t, tError } from "$lib/i18n";
import { defaultSubLangs, isVideo } from "$lib/rules";
import { settings } from "$lib/settings.svelte";
import type { SubCandidate, SubsAuth } from "$lib/types";
import { moved, without, type PageCtx } from "./common";
import type { Movies } from "./movies.svelte";
import type { Series } from "./series.svelte";

/** Subtitles found for a video: the languages asked for it has already,
 *  the candidates best first, and the one chosen per language (an index
 *  in `candidates`; -1: none). `key` tells what was searched (movie or
 *  episode, languages), to search again on a change. */
export type SubsState = { key: string; have: string[]; candidates: SubCandidate[]; pick: Record<string, number> };

const subsKey = (it: { tmdb: number; season?: number; episode?: number }, langs: string) => `${it.tmdb}:${it.season ?? ""}:${it.episode ?? ""}:${langs}`;
/** Subtitles chosen for a video, not there yet. */
const chosen = (s: SubsState | undefined) => (s ? Object.entries(s.pick).filter(([l, i]) => i >= 0 && !s.have.includes(l)).length : 0);

export class Subtitles {
  #ctx: PageCtx;
  #movies: Movies;
  #series: Series;
  /** What was found per video (by its path). */
  found = $state<Record<string, SubsState>>({});
  /** The languages of the last search, as understood (`pt-BR`, `en`). */
  langs = $state<string[]>([]);
  progress = $state<{ done: number; total: number; kind: "search" | "save" } | null>(null);
  /** The review window is open. */
  review = $state(false);
  /** OpenSubtitles downloads left today, when known. */
  remaining = $state<number | null>(null);

  constructor(ctx: PageCtx, movies: Movies, series: Series) {
    this.#ctx = ctx;
    this.#movies = movies;
    this.#series = series;
  }

  /** The languages asked for by the movie or TV show rule, when it
   *  downloads subtitles. */
  wanted = $derived.by(() => {
    const m = this.#movies.rule?.rule;
    const s = this.#series.rule?.rule;
    const r = m?.type === "movie" ? m : s?.type === "series" && s.source !== "anilist" ? s : null;
    return r?.subtitles ? (r.subLangs ?? defaultSubLangs()) : null;
  });
  counts = $derived.by(() => {
    const visible = new Set(this.#ctx.items.map((i) => i.path));
    let subs = 0;
    let videos = 0;
    let missing = 0;
    for (const [p, s] of Object.entries(this.found)) {
      if (!visible.has(p)) continue;
      const n = chosen(s);
      subs += n;
      if (n) videos++;
      if (this.langs.some((l) => !s.have.includes(l) && !(s.pick[l] >= 0))) missing++;
    }
    return { subs, videos, missing };
  });
  /** Videos whose names are already right but that still get subtitles. */
  only = $derived.by(() =>
    this.wanted === null ? [] : this.#ctx.items.filter((i) => i.status === "unchanged" && chosen(this.found[i.path]) > 0),
  );

  /** Subtitles chosen for a video, not there yet. */
  chosen(s: SubsState | undefined) {
    return chosen(s);
  }

  /** The keys and account from the options. */
  auth(): SubsAuth {
    return {
      osKey: settings.osKey.trim(),
      osUser: settings.osUser.trim(),
      osPassword: settings.osPassword,
      subdlKey: settings.subdlKey.trim(),
    };
  }

  /** The visible videos identified, with what to look their subtitles up by. */
  items() {
    const out: { path: string; tmdb: number; season?: number; episode?: number }[] = [];
    for (const i of this.#ctx.items) {
      if (i.isDir || !isVideo(i.original)) continue;
      const m = this.#movies.rule ? this.#movies.state[i.path]?.choice : null;
      const e = this.#series.rule ? this.#series.matches[i.path] : null;
      if (m) out.push({ path: i.path, tmdb: m.id });
      else if (e && !e.source && e.episodes.length === 1) out.push({ path: i.path, tmdb: e.id, season: e.season, episode: e.episodes[0] });
    }
    return out;
  }

  /** Searches subtitles for the visible videos identified (those searched
   *  already for the same languages are kept, unless `again`). */
  async search(again = false) {
    const langs = this.wanted;
    if (langs === null) return;
    const auth = this.auth();
    if (!auth.osKey && !auth.subdlKey) {
      const ok = await confirm({ title: t("subsTitle"), text: t("subsNeedKey"), ok: t("options"), cancel: t("cancel") });
      if (ok) this.#ctx.openOptions();
      return;
    }
    const items = this.items().filter((it) => again || this.found[it.path]?.key !== subsKey(it, langs));
    if (!items.length) {
      if (!this.items().length) this.#ctx.say(t("subsNoVideos"));
      else if (this.counts.videos) this.review = true;
      return;
    }
    this.progress = { done: 0, total: items.length, kind: "search" };
    try {
      const r = await api.subsSearch(auth, langs, items);
      this.langs = r.langs;
      if (r.remaining !== null) this.remaining = r.remaining;
      const asked = new Map(items.map((it) => [it.path, it]));
      for (const f of r.found) {
        const pick: Record<string, number> = {};
        f.candidates.forEach((c, i) => {
          if (!(c.lang in pick)) pick[c.lang] = i;
        });
        this.found[f.path] = { key: subsKey(asked.get(f.path)!, langs), have: f.have, candidates: f.candidates, pick };
      }
      if (r.errors.length) await notify({ title: t("subsTitle"), text: r.errors.map(tError).join("\n") });
      else if (r.found.some((f) => f.candidates.length)) this.review = true;
    } catch (e) {
      await notify({ title: t("subsTitle"), text: tError(e) });
    } finally {
      this.progress = null;
    }
  }

  /** What to download for renamed videos (`from` → `to`): per language, the
   *  one chosen first and the others of that language after it. */
  writes(pairs: [string, string][]) {
    return pairs.flatMap(([from, to]) => {
      const s = this.found[from];
      if (!s) return [];
      const langs = Object.entries(s.pick)
        .filter(([l, i]) => i >= 0 && !s.have.includes(l))
        .map(([l, i]) => [s.candidates[i], ...s.candidates.filter((c, j) => j !== i && c.lang === l)]);
      if (!langs.length) return [];
      const e = this.#series.matches[from];
      return [{ path: to, season: e?.season, episode: e?.episodes[0], langs }];
    });
  }

  /** Forgets the files taken out of the list (all, with null). */
  forget(paths: Set<string> | null) {
    this.found = paths ? without(this.found, paths) : {};
  }

  /** The files renamed: what was found follows them. */
  follow(to: (p: string) => string) {
    this.found = moved(this.found, to);
  }
}
