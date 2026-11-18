// The TV show rule's side of the main page: the shows found on TMDB (or
// AniList), each file's episode, the choices and what the metadata needs.

import { api } from "$lib/api";
import { confirm, notify } from "$lib/dialogs.svelte";
import { t, tError } from "$lib/i18n";
import { isVideo } from "$lib/rules";
import { settings, tmdbLang } from "$lib/settings.svelte";
import type { Candidate, EpKey, EpisodeMatch, EpisodeState, Rule, Show } from "$lib/types";
import { counts, moved, ruleOf, without, type PageCtx } from "./common";

/** An episode's template fields: the show's, its air date and its number
 *  counted from the first episode (specials apart). */
function episodeExtra(show: Show, season: number, number: number): Record<string, string> {
  const extra: Record<string, string> = { ...(show.extra ?? {}) };
  const ep = show.episodes.find((x) => x.season === season && x.number === number);
  if (ep?.airDate) extra.date = ep.airDate;
  if (season > 0 && ep) {
    extra.absolute = String(show.episodes.filter((x) => x.season > 0 && (x.season < season || (x.season === season && x.number <= number))).length);
  }
  return extra;
}

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
  /** Each file's episode, as the rule names it. */
  matches = $derived.by(() =>
    Object.fromEntries(
      Object.entries(this.state)
        .filter(([, e]) => e.choice && this.shows[e.showId])
        .map(([p, e]): [string, EpisodeMatch] => {
          const show = this.shows[e.showId];
          const keys = e.choice!;
          return [p, {
            source: show.source ?? "",
            id: show.id,
            show: show.name,
            originalShow: show.originalName,
            year: show.year,
            season: keys[0][0],
            episodes: keys.map((k) => k[1]),
            titles: keys.map((k) => show.episodes.find((x) => x.season === k[0] && x.number === k[1])?.name ?? ""),
            extra: episodeExtra(show, keys[0][0], keys[0][1]),
          }];
        }),
    ),
  );
  counts = $derived.by(() => counts(this.state, this.#ctx.items));
  /** Episodes whose names are already right but that still get metadata. */
  metaOnly = $derived.by(() => {
    const r = this.rule?.rule;
    return r?.type === "series" && r.nfo && r.source !== "anilist"
      ? this.#ctx.items.filter((i) => i.status === "unchanged" && this.matches[i.path] && isVideo(i.original))
      : [];
  });

  /** What the metadata of an episode needs; `key` is the path before renaming. */
  sidecarItem(key: string, path: string) {
    const m = this.matches[key];
    return { path, id: m.id, season: m.season, episodes: m.episodes, titles: m.titles };
  }

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
          status: f.choice.length ? "auto" : f.alternatives.length ? "choose" : "none",
          group: f.group,
        };
      }
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
  choose(path: string, choice: EpKey[] | null) {
    const e = this.state[path];
    if (!e) return;
    for (const [p, o] of Object.entries(this.state)) {
      const undecided = o.status === "choose" || o.status === "none";
      if (p === path || (undecided && o.group === e.group)) {
        this.state[p] = { ...o, choice, status: choice ? "chosen" : "none" };
      }
    }
  }

  /** Forgets the files taken out of the list (all, with the shows picked
   *  and searched, with null). */
  forget(paths: Set<string> | null) {
    if (!paths) {
      this.state = {};
      this.picked = {};
      this.searched = [];
      return;
    }
    this.state = without(this.state, paths);
  }

  /** The files renamed: what was found follows them. */
  follow(to: (p: string) => string) {
    this.state = moved(this.state, to);
  }

  /** What was found, to put back with Ctrl+Z. */
  snapshot() {
    return { state: this.state, picked: this.picked, searched: this.searched };
  }
  restore(s: ReturnType<Series["snapshot"]>) {
    this.state = s.state;
    this.picked = s.picked;
    this.searched = s.searched;
  }
}
