// The music rule's side of the main page: the albums looked up on
// MusicBrainz (or Discogs), the releases chosen, the tags typed in and
// what writing covers, playlists and tags needs.

import { api } from "$lib/api";
import { notify } from "$lib/dialogs.svelte";
import { t, tError } from "$lib/i18n";
import type { AlbumState, EditField, MbCandidate, Rule, SongMatch } from "$lib/types";
import { editFields, moved, reviewQueue, ruleOf, without, type PageCtx } from "./common";

export class Music {
  #ctx: PageCtx;
  /** Albums looked up, with their songs and the release chosen. */
  albums = $state<AlbumState[]>([]);
  /** Songs whose tags were typed in (over what the release said). */
  edits = $state<Record<string, SongMatch>>({});
  /** The details window, while open. */
  edit = $state<{ paths: string[]; songs: SongMatch[]; rows: Record<string, string>[] } | null>(null);
  /** Albums in the review window (indexes); null when closed. */
  review = $state<number[] | null>(null);
  progress = $state<{ done: number; total: number; kind: "search" | "listen" | "save" | "tags" } | null>(null);

  constructor(ctx: PageCtx) {
    this.#ctx = ctx;
  }

  rule = $derived.by(() => ruleOf(this.#ctx.steps, "music"));
  /** Each song's details: from its chosen release, or typed in. */
  matches = $derived.by(() => ({
    ...(Object.fromEntries(this.albums.flatMap((a) => (a.choice ? Object.entries(a.matches) : []))) as Record<string, SongMatch>),
    ...this.edits,
  }));
  /** The album each song is in. */
  albumOf = $derived.by(() => new Map(this.albums.flatMap((a, i) => a.files.map((p): [string, number] => [p, i]))));
  /** The songs among the rows chosen in the list. */
  selected = $derived.by(() => this.#ctx.items.filter((i) => this.#ctx.selected.has(i.path) && i.editable));
  counts = $derived.by(() => {
    let found = 0, choose = 0, none = 0;
    for (const a of this.albums) {
      if (a.status === "auto" || a.status === "chosen") found++;
      else if (a.status === "choose") choose++;
      else none++;
    }
    return { found, choose, none };
  });
  /** The details window's fields. */
  fields = $derived<EditField[]>(editFields("music"));

  /** Looks up the visible songs' albums on MusicBrainz; albums already
   *  settled keep their release, doubtful ones go to the review window. */
  async search(rule: Extract<Rule, { type: "music" }>) {
    const settled = this.albums.filter((a) => a.status === "auto" || a.status === "chosen").flatMap((a) => a.files);
    this.progress = { done: 0, total: 0, kind: "search" };
    try {
      // Which songs, and what is sure, the shared flow says.
      const found = await api.musicSearch(this.#ctx.items, settled, rule.fromName, rule.identifyBySound);
      if (!found.length) {
        this.#ctx.say(t("mbNoFiles"));
        return;
      }
      const looked = new Set(found.flatMap((f) => f.files));
      this.albums = [
        ...this.albums.filter((a) => !a.files.some((p) => looked.has(p))),
        ...found.map((f): AlbumState => ({
          files: f.files,
          dir: f.dir,
          query: f.query,
          candidates: f.candidates,
          choice: f.choice,
          matches: Object.fromEntries(f.matches),
          status: f.status,
          error: f.error,
          heard: f.heard,
          tracks: f.tracks,
        })),
      ];
      this.openReview();
    } catch (e) {
      await notify({ title: t("rule.music"), text: tError(e) });
    } finally {
      this.progress = null;
    }
  }

  /** Opens the review on the albums without a release (or on one). */
  openReview(start?: number) {
    this.review = reviewQueue(this.albums.map((_, i) => i), (i) => this.albums[i].status, start);
  }

  /** The release the user chose for an album: its songs are matched to
   *  the tracks; `null` keeps the tags. */
  async choose(index: number, choice: MbCandidate | null, candidates?: MbCandidate[]) {
    const a = this.albums[index];
    const rule = this.rule?.rule;
    if (!choice || rule?.type !== "music") {
      this.albums[index] = { ...a, choice: null, matches: {}, tracks: [], status: "none" };
      return;
    }
    const res = await api.mbChoose(choice.id, a.files, rule.fromName);
    this.albums[index] = {
      ...a,
      choice,
      candidates: candidates ?? a.candidates,
      matches: Object.fromEntries(res.matches),
      tracks: res.tracks,
      status: "chosen",
    };
  }

  /** Opens the details window on the songs chosen in the list; what each
   *  starts from, the shared flow says (the command line's --edit too). */
  async openEdit() {
    if (!this.selected.length) return;
    const r = this.rule?.rule;
    const known = Object.fromEntries(this.selected.map((i) => [i.path, { current: this.matches[i.path] }]));
    const [paths, songs, rows] = await api.editStart<SongMatch>({
      kind: "music",
      chosen: this.selected.map((i) => i.path),
      known: $state.snapshot(known),
      parts: {},
      dates: {},
      fromName: r?.type === "music" && !!r.fromName,
      byFolder: false,
      audiobooks: false,
    });
    if (paths.length) this.edit = { paths, songs, rows };
  }

  /** The fields changed in the details window, for each of its songs (applied as the command line's --edit does). */
  async saveEdit(changed: Record<string, string>) {
    const edit = this.edit;
    if (!edit) return;
    const songs = await api.editDetails("song", $state.snapshot(edit.songs), changed);
    const next = { ...this.edits };
    edit.paths.forEach((p, i) => (next[p] = songs[i]));
    this.edits = next;
    this.edit = null;
  }

  /** Forgets the files taken out of the list (all, with null). */
  forget(paths: Set<string> | null) {
    if (!paths) {
      this.albums = [];
      this.edits = {};
      return;
    }
    this.albums = this.albums
      .map((a) => ({ ...a, files: a.files.filter((p) => !paths.has(p)), matches: without(a.matches, paths) }))
      .filter((a) => a.files.length);
    this.edits = without(this.edits, paths);
  }

  /** The files renamed: what was found and typed follows them. */
  follow(to: (p: string) => string) {
    for (const a of this.albums) {
      a.files = a.files.map(to);
      a.matches = moved(a.matches, to);
    }
    this.edits = moved(this.edits, to);
  }

  /** What was found, to put back with Ctrl+Z. */
  snapshot() {
    return { albums: this.albums, edits: this.edits };
  }
  restore(s: ReturnType<Music["snapshot"]>) {
    this.albums = s.albums;
    this.edits = s.edits;
  }
}
