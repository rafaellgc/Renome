// The movie rule's side of the main page: what TMDB found per file, the
// choices, the collections (for the movies missing from them) and the
// template fields of each movie.

import { api } from "$lib/api";
import { confirm, notify } from "$lib/dialogs.svelte";
import { t, tError } from "$lib/i18n";
import { isScreen, isVideo, splitExt } from "$lib/rules";
import { settings, tmdbLang } from "$lib/settings.svelte";
import type { Candidate, MovieMatch, MovieSet, MovieState } from "$lib/types";
import { counts, moved, ruleOf, without, type PageCtx } from "./common";

export class Movies {
  #ctx: PageCtx;
  /** TMDB results and choices per file. */
  state = $state<Record<string, MovieState>>({});
  /** Files in the review window; null when closed. */
  review = $state<string[] | null>(null);
  /** TMDB work under way: searching movies, or saving the metadata of
   *  movies (`save`) and episodes (`saveTv`). */
  progress = $state<{ done: number; total: number; kind: "search" | "save" | "saveTv" } | null>(null);
  /** Each chosen movie's template fields (genre, director…), by id. */
  extra = $state<Record<number, Record<string, string>>>({});
  /** Each chosen movie's TMDB collection (null: none), and the
   *  collections, for the movies missing from them. */
  setOf = $state<Record<number, number | null>>({});
  sets = $state<Record<number, MovieSet>>({});
  /** Movies already asked about (an answer may not come). */
  #asked = new Set<string>();

  constructor(ctx: PageCtx) {
    this.#ctx = ctx;
    $effect(() => {
      const r = this.rule?.rule;
      if (r?.type !== "movie" || !settings.tmdbKey) return;
      const sets = r.missing;
      // Read without tracking the fields: they come from this.
      const ids = [...new Set(Object.values(this.state).flatMap((m) => (m.choice ? [m.choice.id] : [])))].filter(
        (id) => !this.#asked.has(`${id}:${sets}`) && !(sets ? false : this.#asked.has(`${id}:true`)),
      );
      if (!ids.length) return;
      for (const id of ids) this.#asked.add(`${id}:${sets}`);
      api.tmdbCollections(settings.tmdbKey, tmdbLang(), ids, sets).then(
        (res) => {
          for (const [id, set] of res.of) this.setOf[id] = set;
          for (const set of res.sets) this.sets[set.id] = set;
          for (const [id, extra] of res.extras) this.extra[id] = extra;
        },
        () => {},
      );
    });
  }

  rule = $derived.by(() => ruleOf(this.#ctx.steps, "movie"));
  /** Each chosen movie, as the rule names it. */
  matches = $derived.by(() =>
    Object.fromEntries(
      Object.entries(this.state)
        .filter(([, m]) => m.choice)
        .map(([p, m]): [string, MovieMatch] => {
          const c = m.choice!;
          return [p, { id: c.id, title: c.title, originalTitle: c.originalTitle, year: c.year, extra: this.extra[c.id] }];
        }),
    ),
  );
  /** Movies whose names are already right but that still get .nfo and
   *  images (files that already exist are skipped when writing). */
  metaOnly = $derived.by(() => {
    const r = this.rule?.rule;
    return r?.type === "movie" && r.sidecar
      ? this.#ctx.items.filter((i) => i.status === "unchanged" && this.state[i.path]?.choice && isVideo(i.original))
      : [];
  });
  counts = $derived.by(() => counts(this.state, this.#ctx.items));

  /** Looks up the visible videos (and folders) without a choice yet; those
   *  picked with confidence are done, the rest go to the review window. */
  async search() {
    if (!settings.tmdbKey.trim()) {
      const ok = await confirm({ title: t("rule.movie"), text: t("tmdbNeedsKey"), ok: t("options"), cancel: t("cancel") });
      if (ok) this.#ctx.openOptions();
      return;
    }
    const items = this.#ctx.items
      .filter((i) => (i.isDir || isScreen(i.original)) && this.state[i.path]?.status !== "auto" && this.state[i.path]?.status !== "chosen")
      .map((i) => ({ path: i.path, name: i.isDir ? this.#ctx.readName(i) : splitExt(this.#ctx.readName(i))[0] }));
    if (!items.length) {
      this.#ctx.say(t("tmdbNoFiles"));
      return;
    }
    this.progress = { done: 0, total: items.length, kind: "search" };
    try {
      const found = await api.tmdbLookup(settings.tmdbKey, tmdbLang(), items);
      // Nothing was really searched (bad key, no connection): mark nothing.
      const failed = found.find((f) => f.error);
      if (failed && found.every((f) => f.error || !f.guess.title)) {
        await notify({ title: t("rule.movie"), text: tError(failed.error) });
        return;
      }
      for (const f of found) {
        this.state[f.path] = {
          guess: f.guess,
          candidates: f.candidates,
          choice: f.auto ? f.candidates[0] : null,
          status: f.auto ? "auto" : f.candidates.length ? "choose" : "none",
          error: f.error,
        };
      }
      if (found.some((f) => !f.auto && f.candidates.length)) this.openReview();
    } catch (e) {
      await notify({ title: t("rule.movie"), text: tError(e) });
    } finally {
      this.progress = null;
    }
  }

  /** Opens the review on the files still without a movie (or on one). */
  openReview(start?: string) {
    // Files with options first; those without results after.
    const paths = this.#ctx.items.map((i) => i.path);
    const pending = [
      ...paths.filter((p) => this.state[p]?.status === "choose"),
      ...paths.filter((p) => this.state[p]?.status === "none"),
    ];
    const queue = start ? [start, ...pending.filter((p) => p !== start)] : pending;
    this.review = queue.length ? queue : null;
  }

  /** The user's pick for a file; undecided files with the same guess (its
   *  subtitles, say) get it too. */
  choose(path: string, choice: Candidate | null, candidates?: Candidate[]) {
    const m = this.state[path];
    if (!m) return;
    const set = (p: string) => {
      this.state[p] = { ...this.state[p], choice, status: choice ? "chosen" : "none", candidates: candidates ?? this.state[p].candidates };
    };
    set(path);
    for (const [p, o] of Object.entries(this.state)) {
      const same = o.guess.title === m.guess.title && o.guess.year === m.guess.year;
      if (p !== path && same && (o.status === "choose" || o.status === "none")) set(p);
    }
  }

  /** Forgets the files taken out of the list (all, with null). */
  forget(paths: Set<string> | null) {
    this.state = paths ? without(this.state, paths) : {};
  }

  /** The files renamed: what was found follows them. */
  follow(to: (p: string) => string) {
    this.state = moved(this.state, to);
  }

  /** What was found, to put back with Ctrl+Z. */
  snapshot() {
    return { state: this.state };
  }
  restore(s: ReturnType<Movies["snapshot"]>) {
    this.state = s.state;
  }
}
