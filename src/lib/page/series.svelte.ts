// The TV show rule's side of the main page: the shows found on TMDB (or
// AniList), each file's episode, the choices and what the metadata needs.

import { api } from "$lib/api";
import { confirm, notify } from "$lib/dialogs.svelte";
import { t, tError } from "$lib/i18n";
import { settings, tmdbLang } from "$lib/settings.svelte";
import type { Candidate, EpKey, EpisodeMatch, EpisodeState, Rule, Show } from "$lib/types";
import { counts, moved, ruleOf, takers, without, type PageCtx } from "./common";

/** What the show picker asks, while open. */
export type ShowPick = { source: "tmdb" | "anilist"; query: string; candidates: Candidate[]; resolve: (c: Candidate | null) => void };

export class Series {
  #ctx: PageCtx;
  /** Each file's episode. */
  state = $state<Record<string, EpisodeState>>({});
  /** Shows fetched, by id (AniList's under negative numbers). */
  shows = $state<Record<number, Show>>({});
  /** The shows picked, by the text they were found by (not asked again). */
  picked = $state<Record<string, Candidate>>({});
  /** The shows of the last search. */
  searched = $state<Candidate[]>([]);
  /** Files in the review window; null when closed. */
  review = $state<string[] | null>(null);
  /** The show picker, while open. */
  pick = $state<ShowPick | null>(null);
  progress = $state<string | null>(null);

  constructor(ctx: PageCtx) {
    this.#ctx = ctx;
  }

  rule = $derived.by(() => ruleOf(this.#ctx.steps, "series"));
  /** Each file's episode, as the rule names it (made by the shared flow);
   *  rebuilt whenever the choices change. */
  matches = $state<Record<string, EpisodeMatch>>({});

  /** Asks the shared flow for the episodes' details, from the choices. */
  async rebuild() {
    const keys = [...new Set(Object.values(this.state).map((e) => e.showId))].filter((k) => this.shows[k]);
    const at = new Map(keys.map((k, i) => [k, i]));
    const files = Object.entries(this.state).flatMap(([path, e]) =>
      e.choice && at.has(e.showId) ? [{ path, show: at.get(e.showId)!, choice: e.choice }] : [],
    );
    this.matches = files.length ? await api.seriesNamed($state.snapshot(files), $state.snapshot(keys.map((k) => this.shows[k]))) : {};
  }
  counts = $derived.by(() => counts(this.state, this.#ctx.items));


  /** Finds the show (asking when unsure), then each visible file's episode;
   *  doubtful files go to the review window. With no show typed, each show
   *  the file names (or folders) say is looked up in turn. The flow is the
   *  one the command line runs; its questions come as `flow-ask` (see
   *  {@link ask}). */
  async search(rule: Extract<Rule, { type: "series" }>) {
    // AniList: anime, with no key; its shows are kept apart from TMDB's
    // (their ids would meet), under negative numbers.
    const anime = rule.source === "anilist";
    const showKey = (id: number) => (anime ? -id : id);
    if (!anime && !settings.tmdbKey.trim()) {
      const ok = await confirm({ title: t("rule.series"), text: t("tmdbNeedsKey"), ok: t("options"), cancel: t("cancel") });
      if (ok) this.#ctx.openOptions();
      return;
    }
    // Files already settled keep their episode in their show.
    const settled = Object.entries(this.state)
      .filter(([, e]) => (anime ? e.showId < 0 : e.showId > 0) && (e.status === "auto" || e.status === "chosen"))
      .map(([p, e]): [string, number] => [p, Math.abs(e.showId)]);
    this.progress = t("tvFinding");
    try {
      const res = await api.seriesSearch(settings.tmdbKey, tmdbLang(), rule.source ?? "tmdb", rule.show, this.#ctx.items, settled, this.picked);
      if (res.empty) {
        this.#ctx.say(t("tvNoFiles"));
        return;
      }
      this.picked = res.picked;
      for (const show of res.shows) this.shows[showKey(show.id)] = show;
      for (const f of res.files) {
        this.state[f.path] = {
          showId: showKey(f.showId),
          guess: f.guess,
          alternatives: f.alternatives,
          conflict: f.conflict,
          doubled: f.doubled,
          choice: f.choice.length ? f.choice : null,
          status: f.status,
          group: f.group,
          read: f.read,
        };
      }
      await this.rebuild();
      if (res.found.length) this.searched = res.found;
      if (!res.searched && res.found.length) this.#ctx.say(t("tvNoFiles"));
      else if (res.loose) this.#ctx.say(t("tvLoose", { n: res.loose }));
      if (res.files.some((f) => !f.choice.length)) this.openReview();
    } catch (e) {
      await notify({ title: t("rule.series"), text: tError(e) });
    } finally {
      this.pick = null;
      this.progress = null;
    }
  }

  /** The flow's progress: finding a show, or loading one's episodes. */
  onProgress(p: { stage: "finding" | "loading"; show?: string }) {
    if (this.progress) this.progress = p.stage === "loading" ? t("tvLoading", { show: p.show ?? "" }) : t("tvFinding");
  }

  /** The flow asks which show the files are: the picker opens, and its
   *  answer goes back. */
  ask(id: number, q: { source: "tmdb" | "anilist"; query: string; candidates: Candidate[] }) {
    this.pick = {
      ...q,
      resolve: (c) => {
        this.pick = null;
        api.flowAnswer(id, c);
      },
    };
  }

  /** Opens the review on the files still without an episode (or on one). */
  openReview(start?: string) {
    const paths = this.#ctx.items.map((i) => i.path);
    const pending = [
      ...paths.filter((p) => this.state[p]?.status === "choose"),
      ...paths.filter((p) => this.state[p]?.status === "none"),
    ];
    const queue = start ? [start, ...pending.filter((p) => p !== start)] : pending;
    this.review = queue.length ? queue : null;
  }

  /** The user's pick for a file; undecided files of its group (its
   *  subtitles, say) get it too. */
  async choose(path: string, choice: EpKey[] | null) {
    if (!this.state[path]) return;
    for (const p of await takers(path, this.state)) {
      this.state[p] = { ...this.state[p], choice, status: choice ? "chosen" : "none" };
    }
    await this.rebuild();
  }

  /** Forgets the files taken out of the list (all, with the shows picked
   *  and searched, with null). */
  forget(paths: Set<string> | null) {
    if (!paths) {
      this.state = {};
      this.picked = {};
      this.searched = [];
      this.matches = {};
      return;
    }
    this.state = without(this.state, paths);
    this.matches = without(this.matches, paths);
  }

  /** The files renamed: what was found follows them. */
  follow(to: (p: string) => string) {
    this.state = moved(this.state, to);
    this.matches = moved(this.matches, to);
  }

  /** What was found, to put back with Ctrl+Z. */
  snapshot() {
    return { state: this.state, picked: this.picked, searched: this.searched, matches: this.matches };
  }
  restore(s: ReturnType<Series["snapshot"]>) {
    this.state = s.state;
    this.picked = s.picked;
    this.searched = s.searched;
    this.matches = s.matches;
  }
}
