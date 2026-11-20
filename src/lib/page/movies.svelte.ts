// The movie rule's side of the main page: what TMDB found per file, the
// choices, the collections (for the movies missing from them) and the
// template fields of each movie.

import { api } from "$lib/api";
import { confirm, notify } from "$lib/dialogs.svelte";
import { t, tError } from "$lib/i18n";
import { settings, tmdbLang } from "$lib/settings.svelte";
import type { Candidate, MovieMatch, MovieSet, MovieState } from "$lib/types";
import { needsKey } from "$lib/rules";
import { counts, moved, reviewQueue, ruleOf, takers, without, type PageCtx } from "./common";

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
          if (res.extras.length) this.rebuild();
        },
        () => {},
      );
    });
  }

  rule = $derived.by(() => ruleOf(this.#ctx.steps, "movie"));
  /** Each chosen movie, as the rule names it (made by the shared flow:
   *  its choice with its template fields); rebuilt whenever they change. */
  matches = $state<Record<string, MovieMatch>>({});

  /** Asks the shared flow for the movies' details, from the choices. */
  async rebuild() {
    const files = Object.entries(this.state).flatMap(([p, m]): [string, Candidate][] => (m.choice ? [[p, m.choice]] : []));
    this.matches = files.length ? await api.movieNamed($state.snapshot(files), $state.snapshot(this.extra)) : {};
  }
  counts = $derived.by(() => counts(this.state, this.#ctx.items));

  /** Looks up the visible videos (and folders) without a choice yet; those
   *  picked with confidence are done, the rest go to the review window. */
  async search() {
    // Those already settled are not searched again; which of the rest are
    // movies, and what is sure, the shared flow says.
    const items = this.#ctx.items.filter((i) => this.state[i.path]?.status !== "auto" && this.state[i.path]?.status !== "chosen");
    this.progress = { done: 0, total: items.length, kind: "search" };
    try {
      const found = await api.movieSearch(settings.tmdbKey, tmdbLang(), items);
      if (!found.length) {
        this.#ctx.say(t("tmdbNoFiles"));
        return;
      }
      for (const f of found) {
        this.state[f.path] = {
          guess: f.guess,
          candidates: f.candidates,
          choice: f.choice,
          status: f.status,
          error: f.error,
          group: f.group,
        };
      }
      await this.rebuild();
      if (found.some((f) => !f.choice && f.candidates.length)) this.openReview();
    } catch (e) {
      // No key: it is typed in the options.
      if (needsKey(e)) {
        if (await confirm({ title: t("rule.movie"), text: tError(e), ok: t("options"), cancel: t("cancel") })) this.#ctx.openOptions();
      } else await notify({ title: t("rule.movie"), text: tError(e) });
    } finally {
      this.progress = null;
    }
  }

  /** Opens the review on the files still without a movie (or on one). */
  openReview(start?: string) {
    this.review = reviewQueue(this.#ctx.items.map((i) => i.path), (p) => this.state[p]?.status, start);
  }

  /** The user's pick for a file; undecided files of its group (its
   *  subtitles, say) get it too (the shared rule). */
  async choose(path: string, choice: Candidate | null, candidates?: Candidate[]) {
    if (!this.state[path]) return;
    for (const p of await takers(path, this.state)) {
      this.state[p] = { ...this.state[p], choice, status: choice ? "chosen" : "none", candidates: candidates ?? this.state[p].candidates };
    }
    await this.rebuild();
  }

  /** Forgets the files taken out of the list (all, with null). */
  forget(paths: Set<string> | null) {
    this.state = paths ? without(this.state, paths) : {};
    this.matches = paths ? without(this.matches, paths) : {};
  }

  /** The files renamed: what was found follows them. */
  follow(to: (p: string) => string) {
    this.state = moved(this.state, to);
    this.matches = moved(this.matches, to);
  }

  /** What was found, to put back with Ctrl+Z. */
  snapshot() {
    return { state: this.state, matches: this.matches };
  }
  restore(s: ReturnType<Movies["snapshot"]>) {
    this.state = s.state;
    this.matches = s.matches;
  }
}
