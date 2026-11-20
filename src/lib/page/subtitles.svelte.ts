// Subtitles for the videos the movie and TV show rules identified: what
// OpenSubtitles and SubDL found, the ones chosen and what downloading them
// with the rename needs.

import { api } from "$lib/api";
import { confirm, notify } from "$lib/dialogs.svelte";
import { t, tError } from "$lib/i18n";
import { settings } from "$lib/settings.svelte";
import type { SubCandidate, SubsAuth } from "$lib/types";
import { moved, without, type PageCtx } from "./common";
import type { Movies } from "./movies.svelte";
import type { Series } from "./series.svelte";

/** Subtitles found for a video: the languages asked for it has already
 *  beside it, those it has only inside it (looked up, not chosen until
 *  asked), the candidates best first, and the one chosen per language (an index
 *  in `candidates`; -1: none). `key` tells what was searched (movie or
 *  episode, languages), to search again on a change. */
export type SubsState = { key: string; have: string[]; inside: string[]; candidates: SubCandidate[]; pick: Record<string, number> };

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
    $effect(() => {
      const rules = this.#ctx.steps.filter((s) => s.enabled).map((s) => $state.snapshot(s.rule));
      api.subsWanted(rules).then((w) => (this.wanted = w), () => {});
    });
  }

  /** The languages asked for by the movie or TV show rule, when it
   *  downloads subtitles (the shared flow says: `subtitles::wanted`). */
  wanted = $state<string | null>(null);

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
      if (this.langs.some((l) => !s.have.includes(l) && !s.inside.includes(l) && !(s.pick[l] >= 0))) missing++;
    }
    return { subs, videos, missing };
  });

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
      subsourceKey: settings.subsourceKey.trim(),
      tmdbKey: settings.tmdbKey.trim(),
    };
  }

  /** The visible videos identified, with what to look their subtitles up
   *  by (which ones, the shared flow says). */
  items() {
    const files = this.#ctx.items.map((i) => ({
      path: i.path,
      original: i.original,
      isDir: i.isDir,
      movie: (this.#movies.rule ? this.#movies.state[i.path]?.choice?.id : null) ?? null,
      episode: (this.#series.rule ? this.#series.matches[i.path] : null) ?? null,
      conflict: i.status === "conflict",
    }));
    return api.subsTargets($state.snapshot(files), this.wanted ?? "");
  }

  /** Searches subtitles for the visible videos identified (those searched
   *  already for the same languages are kept, unless `again`). */
  async search(again = false) {
    const langs = this.wanted;
    if (langs === null) return;
    const auth = this.auth();
    if (!(await api.subsReady(auth))) {
      const ok = await confirm({ title: t("subsTitle"), text: t("subsNeedKey"), ok: t("options"), cancel: t("cancel") });
      if (ok) this.#ctx.openOptions();
      return;
    }
    const all = await this.items();
    const items = all.filter((it) => again || this.found[it.path]?.key !== it.key);
    if (!items.length) {
      if (!all.length) this.#ctx.say(t("subsNoVideos"));
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
        this.found[f.path] = { key: asked.get(f.path)!.key, have: f.have, inside: f.inside, candidates: f.candidates, pick: f.pick };
      }
      if (r.errors.length) await notify({ title: t("subsTitle"), text: r.errors.map(tError).join("\n") });
      // Shown even with nothing found, to tell why.
      else if (r.found.length) this.review = true;
    } catch (e) {
      await notify({ title: t("subsTitle"), text: tError(e) });
    } finally {
      this.progress = null;
    }
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
