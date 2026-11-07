// The music rule's side of the main page: the albums looked up on
// MusicBrainz (or Discogs), the releases chosen, the tags typed in and
// what writing covers, playlists and tags needs.

import { api } from "$lib/api";
import { notify } from "$lib/dialogs.svelte";
import { t, tError } from "$lib/i18n";
import { isAudio } from "$lib/rules";
import type { AlbumState, ApplyReport, EditField, MbCandidate, Rule, SongMatch } from "$lib/types";
import { moved, ruleOf, without, type PageCtx } from "./common";

const EMPTY: SongMatch = { release: "", title: "", artist: "", albumArtist: "", album: "", year: null, track: 0, trackTotal: 0, disc: 0, discTotal: 0, typed: true };

export class Music {
  #ctx: PageCtx;
  /** Albums looked up, with their songs and the release chosen. */
  albums = $state<AlbumState[]>([]);
  /** Songs whose tags were typed in (over what the release said). */
  edits = $state<Record<string, SongMatch>>({});
  /** The details window, while open. */
  edit = $state<{ paths: string[]; songs: SongMatch[] } | null>(null);
  /** Albums in the review window (indexes); null when closed. */
  review = $state<number[] | null>(null);
  progress = $state<{ done: number; total: number; kind: "search" | "listen" | "save" | "tags" } | null>(null);
  /** Albums whose songs keep their names but still get a cover: those
   *  whose folder has none yet (asked of the disk; an existing cover is
   *  kept when writing). */
  coverOnly = $state(0);
  /** Album folders whose songs keep their names but still get a playlist
   *  (asked of the disk: those without one and with two songs or more). */
  playlistOnly = $state<string[]>([]);

  constructor(ctx: PageCtx) {
    this.#ctx = ctx;
    $effect(() => {
      const r = this.rule?.rule;
      const on = r?.type === "music" && r.cover;
      const items = this.#ctx.items;
      const status = new Map(items.map((i) => [i.path, i.status]));
      const dirs = on
        ? [...new Set(this.albums.filter((a) => a.choice && a.files.every((p) => status.get(p) === "unchanged")).map((a) => a.dir))]
        : [];
      if (!dirs.length) {
        this.coverOnly = 0;
        return;
      }
      api.coverDirs(dirs).then((d) => {
        if (items === this.#ctx.items) this.coverOnly = d.length;
      }, () => {});
    });
    $effect(() => {
      const r = this.rule?.rule;
      const on = r?.type === "music" && r.playlist;
      const items = this.#ctx.items;
      const changed = new Set(items.filter((i) => i.status !== "unchanged").map((i) => i.dir));
      const dirs = on
        ? [...new Set(items.filter((i) => !i.isDir && i.status === "unchanged" && isAudio(i.original) && !changed.has(i.dir)).map((i) => i.dir))]
        : [];
      if (!dirs.length) {
        this.playlistOnly = [];
        return;
      }
      api.playlistDirs(dirs).then((d) => {
        if (items === this.#ctx.items) this.playlistOnly = d;
      }, () => {});
    });
  }

  rule = $derived.by(() => ruleOf(this.#ctx.steps, "music"));
  /** Songs the rule leaves as they are (no title in their tags). */
  untagged = $derived.by(() => (this.rule ? this.#ctx.items.filter((i) => i.untagged).length : 0));
  /** Each song's details: from its chosen release, or typed in. */
  matches = $derived.by(() => ({
    ...(Object.fromEntries(this.albums.flatMap((a) => (a.choice ? Object.entries(a.matches) : []))) as Record<string, SongMatch>),
    ...this.edits,
  }));
  /** The album each song is in. */
  albumOf = $derived.by(() => new Map(this.albums.flatMap((a, i) => a.files.map((p): [string, number] => [p, i]))));
  /** The songs among the rows chosen in the list. */
  selected = $derived.by(() => this.#ctx.items.filter((i) => this.#ctx.selected.has(i.path) && !i.isDir && isAudio(i.original)));
  /** Songs whose names are already right but whose tags get written. */
  tagsOnly = $derived.by(() => {
    const r = this.rule?.rule;
    return r?.type === "music" && r.writeTags
      ? this.#ctx.items.filter((i) => i.status === "unchanged" && (i.retag || (r.cleanTags && i.junk)) && this.matches[i.path])
      : [];
  });
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
  fields = $derived<EditField[]>([
    { key: "title", label: t("songEditTitle"), wide: true },
    { key: "artist", label: t("songEditArtist") },
    { key: "albumArtist", label: t("songEditAlbumArtist") },
    { key: "genre", label: t("songEditGenre") },
    { key: "album", label: t("songEditAlbum"), wide: true },
    { key: "year", label: t("songEditYear"), numeric: true },
    { key: "track", label: t("songEditTrack"), numeric: true },
    { key: "trackTotal", label: t("songEditTrackTotal"), numeric: true },
    { key: "disc", label: t("songEditDisc"), numeric: true },
    { key: "discTotal", label: t("songEditDiscTotal"), numeric: true },
  ]);

  /** Looks up the visible songs' albums on MusicBrainz; albums already
   *  settled keep their release, doubtful ones go to the review window. */
  async search(rule: Extract<Rule, { type: "music" }>) {
    const settled = new Set(this.albums.filter((a) => a.status === "auto" || a.status === "chosen").flatMap((a) => a.files));
    const paths = this.#ctx.items
      // Songs without a title can still be identified by their sound.
      .filter((i) => !i.isDir && (!i.untagged || rule.listen) && isAudio(i.original) && !settled.has(i.path))
      .map((i) => i.path);
    if (!paths.length) {
      this.#ctx.say(t("mbNoFiles"));
      return;
    }
    this.progress = { done: 0, total: 0, kind: "search" };
    try {
      const found = await api.mbLookup(paths, rule.fromName, rule.listen);
      if (!found.length) {
        this.#ctx.say(t("mbNoFiles"));
        return;
      }
      const failed = found.find((f) => f.error);
      if (failed && found.every((f) => f.error)) {
        await notify({ title: t("rule.music"), text: tError(failed.error) });
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
          status: f.choice ? "auto" : f.candidates.length ? "choose" : "none",
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
    const pending = [
      ...this.albums.flatMap((a, i) => (a.status === "choose" ? [i] : [])),
      ...this.albums.flatMap((a, i) => (a.status === "none" ? [i] : [])),
    ];
    const queue = start === undefined ? pending : [start, ...pending.filter((i) => i !== start)];
    this.review = queue.length ? queue : null;
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

  /** Where each album with a release ends up after renaming, for its cover. */
  coverItems(r: ApplyReport): { dir: string; release: string }[] {
    const to = new Map(r.done.map((op) => [op.from, op.to]));
    const parent = (p: string) => p.slice(0, Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\")));
    const items = new Map<string, string>();
    for (const a of this.albums) {
      if (!a.choice || !a.files.some((p) => this.#ctx.items.some((i) => i.path === p) || to.has(p))) continue;
      items.set(parent(to.get(a.files[0]) ?? a.files[0]), a.choice.id);
    }
    return [...items].map(([dir, release]) => ({ dir, release }));
  }

  /** Opens the details window on the songs chosen in the list. */
  async openEdit() {
    const rows = this.selected;
    if (!rows.length) return;
    const r = this.rule?.rule;
    const missing = rows.filter((i) => !this.matches[i.path]);
    const own: Record<string, SongMatch | null> = {};
    if (missing.length) {
      const got = await api.songTags(missing.map((i) => i.path), r?.type === "music" && r.fromName);
      missing.forEach((i, n) => (own[i.path] = got[n]));
    }
    this.edit = {
      paths: rows.map((i) => i.path),
      songs: rows.map((i) => $state.snapshot(this.matches[i.path] ?? own[i.path] ?? EMPTY)),
    };
  }

  /** The fields changed in the details window, for each of its songs. */
  saveEdit(changed: Record<string, string>) {
    const edit = this.edit;
    if (!edit) return;
    const next = { ...this.edits };
    edit.paths.forEach((p, i) => {
      const m: SongMatch = { ...edit.songs[i] };
      for (const [k, v] of Object.entries(changed)) {
        if (k === "year") m.year = v ? Number.parseInt(v, 10) : null;
        else if (k === "track" || k === "trackTotal" || k === "disc" || k === "discTotal") m[k] = v ? Number.parseInt(v, 10) : 0;
        else (m as unknown as Record<string, string>)[k] = v;
      }
      // Another album: the release's ids no longer tell it.
      if (m.typed || !m.release || changed.album !== undefined || changed.albumArtist !== undefined) {
        Object.assign(m, { typed: true, release: "", recording: "", trackId: "", releaseGroup: "", artistIds: [], albumArtistIds: [] });
      }
      next[p] = m;
    });
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
