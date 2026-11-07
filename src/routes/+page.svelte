<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "$lib/api";
  import { authorSpellings } from "$lib/authors";
  import Dialog from "$lib/Dialog.svelte";
  import { closeDialog, confirm, dialog, notify } from "$lib/dialogs.svelte";
  import FileTable from "$lib/FileTable.svelte";
  import Help from "$lib/Help.svelte";
  import AlbumReview from "$lib/AlbumReview.svelte";
  import BookReview from "$lib/BookReview.svelte";
  import DetailsEdit from "$lib/DetailsEdit.svelte";
  import ComicPicker from "$lib/ComicPicker.svelte";
  import PictureCompare from "$lib/PictureCompare.svelte";
  import ComicReview from "$lib/ComicReview.svelte";
  import EpisodeReview from "$lib/EpisodeReview.svelte";
  import { missingBooks, missingEpisodes, missingFolder, missingIssues, missingMovies, missingTracks, today, type Ghost } from "$lib/missing";
  import MovieReview from "$lib/MovieReview.svelte";
  import Options from "$lib/Options.svelte";
  import RuleCard from "$lib/RuleCard.svelte";
  import ShowPicker from "$lib/ShowPicker.svelte";
  import SubtitleReview from "$lib/SubtitleReview.svelte";
  import { LOCALES, MOD, t, tError, type Key } from "$lib/i18n";
  import { RULE_TYPES, LOOKUP_TYPES, isLookup, oneLookup, byFit, isAudio, isBook, isComic, isScreen, isVideo, newRule, newStep, seriesOnly, splitExt, defaultSubLangs } from "$lib/rules";
  import { comicKey, loadSecrets, loadSettings, saveSecrets, saveSettings, settings, tmdbLang } from "$lib/settings.svelte";
  import type {
    AlbumState, ApplyReport, BookCandidate, BookMatch, BookState, Candidate, ComicIssue, ComicMatch, ComicSource, ComicState, ComicVolume, Copies, EditField, MbCandidate, SongMatch, EpKey, EpisodeMatch, EpisodeState, MovieMatch, MovieSet, MovieState, MovieStatus, HistoryEntry,
    OpError, Preview, PreviewItem, Rule, RuleType, Show, Step, SubCandidate, SubsAuth, View, ViewSort,
  } from "$lib/types";
  import { DETAIL_COLUMNS, type DetailColumn } from "$lib/types";
  import { COL_LABEL, detailText } from "$lib/details";

  let ready = $state(false);
  let steps = $state<Step[]>([]);
  let overrides = $state<Record<string, string>>({});
  let preview = $state<Preview>({ items: [], ruleErrors: [], filterError: null, total: 0, changed: 0, conflicts: 0, leftFolders: [] });
  let selected = $state(new Set<string>());
  let filter = $state("");
  let sortKey = $state<ViewSort>("newName");
  let sortDesc = $state(false);
  let notice = $state("");
  /** A short note that the work is done, in a corner; it goes by itself. */
  let toast = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function done(text: string) {
    clearTimeout(toastTimer);
    toast = text;
    toastTimer = setTimeout(() => (toast = null), 4000);
  }
  let busy = $state(false);
  /** What `busy` is doing, for the indicator over the list. */
  let task = $state<"add" | "rename" | "undo" | "sort" | null>(null);
  let dragging = $state(false);
  let history = $state<HistoryEntry[] | null>(null);
  let showOptions = $state(false);
  let help = $state<"manual" | "about" | "licenses" | null>(null);
  /** Identical copies of each file in the list. */
  let copies = $state<Record<string, Copies>>({});
  let identicalOnly = $state(false);
  /** Songs that are the same music as another (other files, not copies). */
  let alike = $state<Record<string, Copies>>({});
  let alikeOnly = $state(false);
  const alikeCount = $derived(Object.keys(alike).length);
  /** Pictures that look like another in the list. */
  let similar = $state<Record<string, Copies>>({});
  let similarOnly = $state(false);
  const similarCount = $derived(Object.keys(similar).length);
  /** The groups, in order, for the comparison window. */
  let copyGroups = $state<string[][]>([]);
  let similarGroups = $state<string[][]>([]);
  /** The comparison window, when open. */
  let compare = $state<{ kind: "similar" | "identical"; start: number } | null>(null);
  /** Share of the new pictures read while comparing them (null: not). */
  let similarProgress = $state<number | null>(null);
  /** Search progress (0–1) while reading whole files; null when idle. */
  let identicalProgress = $state<number | null>(null);
  /** How far the photo rule's first reading of the files is. */
  let photosProgress = $state<number | null>(null);
  const identicalCount = $derived(Object.keys(copies).length);

  /** TMDB results and choices per file, for the movie rule. */
  let movies = $state<Record<string, MovieState>>({});
  /** Files in the movie review window; null when closed. */
  let review = $state<string[] | null>(null);
  let tmdbProgress = $state<{ done: number; total: number; kind: "search" | "save" | "saveTv" } | null>(null);
  const movieRule = $derived(steps.find((s) => s.enabled && s.rule.type === "movie"));
  /** Each chosen movie's template fields (genre, director…), by id. */
  let movieExtra = $state<Record<number, Record<string, string>>>({});
  const movieMatches = $derived(
    Object.fromEntries(
      Object.entries(movies)
        .filter(([, m]) => m.choice)
        .map(([p, m]): [string, MovieMatch] => {
          const c = m.choice!;
          return [p, { id: c.id, title: c.title, originalTitle: c.originalTitle, year: c.year, extra: movieExtra[c.id] }];
        }),
    ),
  );
  /** Movies whose names are already right but that still get .nfo and
   *  images (files that already exist are skipped when writing). */
  const metaOnly = $derived(
    movieRule?.rule.type === "movie" && movieRule.rule.sidecar
      ? preview.items.filter((i) => i.status === "unchanged" && movies[i.path]?.choice && isVideo(i.original))
      : [],
  );
  const movieCounts = $derived(counts(movies));
  /** Each chosen movie's TMDB collection (null: none), and the collections,
   *  for the movies missing from them. */
  let setOf = $state<Record<number, number | null>>({});
  let movieSets = $state<Record<number, MovieSet>>({});
  /** Movies already asked about (an answer may not come). */
  const setAsked = new Set<string>();
  $effect(() => {
    if (movieRule?.rule.type !== "movie" || !settings.tmdbKey) return;
    const sets = movieRule.rule.missing;
    // Read without tracking the fields: they come from this.
    const ids = [...new Set(Object.values(movies).flatMap((m) => (m.choice ? [m.choice.id] : [])))].filter(
      (id) => !setAsked.has(`${id}:${sets}`) && !(sets ? false : setAsked.has(`${id}:true`)),
    );
    if (!ids.length) return;
    for (const id of ids) setAsked.add(`${id}:${sets}`);
    api.tmdbCollections(settings.tmdbKey, tmdbLang(), ids, sets).then(
      (r) => {
        for (const [id, set] of r.of) setOf[id] = set;
        for (const set of r.sets) movieSets[set.id] = set;
        for (const [id, extra] of r.extras) movieExtra[id] = extra;
      },
      () => {},
    );
  });

  /** Open Library results and choices per file, for the book rule. */
  let books = $state<Record<string, BookState>>({});
  /** Files in the book review window; null when closed. */
  let bookReview = $state<string[] | null>(null);
  let bookProgress = $state<{ done: number; total: number; kind: "search" | "save" } | null>(null);
  /** Details being written inside comics and books. */
  let insideProgress = $state<{ done: number; total: number } | null>(null);
  /** A long operation under way: the list waits for it, with what it is
   *  doing and how far it got (no count: still finding out). Searches can
   *  be stopped (`stop`). */
  const working = $derived.by((): { label: string; done?: number; total?: number; stop?: boolean } | null => {
    const counted = (label: string, p: { done: number; total: number }, stop: boolean) => ({ label, done: p.done, total: p.total, stop });
    if (insideProgress) return counted(t("insideWriting", insideProgress), insideProgress, false);
    if (subsProgress)
      return counted(t(subsProgress.kind === "search" ? "subsSearching" : "subsWriting", subsProgress), subsProgress, true);
    if (bookProgress)
      return counted(t(bookProgress.kind === "search" ? "olSearching" : "sidecarsWriting", bookProgress), bookProgress, bookProgress.kind === "search");
    if (tmdbProgress)
      return counted(t(tmdbProgress.kind === "search" ? "tmdbSearching" : "sidecarsWriting", tmdbProgress), tmdbProgress, tmdbProgress.kind === "search");
    if (mbProgress)
      return counted(
        t({ search: "mbSearching", listen: "mbListening", save: "coversWriting", tags: "tagsWriting" }[mbProgress.kind] as "mbSearching", mbProgress),
        mbProgress,
        mbProgress.kind === "search" || mbProgress.kind === "listen",
      );
    if (tvProgress) return { label: tvProgress, stop: true };
    if (comicProgress && !comicPick) return { label: comicProgress, stop: true };
    if (task === "rename" && applyProgress?.total)
      return {
        label: t(sendTo?.copy ? "workingCopy" : "workingMove", { p: Math.floor((applyProgress.done / applyProgress.total) * 100) }),
        done: applyProgress.done,
        total: applyProgress.total,
        stop: true,
      };
    if (task) return { label: t({ add: "workingAdd", rename: "workingRename", undo: "workingUndo", sort: "workingSort" }[task] as "workingAdd") };
    return null;
  });
  /** Bumped by Stop: a search loop that sees a newer number stops. */
  let stopRun = 0;
  let stopping = $state(false);
  function stopSearch() {
    stopRun++;
    stopping = true;
    api.stopSearch();
  }
  // Once the stopped search is over, say what became of it.
  $effect(() => {
    if (!working && stopping) {
      stopping = false;
      done(t("searchStopped"));
    }
  });
  const bookRule = $derived(steps.find((s) => s.enabled && s.rule.type === "book"));
  /** What the rule uses of each chosen book: the edition's publisher and
   *  ISBN only when it was found by the file's ISBN (otherwise they would
   *  be another edition's), else the file's own. */
  function bookMatch(b: BookState): BookMatch {
    const c = b.choice!;
    return {
      work: c.work, title: c.title, originalTitle: c.originalTitle, authors: c.authors, year: c.year,
      editionYear: c.exact ? c.editionYear : null,
      publisher: c.exact ? c.publisher : b.guess.publisher,
      isbn: c.exact ? c.isbn : (b.guess.isbns[0] ?? ""),
      series: b.guess.series, seriesIndex: b.guess.seriesIndex,
      extra: Object.fromEntries(
        [["language", c.exact ? c.language : b.guess.language || c.language], ["pages", c.pages ? String(c.pages) : ""]].filter(([, v]) => v),
      ),
    };
  }
  const bookMatches = $derived.by(() => {
    const raw = Object.entries(books).filter(([, b]) => b.choice).map(([p, b]): [string, BookMatch] => [p, bookMatch(b)]);
    const spelling = authorSpellings(raw.flatMap(([, m]) => m.authors));
    const same = (a: string) => spelling.get(a.normalize("NFC")) ?? a.normalize("NFC");
    return Object.fromEntries(
      raw.map(([p, m]) => [p, { ...m, title: m.title.normalize("NFC"), authors: [...new Set(m.authors.map(same))] }]),
    );
  });
  const bookCounts = $derived(counts(books));
  /** Books whose names are already right but that still get a cover or a
   *  metadata.opf (files that already exist are skipped when writing). */
  const bookMetaOnly = $derived(
    bookRule?.rule.type === "book" && (bookRule.rule.cover || bookRule.rule.opf)
      ? preview.items.filter((i) => i.status === "unchanged" && books[i.path]?.choice && isBook(i.original))
      : [],
  );
  const extOf = (p: string) => splitExt(p.split(/[\\/]/).pop() ?? p)[1].toLowerCase();
  const isEpub = (p: string) => ["epub", "kepub"].includes(extOf(p));
  /** EPUBs whose names are already right but whose details get written
   *  (those that already have them are skipped when writing). */
  const epubOnly = $derived(
    bookRule?.rule.type === "book" && bookRule.rule.writeEpub
      ? preview.items.filter((i) => i.status === "unchanged" && books[i.path]?.choice && isEpub(i.original))
      : [],
  );
  /** What writing a book's details inside it needs. */
  function bookInsideItem(key: string, path: string) {
    const c = books[key].choice!;
    return { path, book: bookMatches[key], language: c.exact ? c.language : books[key].guess.language };
  }
  /** What the cover and metadata.opf of a book need; `key` is the path before renaming. */
  function bookSidecarItem(key: string, path: string) {
    const c = books[key].choice!;
    return { path, book: bookMatches[key], cover: c.cover, coverUrl: c.coverUrl ?? "", description: c.description ?? "", language: c.exact ? c.language : books[key].guess.language };
  }

  /** Comic Vine issues per file, for the comic rule. */
  let comics = $state<Record<string, ComicState>>({});
  /** Series fetched with their issues, by `source:id`. */
  let comicVolumes = $state<Record<string, { volume: ComicVolume; issues: ComicIssue[] }>>({});
  /** The series picked for each search text (typed, or the files' series). */
  let pickedComics = $state<Record<string, ComicVolume>>({});
  /** Files in the comic review window; null when closed. */
  let comicReview = $state<string[] | null>(null);
  /** The series picker, while open. */
  let comicPick = $state<{ source: ComicSource; skip: string; query: string; files: number; highest: number; candidates: ComicVolume[]; path?: string; comic?: ComicMatch; resolve: (v: ComicVolume | null) => void } | null>(null);
  /** Whether this session was told that the GCD works better with an account. */
  let gcdWarned = false;
  let comicProgress = $state<string | null>(null);
  const comicRule = $derived(steps.find((s) => s.enabled && s.rule.type === "comic"));
  /** A comic named after its issue: series, number, title, year and
   *  publisher from Comic Vine; volume and chapter from the file (as the
   *  Rust core's `with_issue`). */
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
  /** Comics whose details were typed in (over what the catalogue said). */
  let comicEdits = $state<Record<string, ComicMatch>>({});
  /** The comic details window, while open. */
  let comicEdit = $state<{ paths: string[]; comics: ComicMatch[] } | null>(null);
  const comicMatches = $derived({
    ...Object.fromEntries(
      Object.entries(comics).flatMap(([p, c]): [string, ComicMatch][] => {
        const v = comicVolumes[c.volumeKey];
        if (!v || c.choice == null) return [];
        // "": the series alone (MangaDex).
        const issue = c.choice === "" ? undefined : v.issues.find((i) => i.id === c.choice);
        return c.choice === "" || issue ? [[p, withIssue(c.own, v.volume, issue)]] : [];
      }),
    ),
    ...comicEdits,
  });
  /** The comics among the rows chosen in the list. */
  const selectedComics = $derived(preview.items.filter((i) => selected.has(i.path) && (i.isDir ? i.convert : isComic(i.original))));

  async function openComicEdit() {
    const rows = selectedComics;
    if (!rows.length) return;
    const r = comicRule?.rule;
    const missing = rows.filter((i) => !comicMatches[i.path] && !comics[i.path]);
    const guessed: Record<string, ComicMatch | null> = {};
    if (missing.length) {
      const got = await api.comicGuesses(
        missing.map((i) => i.path),
        missing.map((i) => (i.lookupName ? (i.isDir ? i.lookupName : splitExt(i.lookupName)[0]) : null)),
        r?.type === "comic" && !!r.byFolder,
      );
      missing.forEach((i, n) => (guessed[i.path] = got[n]));
    }
    const empty: ComicMatch = { series: "", seriesYear: null, number: "", volume: null, chapter: "", title: "", year: null, count: null, publisher: "", writer: "", artist: "", id: "" };
    comicEdit = {
      paths: rows.map((i) => i.path),
      comics: rows.map((i) => $state.snapshot(comicMatches[i.path] ?? comics[i.path]?.own ?? guessed[i.path] ?? empty)),
    };
  }
  /** The comic details window's fields. */
  const COMIC_FIELDS = $derived<EditField[]>([
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
  /** Values as the details window shows them (nothing: empty). */
  const asText = (m: object, keys: EditField[]) =>
    Object.fromEntries(keys.map((f) => {
      const v = (m as Record<string, unknown>)[f.key];
      return [f.key, v == null || v === 0 ? "" : String(v)];
    }));

  function saveComicEdit(changed: Record<string, string>) {
    if (!comicEdit) return;
    const num = (v: string) => (v ? Number.parseInt(v, 10) : null);
    const next = { ...comicEdits };
    comicEdit.paths.forEach((p, i) => {
      const m: ComicMatch = { ...comicEdit!.comics[i] };
      for (const [k, v] of Object.entries(changed)) {
        if (k === "seriesYear" || k === "volume" || k === "year") m[k] = num(v);
        else (m as unknown as Record<string, string>)[k] = v;
      }
      // The catalogue's issue no longer tells it.
      if (changed.series !== undefined || changed.number !== undefined) m.id = "";
      next[p] = m;
    });
    comicEdits = next;
    comicEdit = null;
  }
  const comicCounts = $derived(counts(comics));
  /** Comics whose names are already right but that get converted to CBZ
   *  (a RAR named `.cbz`), or whose ComicInfo.xml says something else. */
  const comicInfoOnly = $derived.by(() => {
    const r = comicRule?.rule;
    if (r?.type !== "comic") return [];
    return preview.items.filter(
      (i) =>
        i.status === "unchanged" &&
        ((r.convert && i.convert) ||
          (extOf(i.original) === "cbz" && ((r.writeInfo && (i.retag || (r.cleanInfo && i.junk)) && comicMatches[i.path]) || clearsInfo(r, i.junk)))),
    );
  });
  /** A CBZ whose ComicInfo.xml the comic rule takes out: with "clear", the
   *  ones it does not write (without "write", every one; with it, none:
   *  an unmatched comic's details are what name it). */
  function clearsInfo(r: Rule, junk: boolean) {
    return r.type === "comic" && !!r.cleanInfo && junk && !r.writeInfo;
  }
  /** What converting a comic to CBZ and writing its ComicInfo.xml need: the
   *  original's fate, its match and its page in the catalogue. */
  function comicInsideItem(key: string, path: string, convert: boolean) {
    const r = comicRule?.rule;
    const original = r?.type === "comic" && r.convert && convert ? (r.original ?? "keep") : undefined;
    const pdf = r?.type === "comic" ? { draw: r.drawPages !== false, size: r.pageSize ?? 2400, info: !!r.writeInfo } : undefined;
    const fresh = r?.type === "comic" && !!r.cleanInfo;
    // Typed in: no page in a catalogue.
    if (r?.type === "comic" && r.writeInfo && comicEdits[key]) return { path, comic: comicEdits[key], web: "", manga: false, convert: original, pdf, fresh };
    if (!(r?.type === "comic" && r.writeInfo && comicMatches[key] && comics[key])) return { path, convert: original, pdf, fresh };
    const c = comics[key];
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
    return { path, comic: comicMatches[key], web, manga: seriesOnly(source), convert: original, pdf, fresh };
  }
  /** TMDB episodes per file, for the series rule. */
  let series = $state<Record<string, EpisodeState>>({});
  /** Shows fetched, by id. */
  let shows = $state<Record<number, Show>>({});
  /** The shows picked, by the text they were found by (not asked again). */
  let pickedShows = $state<Record<string, Candidate>>({});
  /** The shows of the last search. */
  let searchedShows = $state<Candidate[]>([]);
  /** Files in the episode review window; null when closed. */
  let episodeReview = $state<string[] | null>(null);
  /** The show picker, while open. */
  let showPick = $state<{ source: "tmdb" | "anilist"; query: string; candidates: Candidate[]; resolve: (c: Candidate | null) => void } | null>(null);
  let tvProgress = $state<string | null>(null);
  const seriesRule = $derived(steps.find((s) => s.enabled && s.rule.type === "series"));
  const musicRule = $derived(steps.find((s) => s.enabled && s.rule.type === "music"));
  const photoRule = $derived(steps.find((s) => s.enabled && s.rule.type === "photo"));
  /** Photos and videos the photo rule leaves as they are (no date). */
  const undatedCount = $derived(photoRule ? preview.items.filter((i) => i.photoDate === "none").length : 0);
  const untaggedCount = $derived(musicRule ? preview.items.filter((i) => i.untagged).length : 0);

  /** Albums looked up on MusicBrainz, for the music rule. */
  let albums = $state<AlbumState[]>([]);
  /** Albums in the review window (indexes); null when closed. */
  let albumReview = $state<number[] | null>(null);
  let mbProgress = $state<{ done: number; total: number; kind: "search" | "listen" | "save" | "tags" } | null>(null);
  /** Songs whose tags were typed in (over what the release said). */
  let songEdits = $state<Record<string, SongMatch>>({});
  /** Each song's details from its chosen release. */
  const songMatches = $derived({
    ...(Object.fromEntries(albums.flatMap((a) => (a.choice ? Object.entries(a.matches) : []))) as Record<string, SongMatch>),
    ...songEdits,
  });
  /** The song details window, while open. */
  let songEdit = $state<{ paths: string[]; songs: SongMatch[] } | null>(null);
  /** The songs among the rows chosen in the list. */
  const selectedSongs = $derived(preview.items.filter((i) => selected.has(i.path) && !i.isDir && isAudio(i.original)));
  const SONG_FIELDS = $derived<EditField[]>([
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

  async function openSongEdit() {
    const rows = selectedSongs;
    if (!rows.length) return;
    const r = musicRule?.rule;
    const missing = rows.filter((i) => !songMatches[i.path]);
    const own: Record<string, SongMatch | null> = {};
    if (missing.length) {
      const got = await api.songTags(missing.map((i) => i.path), r?.type === "music" && r.fromName);
      missing.forEach((i, n) => (own[i.path] = got[n]));
    }
    const empty: SongMatch = { release: "", title: "", artist: "", albumArtist: "", album: "", year: null, track: 0, trackTotal: 0, disc: 0, discTotal: 0, typed: true };
    songEdit = {
      paths: rows.map((i) => i.path),
      songs: rows.map((i) => $state.snapshot(songMatches[i.path] ?? own[i.path] ?? empty)),
    };
  }

  function saveSongEdit(changed: Record<string, string>) {
    if (!songEdit) return;
    const next = { ...songEdits };
    songEdit.paths.forEach((p, i) => {
      const m: SongMatch = { ...songEdit!.songs[i] };
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
    songEdits = next;
    songEdit = null;
  }
  /** The album each song is in. */
  const albumOf = $derived(new Map(albums.flatMap((a, i) => a.files.map((p): [string, number] => [p, i]))));
  /** Albums whose songs keep their names but still get a cover: those
   *  whose folder has none yet (asked of the disk; an existing cover is
   *  kept when writing). */
  let coverOnly = $state(0);
  $effect(() => {
    const on = musicRule?.rule.type === "music" && musicRule.rule.cover;
    const items = preview.items;
    const status = new Map(items.map((i) => [i.path, i.status]));
    const dirs = on
      ? [...new Set(albums.filter((a) => a.choice && a.files.every((p) => status.get(p) === "unchanged")).map((a) => a.dir))]
      : [];
    if (!dirs.length) {
      coverOnly = 0;
      return;
    }
    api.coverDirs(dirs).then((d) => {
      if (items === preview.items) coverOnly = d.length;
    }, () => {});
  });
  /** Album folders whose songs keep their names but still get a playlist
   *  (asked of the disk: those without one and with two songs or more). */
  let playlistOnly = $state<string[]>([]);
  /** Songs whose names are already right but whose tags get written. */
  const tagsOnly = $derived(
    musicRule?.rule.type === "music" && musicRule.rule.writeTags
      ? preview.items.filter(
          (i) => i.status === "unchanged" && (i.retag || (musicRule.rule.type === "music" && musicRule.rule.cleanTags && i.junk)) && songMatches[i.path],
        )
      : [],
  );
  $effect(() => {
    const on = musicRule?.rule.type === "music" && musicRule.rule.playlist;
    const items = preview.items;
    const changed = new Set(items.filter((i) => i.status !== "unchanged").map((i) => i.dir));
    const dirs = on
      ? [...new Set(items.filter((i) => !i.isDir && i.status === "unchanged" && isAudio(i.original) && !changed.has(i.dir)).map((i) => i.dir))]
      : [];
    if (!dirs.length) {
      playlistOnly = [];
      return;
    }
    api.playlistDirs(dirs).then((d) => {
      if (items === preview.items) playlistOnly = d;
    }, () => {});
  });
  const albumCounts = $derived.by(() => {
    let found = 0, choose = 0, none = 0;
    for (const a of albums) {
      if (a.status === "auto" || a.status === "chosen") found++;
      else if (a.status === "choose") choose++;
      else none++;
    }
    return { found, choose, none };
  });
  /** An episode's template fields: the show's, its air date and its
   *  number counted from the first episode (specials apart). */
  function episodeExtra(show: Show, season: number, number: number): Record<string, string> {
    const extra: Record<string, string> = { ...(show.extra ?? {}) };
    const ep = show.episodes.find((x) => x.season === season && x.number === number);
    if (ep?.airDate) extra.date = ep.airDate;
    if (season > 0 && ep) {
      extra.absolute = String(show.episodes.filter((x) => x.season > 0 && (x.season < season || (x.season === season && x.number <= number))).length);
    }
    return extra;
  }
  const episodeMatches = $derived(
    Object.fromEntries(
      Object.entries(series)
        .filter(([, e]) => e.choice && shows[e.showId])
        .map(([p, e]): [string, EpisodeMatch] => {
          const show = shows[e.showId];
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
  const seriesCounts = $derived(counts(series));

  /** Subtitles found per video (by its path): the languages asked for it
   *  has already, the candidates best first, and the one chosen per
   *  language (an index in `candidates`; -1: none). `key` tells what was
   *  searched (movie or episode, languages), to search again on a change. */
  type SubsState = { key: string; have: string[]; candidates: SubCandidate[]; pick: Record<string, number> };
  let subsFound = $state<Record<string, SubsState>>({});
  /** The languages of the last search, as understood (`pt-BR`, `en`). */
  let subsLangs = $state<string[]>([]);
  let subsProgress = $state<{ done: number; total: number; kind: "search" | "save" } | null>(null);
  let subsReview = $state(false);
  /** OpenSubtitles downloads left today, when known. */
  let subsRemaining = $state<number | null>(null);
  /** The languages asked for by the movie or TV show rule, when it
   *  downloads subtitles. */
  const subsWanted = $derived.by(() => {
    const r = movieRule?.rule.type === "movie" ? movieRule.rule : seriesRule?.rule.type === "series" && seriesRule.rule.source !== "anilist" ? seriesRule.rule : null;
    return r?.subtitles ? (r.subLangs ?? defaultSubLangs()) : null;
  });
  const subsAuth = (): SubsAuth => ({
    osKey: settings.osKey.trim(),
    osUser: settings.osUser.trim(),
    osPassword: settings.osPassword,
    subdlKey: settings.subdlKey.trim(),
  });
  /** The visible videos identified, with what to look their subtitles up by. */
  function subsItems() {
    const out: { path: string; tmdb: number; season?: number; episode?: number }[] = [];
    for (const i of preview.items) {
      if (i.isDir || !isVideo(i.original)) continue;
      const m = movieRule ? movies[i.path]?.choice : null;
      const e = seriesRule ? episodeMatches[i.path] : null;
      if (m) out.push({ path: i.path, tmdb: m.id });
      else if (e && !e.source && e.episodes.length === 1) out.push({ path: i.path, tmdb: e.id, season: e.season, episode: e.episodes[0] });
    }
    return out;
  }
  const subsKey = (it: { tmdb: number; season?: number; episode?: number }, langs: string) =>
    `${it.tmdb}:${it.season ?? ""}:${it.episode ?? ""}:${langs}`;
  /** Subtitles chosen per video, not there yet. */
  const subsChosen = (s: SubsState | undefined) => (s ? Object.entries(s.pick).filter(([l, i]) => i >= 0 && !s.have.includes(l)).length : 0);
  const subsCounts = $derived.by(() => {
    const visible = new Set(preview.items.map((i) => i.path));
    let subs = 0;
    let videos = 0;
    let missing = 0;
    for (const [p, s] of Object.entries(subsFound)) {
      if (!visible.has(p)) continue;
      const n = subsChosen(s);
      subs += n;
      if (n) videos++;
      if (subsLangs.some((l) => !s.have.includes(l) && !(s.pick[l] >= 0))) missing++;
    }
    return { subs, videos, missing };
  });

  /** Searches subtitles for the visible videos identified (those searched
   *  already for the same languages are kept, unless `again`). */
  async function searchSubs(again = false) {
    const langs = subsWanted;
    if (langs === null) return;
    const auth = subsAuth();
    if (!auth.osKey && !auth.subdlKey) {
      const ok = await confirm({ title: t("subsTitle"), text: t("subsNeedKey"), ok: t("options"), cancel: t("cancel") });
      if (ok) showOptions = true;
      return;
    }
    const items = subsItems().filter((it) => again || subsFound[it.path]?.key !== subsKey(it, langs));
    if (!items.length) {
      if (!subsItems().length) notice = t("subsNoVideos");
      else if (subsCounts.videos) subsReview = true;
      return;
    }
    subsProgress = { done: 0, total: items.length, kind: "search" };
    try {
      const r = await api.subsSearch(auth, langs, items);
      subsLangs = r.langs;
      if (r.remaining !== null) subsRemaining = r.remaining;
      const asked = new Map(items.map((it) => [it.path, it]));
      for (const f of r.found) {
        const pick: Record<string, number> = {};
        f.candidates.forEach((c, i) => {
          if (!(c.lang in pick)) pick[c.lang] = i;
        });
        subsFound[f.path] = { key: subsKey(asked.get(f.path)!, langs), have: f.have, candidates: f.candidates, pick };
      }
      if (r.errors.length) await notify({ title: t("subsTitle"), text: r.errors.map(tError).join("\n") });
      else if (r.found.some((f) => f.candidates.length)) subsReview = true;
    } catch (e) {
      await notify({ title: t("subsTitle"), text: tError(e) });
    } finally {
      subsProgress = null;
    }
  }

  /** Videos whose names are already right but that still get subtitles. */
  const subsOnly = $derived(
    subsWanted === null ? [] : preview.items.filter((i) => i.status === "unchanged" && subsChosen(subsFound[i.path]) > 0),
  );

  /** What to download for renamed videos (`from` → `to`): per language, the
   *  one chosen first and the others of that language after it. */
  function subsWrites(pairs: [string, string][]) {
    return pairs.flatMap(([from, to]) => {
      const s = subsFound[from];
      if (!s) return [];
      const langs = Object.entries(s.pick)
        .filter(([l, i]) => i >= 0 && !s.have.includes(l))
        .map(([l, i]) => [s.candidates[i], ...s.candidates.filter((c, j) => j !== i && c.lang === l)]);
      if (!langs.length) return [];
      const e = episodeMatches[from];
      return [{ path: to, season: e?.season, episode: e?.episodes[0], langs }];
    });
  }
  /** Aired episodes missing from the seasons the listed videos cover,
   *  issues from the series the listed comics are from, tracks from the
   *  albums, released movies from the collections and volumes between a
   *  book series' files. */
  const ghosts = $derived(unique([
    ...(seriesRule?.rule.type === "series" && seriesRule.rule.missing
      ? missingEpisodes(shows, episodeMatches, preview.items, sortDesc, today())
      : []),
    ...(comicRule?.rule.type === "comic" && comicRule.rule.missing
      ? missingIssues(comicVolumes, comics, preview.items, sortDesc, today())
      : []),
    ...(musicRule?.rule.type === "music" && musicRule.rule.missing ? missingTracks(albums, preview.items, sortDesc) : []),
    ...(movieRule?.rule.type === "movie" && movieRule.rule.missing
      ? missingMovies(movieSets, setOf, Object.fromEntries(Object.entries(movieMatches).map(([p, m]) => [p, m.id])), preview.items, sortDesc, today())
      : []),
    ...(bookRule?.rule.type === "book" && bookRule.rule.missing ? missingBooks(books, preview.items, sortDesc) : []),
  ]));
  /** Each missing row once: the list is keyed by them, and a repeated key
   *  stops it from being drawn at all. */
  function unique<T extends { key: string }>(rows: T[]): T[] {
    const seen = new Set<string>();
    return rows.filter((r) => !seen.has(r.key) && !!seen.add(r.key));
  }
  /** The words for each kind of missing item. */
  const MISSING = {
    episode: { count: "missingEpisodes", file: "defaultMissingName" },
    issue: { count: "missingIssues", file: "defaultMissingIssuesName" },
    track: { count: "missingTracks", file: "defaultMissingTracksName" },
    movie: { count: "missingMovies", file: "defaultMissingMoviesName" },
    book: { count: "missingBooks", file: "defaultMissingBooksName" },
  } as const;
  const missingCount = $derived.by(() => {
    const kinds = [...new Set(ghosts.map((g) => g.kind))];
    return t(kinds.length === 1 ? MISSING[kinds[0]].count : "missingItems", { n: ghosts.length });
  });
  /** Episodes whose names are already right but that still get metadata. */
  const episodeMetaOnly = $derived(
    seriesRule?.rule.type === "series" && seriesRule.rule.sidecar && seriesRule.rule.source !== "anilist"
      ? preview.items.filter((i) => i.status === "unchanged" && episodeMatches[i.path] && isVideo(i.original))
      : [],
  );

  /** What the metadata of an episode needs; `key` is the path before renaming. */
  function tvSidecarItem(key: string, path: string) {
    const m = episodeMatches[key];
    return { path, id: m.id, season: m.season, episodes: m.episodes, titles: m.titles };
  }

  /** The ✓ ? ✕ marks next to the new names. */
  const matches = $derived.by(() => {
    const out: Record<string, { status: MovieStatus; tip: string }> = {};
    const tips = {
      movie: { auto: "movieAuto", chosen: "movieChosen", choose: "movieChoose", none: "movieNone" },
      episode: { auto: "episodeAuto", chosen: "episodeChosen", choose: "episodeChoose", none: "episodeNone" },
    } as const;
    if (movieRule) for (const [p, m] of Object.entries(movies)) out[p] = { status: m.status, tip: t(tips.movie[m.status]) };
    if (seriesRule) for (const [p, e] of Object.entries(series)) out[p] = { status: e.status, tip: t(tips.episode[e.status]) };
    if (bookRule) {
      const tip = { auto: "bookAuto", chosen: "bookChosen", choose: "bookChoose", none: "bookNone" } as const;
      for (const [p, b] of Object.entries(books)) out[p] = { status: b.status, tip: t(tip[b.status]) };
    }
    if (comicRule) {
      const tip = { auto: "comicAuto", chosen: "comicChosen", choose: "comicChoose", none: "comicNoneTip" } as const;
      for (const [p, c] of Object.entries(comics)) out[p] = { status: c.status, tip: t(tip[c.status]) };
    }
    if (musicRule) {
      const tip = { auto: "albumAuto", chosen: "albumChosen", choose: "albumChoose", none: "albumNone" } as const;
      for (const a of albums) {
        const key = a.status === "auto" && a.choice?.id.startsWith("discogs:") ? "albumAutoDiscogs" : tip[a.status];
        for (const p of a.files) out[p] = { status: a.status, tip: t(key) };
      }
      for (const i of preview.items) if (i.untagged) out[i.path] = { status: "none", tip: t("musicUntaggedTip") };
    }
    if (photoRule) {
      for (const i of preview.items) {
        if (i.photoDate === "none") out[i.path] = { status: "none", tip: t("photoUndatedTip") };
        else if (i.photoDate === "name") out[i.path] = { status: "auto", tip: t("photoFromNameTip") };
        else if (i.photoDate === "modified") out[i.path] = { status: "choose", tip: t("photoFromModifiedTip") };
      }
    }
    return out;
  });

  /** Found, waiting and without a match, among the visible files. */
  function counts(state: Record<string, { status: MovieStatus }>) {
    let found = 0, choose = 0, none = 0;
    for (const i of preview.items) {
      const st = state[i.path]?.status;
      if (st === "auto" || st === "chosen") found++;
      else if (st === "choose") choose++;
      else if (st === "none") none++;
    }
    return { found, choose, none };
  }
  let addMenu = $state(false);
  /** Another folder the files go to (moved or copied); for this session only. */
  let sendTo = $state<{ dir: string; copy: boolean } | null>(null);
  let sendMenu = $state(false);
  /** Bytes copied so far, while copying. */
  let applyProgress = $state<{ done: number; total: number } | null>(null);
  let ruleMenu = $state(false);
  let table = $state<FileTable>();

  /** Files the lookup rules on leave alone (a `.zip` in a show's folder),
   *  with why: tagged in the list, which can still name them by hand. */
  const outside = $derived.by(() => {
    const kinds: [boolean, (i: PreviewItem) => boolean, Key][] = [
      [!!movieRule, (i) => i.isDir || isScreen(i.original), "outsideMovie"],
      [!!seriesRule, (i) => !i.isDir && isScreen(i.original), "outsideSeries"],
      // Songs bring their lyrics, cue sheets, playlists and covers along.
      [!!musicRule, (i) => !i.isDir && (isAudio(i.original) || /\.(lrc|cue|m3u8?|pls|jpe?g|png|webp)$/i.test(i.original)), "outsideMusic"],
      [!!bookRule, (i) => !i.isDir && isBook(i.original), "outsideBook"],
      [!!comicRule, (i) => (i.isDir ? i.convert : isComic(i.original)), "outsideComic"],
    ];
    const on = kinds.filter(([enabled]) => enabled);
    if (!on.length) return {};
    const out: Record<string, string> = {};
    for (const i of preview.items) {
      // Renamed by another rule: nothing to tell.
      if ((i.status === "ok" && !i.manual) || on.some(([, takes]) => takes(i))) continue;
      out[i.path] = on.length === 1 ? t(on[0][2]) : t("outsideAny");
    }
    return out;
  });

  const view = $derived<View>({ filter, sortByNewName: sortKey === "newName", descending: sortDesc, identicalOnly, alikeOnly, similarOnly, movies: movieMatches, episodes: episodeMatches, songs: songMatches, books: bookMatches, comics: comicMatches, leftoverSuffix: t("leftoverSuffix"), sendTo });
  /** Example name for rule cards: the first file in natural name order, so
   *  it does not jump around while the preview re-sorts. */
  const collator = new Intl.Collator(undefined, { numeric: true });
  /** "+ Rule" menu in alphabetical order of the current language. */
  /** The lookup rule in the stack (on or off): the others wait until it
   *  is removed. */
  const lookupType = $derived(steps.find((s) => isLookup(s.rule.type))?.rule.type ?? null);
  /** The optional columns that make sense with the rules in the stack. */
  const availableColumns = $derived.by<DetailColumn[]>(() => {
    const cols: DetailColumn[] = [];
    if (lookupType === "music") cols.push("artist", "album", "title", "track", "year", "genre", "length", "quality");
    if (lookupType === "movie" || lookupType === "series") cols.push("resolution", "codec");
    if (steps.some((s) => s.rule.type === "insert" && s.rule.source === "taken")) cols.push("taken");
    return cols;
  });
  // Sorted by a column no longer shown: the order stays, as a hand-made one.
  $effect(() => {
    if ((DETAIL_COLUMNS as readonly string[]).includes(sortKey) && !availableColumns.includes(sortKey as DetailColumn)) {
      sortKey = "manual";
    }
  });
  const ruleTypes = $derived(
    [...RULE_TYPES].sort((a, b) => t(`rule.${a}`).localeCompare(t(`rule.${b}`), LOCALES[settings.lang])),
  );
  const sample = $derived(
    preview.items.reduce<string>((min, i) => (!min || collator.compare(i.original, min) < 0 ? i.original : min), ""),
  );
  const canMove = $derived(selected.size > 0 && sortKey !== "newName");
  /** The name the lookups read: what the rules above the lookup rule make of it. */
  const readName = (i: PreviewItem) => i.lookupName ?? i.original;

  function ruleError(i: number) {
    const e = preview.ruleErrors.find((e) => e.index === i);
    return e ? `${t(e.code)}: ${e.detail}` : null;
  }

  // Empty numeric fields become 0 so Rust accepts them.
  function payload(): Step[] {
    return $state.snapshot(steps).map((s) => {
      const rule: Record<string, unknown> = { ...s.rule };
      for (const [k, v] of Object.entries(rule)) {
        if (v === null || (typeof v === "number" && !Number.isFinite(v))) rule[k] = 0;
        else if (typeof v === "number" && k === "shiftHours") rule[k] = Math.max(-48, Math.min(48, Math.trunc(v)));
        else if (typeof v === "number" && k !== "start" && k !== "step") rule[k] = Math.max(0, Math.trunc(v));
      }
      return { ...s, rule } as Step;
    });
  }

  /** The latest preview asked for: an older one that comes back later is
   *  dropped (they run in the background). */
  let previewRun = 0;
  async function refresh() {
    const run = ++previewRun;
    try {
      const p = await api.preview(payload(), $state.snapshot(overrides), $state.snapshot(view));
      if (run === previewRun) preview = p;
      photosProgress = null;
    } catch (e) {
      photosProgress = null;
      notice = `${t("error")}: ${tError(e)}`;
    }
  }

  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    JSON.stringify(steps);
    JSON.stringify(overrides);
    JSON.stringify(view);
    if (!ready) return;
    clearTimeout(timer);
    timer = setTimeout(refresh, 120);
  });

  // Save the settings (and the rules, if asked) when they change.
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const deps = [settings.lang, settings.theme, settings.includeHidden, settings.confirmApply,
      settings.disableRulesAfterApply, settings.warnWindows, settings.rememberRules, settings.tmdbLang, settings.showMissing,
      JSON.stringify(steps)];
    if (!ready || !deps) return;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      settings.rules = settings.rememberRules ? payload() : [];
      saveSettings();
    }, 400);
  });

  // The keys and the GCD account go to the system's password store, apart.
  let secretsTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const deps = [settings.tmdbKey, settings.comicVineKey, settings.discogsToken, settings.metronToken, settings.googleBooksKey, settings.gcdEmail, settings.gcdPassword];
    if (!ready || !deps) return;
    clearTimeout(secretsTimer);
    secretsTimer = setTimeout(() => saveSecrets().catch(() => {}), 600);
  });

  $effect(() => {
    document.documentElement.lang = LOCALES[settings.lang];
  });

  // Discogs is asked when MusicBrainz finds nothing, with the user's token.
  $effect(() => {
    api.discogsToken(settings.discogsToken).catch(() => {});
  });
  // Google Books is asked for books Open Library does not settle.
  $effect(() => {
    api.googleBooksKey(settings.googleBooksKey).catch(() => {});
  });
  $effect(() => {
    api.gcdAccount(settings.gcdEmail, settings.gcdPassword).catch(() => {});
  });

  /** Re-sorts the list in Rust by the chosen column. */
  async function applySort() {
    if (sortKey === "manual") return;
    if (sortKey === "newName") await api.sortFiles("name", false);
    else await api.sortFiles(sortKey, sortDesc);
  }

  async function sortBy(key: Exclude<ViewSort, "manual">) {
    if (busy) return;
    if (key === sortKey) sortDesc = !sortDesc;
    else {
      sortKey = key;
      sortDesc = false;
    }
    // By what the files hold: each file is read (once), which takes a while.
    const reads = (DETAIL_COLUMNS as readonly string[]).includes(key);
    if (reads) {
      busy = true;
      task = "sort";
    }
    try {
      await applySort();
    } finally {
      if (reads) {
        busy = false;
        task = null;
      }
    }
    await refresh();
  }

  async function addPaths(paths: string[], foldersAsItems = false) {
    if (!paths.length) return;
    busy = true;
    task = "add";
    try {
      await remember();
      const r = await api.addPaths(paths, { recursive: true, includeHidden: settings.includeHidden, foldersAsItems });
      if (!r.added.length) await dropCheckpoint();
      await applySort();
      await refresh();
      checkIdentical();
      // Highlight what was just added.
      selected = new Set(r.added);
      await tick();
      table?.reveal(r.added);
      notice = t("added", { n: r.added.length }) + (r.skipped.length ? ` · ${t("skipped", { n: r.skipped.length })}` : "");
    } catch (e) {
      notice = `${t("error")}: ${tError(e)}`;
    } finally {
      busy = false;
      task = null;
    }
  }

  async function pickFiles() {
    addMenu = false;
    const r = await open({ multiple: true, title: t("addFiles") });
    if (r) await addPaths(Array.isArray(r) ? r : [r]);
  }

  async function pickFolders(asItems = false) {
    addMenu = false;
    const title = t(asItems ? "addFolderItems" : "addFolders");
    const r = await open({ directory: true, multiple: true, title });
    if (r) await addPaths(Array.isArray(r) ? r : [r], asItems);
  }

  async function pickSendTo(copy: boolean) {
    sendMenu = false;
    const r = await open({ directory: true, multiple: false, title: t(copy ? "sendCopyTo" : "sendMoveTo") });
    if (typeof r === "string") sendTo = { dir: r, copy };
  }

  async function removeSelected() {
    if (!selected.size) return;
    await remember(found());
    await api.removeFiles([...selected]);
    forget(selected);
    selected = new Set();
    await refresh();
    checkIdentical();
  }

  async function clearAll() {
    await remember(found());
    await api.clearFiles();
    forget(null);
    selected = new Set();
    overrides = {};
    await refresh();
    checkIdentical();
  }

  async function move(delta: -1 | 1) {
    if (!canMove) return;
    await remember();
    await api.moveFiles([...selected], delta);
    sortKey = "manual";
    await refresh();
  }

  function addRule(type: RuleType) {
    if (lookupType && isLookup(type)) return;
    steps.push(newStep(newRule(type)));
    ruleMenu = false;
  }

  /** Loaded steps with one lookup rule at most; the notice says which
   *  were left out, else `done` (when given). */
  function loadSteps(loaded: Step[], done = ""): Step[] {
    const { steps: kept, dropped } = oneLookup(loaded);
    if (dropped.length) notice = t("lookupsDropped", { rules: dropped.map((r) => t(`rule.${r}`)).join(", ") });
    else if (done) notice = done;
    return kept.map((s) => newStep(s.rule, s.enabled));
  }

  function moveStep(i: number, delta: -1 | 1) {
    const j = i + delta;
    [steps[i], steps[j]] = [steps[j], steps[i]];
  }

  async function setOverride(path: string, name: string | null) {
    await remember();
    if (name === null) delete overrides[path];
    else overrides[path] = name;
  }

  const basename = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const errText = (e: OpError) => {
    if (e.kind === "history") return `${t("err.history")} (${e.detail})`;
    const why =
      e.kind === "io"
        ? e.detail || t("err.io")
        : e.kind === "tags"
          ? `${t("err.tags")}: ${tError(e.detail)}`
          : // The temporary name it was left under, to find it.
            e.kind === "restoreFailed" && e.detail
            ? `${t("err.restoreFailed")} ${basename(e.detail)}`
            : t(`err.${e.kind}`);
    return `${basename(e.from)} — ${why}`;
  };

  /** Shows the files that failed, if any. */
  function showErrors(r: ApplyReport, title: string, text: string) {
    if (!r.errors.length) return;
    return notify({ title, text, items: r.errors.map(errText) });
  }

  async function applyAll() {
    // A shortcut may come before the debounced preview update.
    clearTimeout(timer);
    await refresh();
    const meta = metaOnly.map((i) => ({ path: i.path, id: movies[i.path].choice!.id }));
    const tvMeta = episodeMetaOnly.map((i) => tvSidecarItem(i.path, i.path));
    const listOnly = playlistOnly;
    const retagOnly = tagsOnly.map((i) => i.path);
    const subsUnchanged = subsOnly.map((i) => i.path);
    const subsPlanned = subsWanted === null ? 0 : preview.items.reduce((n, i) => n + (i.status !== "conflict" ? subsChosen(subsFound[i.path]) : 0), 0);
    const bookMeta = bookMetaOnly.map((i) => bookSidecarItem(i.path, i.path));
    /** Comics renamed `.cbz` to be converted. */
    const converting = new Set(preview.items.filter((i) => i.convert).map((i) => i.path));
    const cleared = new Set(preview.items.filter((i) => i.junk).map((i) => i.path));
    const insideOnly = [
      ...comicInfoOnly.map((i) => comicInsideItem(i.path, i.path, i.convert)),
      ...epubOnly.map((i) => bookInsideItem(i.path, i.path)),
    ];
    if (!preview.changed && !meta.length && !tvMeta.length && !bookMeta.length && !coverOnly && !listOnly.length && !retagOnly.length && !insideOnly.length && !subsUnchanged.length) return;
    // Files left behind in a show's old folder are always told first.
    const left = preview.leftFolders.map((f) => t("confirmLeftover", { n: f.files, from: basename(f.from), to: basename(f.to) }));
    if (settings.confirmApply || left.length) {
      const lines = [
        ...left,
        preview.changed
          ? sendTo
            ? t(sendTo.copy ? "confirmCopy" : "confirmMove", { n: preview.changed, dir: sendTo.dir })
            : t("confirmText", { n: preview.changed })
          : "",
        meta.length + tvMeta.length ? t("confirmMeta", { n: meta.length + tvMeta.length }) : "",
        bookMeta.length ? t("confirmBookMeta", { n: bookMeta.length }) : "",
        coverOnly ? t("confirmCovers", { n: coverOnly }) : "",
        listOnly.length ? t("confirmPlaylists", { n: listOnly.length }) : "",
        retagOnly.length ? t("confirmTags", { n: retagOnly.length }) : "",
        insideOnly.length ? t("confirmInside", { n: insideOnly.length }) : "",
        subsPlanned ? t("confirmSubs", { n: subsPlanned }) : "",
        preview.conflicts ? t("confirmConflicts", { n: preview.conflicts }) : "",
      ].filter(Boolean);
      const ok = await confirm({
        title: t(sendTo ? (sendTo.copy ? "confirmCopyTitle" : "confirmMoveTitle") : "confirmTitle"),
        text: `${lines.join("\n")}\n\n${t("confirmUndoHint")}`,
        ok: t(sendTo ? (sendTo.copy ? "confirmCopyOk" : "confirmMoveOk") : "confirmOk"),
        cancel: t("cancel"),
      });
      if (!ok) return;
    }
    busy = true;
    task = "rename";
    applyProgress = null;
    // Where the files went, for the notice (the choice is reset below).
    const sent = sendTo;
    try {
      const rule = movieRule?.rule;
      // Old folders renamed for their leftovers: not files.
      const leftFrom = new Set(preview.leftFolders.map((f) => f.from));
      const r: ApplyReport = preview.changed
        ? await api.apply(payload(), $state.snapshot(overrides), $state.snapshot(view))
        : { done: [], errors: [], historyId: null, lists: 0, listsRemoved: 0 };
      let saved = 0;
      // Albums whose playlist was there already (and follows the renames).
      let keptLists = 0;
      // Renamed without a history entry (it could not be written): nothing
      // more is written, or undo could not take it back.
      // The list's undo starts over (as the core's, when it renamed).
      if (preview.changed) foundUndo = [];
      const recorded = !r.done.length || !!r.historyId;
      if (recorded && rule?.type === "movie" && rule.sidecar) {
        const items = r.done
          .filter((op) => movies[op.from]?.choice && isVideo(op.to))
          .map((op) => ({ path: op.to, id: movies[op.from].choice!.id }))
          .concat(meta);
        if (items.length) {
          tmdbProgress = { done: 0, total: items.length, kind: "save" };
          saved += await writeMeta(() => api.writeSidecars(settings.tmdbKey, tmdbLang(), r.historyId, items, rule.fanart, rule.folder));
        }
      }
      const mrule = musicRule?.rule;
      if (recorded && mrule?.type === "music" && mrule.cover) {
        const items = coverItems(r);
        if (items.length) {
          mbProgress = { done: 0, total: items.length, kind: "save" };
          try {
            saved += await writeMeta(() => api.writeCovers(r.historyId, items));
          } finally {
            mbProgress = null;
          }
        }
      }
      if (recorded && mrule?.type === "music" && mrule.playlist) {
        const parent = (p: string) => p.slice(0, Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\")));
        const dirs = [...new Set(r.done.filter((op) => isAudio(op.to)).map((op) => parent(op.to)).concat(listOnly))];
        if (dirs.length)
          saved += await writeMeta(async () => {
            const res = await api.writePlaylists(r.historyId, dirs);
            keptLists = res.kept;
            return res;
          });
      }
      let tagged = 0;
      if (recorded && mrule?.type === "music" && mrule.writeTags) {
        // Every matched song that moved, and those that keep their names.
        const items = r.done
          .filter((op) => songMatches[op.from] && isAudio(op.to))
          .map((op) => ({ path: op.to, song: songMatches[op.from] }))
          .concat(retagOnly.map((p) => ({ path: p, song: songMatches[p] })));
        if (items.length) {
          mbProgress = { done: 0, total: items.length, kind: "tags" };
          try {
            const res = await api.writeTags(r.historyId, $state.snapshot(items), mrule.embedCover, mrule.cleanTags, mrule.coverSize);
            tagged = res.written;
            if (res.errors.length) {
              await notify({
                title: t("tagsTitle"),
                text: t("tagsN", { n: res.written }),
                items: res.errors.map(([file, why]) => (file ? `${file} — ${tError(why)}` : tError(why))),
              });
            }
          } catch (e) {
            await notify({ title: t("tagsTitle"), text: tError(e) });
          } finally {
            mbProgress = null;
          }
        }
      }
      const brule = bookRule?.rule;
      if (recorded && brule?.type === "book" && (brule.cover || brule.opf)) {
        const items = r.done
          .filter((op) => books[op.from]?.choice && isBook(op.to))
          .map((op) => bookSidecarItem(op.from, op.to))
          .concat(bookMeta);
        if (items.length) {
          bookProgress = { done: 0, total: items.length, kind: "save" };
          try {
            saved += await writeMeta(() => api.writeBookSidecars(r.historyId, $state.snapshot(items), brule.cover, brule.opf, brule.folders));
          } finally {
            bookProgress = null;
          }
        }
      }
      let inside = 0;
      let drawnPages = 0;
      {
        const crule = comicRule?.rule;
        const items = [
          ...(crule?.type === "comic" && (crule.writeInfo || crule.convert || crule.cleanInfo)
            ? r.done
                .filter(
                  (op) =>
                    extOf(op.to) === "cbz" &&
                    ((crule.convert && converting.has(op.from)) ||
                      (crule.writeInfo && comicMatches[op.from]) ||
                      clearsInfo(crule, cleared.has(op.from))),
                )
                .map((op) => comicInsideItem(op.from, op.to, converting.has(op.from)))
            : []),
          ...(brule?.type === "book" && brule.writeEpub
            ? r.done.filter((op) => books[op.from]?.choice && isEpub(op.to)).map((op) => bookInsideItem(op.from, op.to))
            : []),
          ...insideOnly,
        ];
        if (recorded && items.length) {
          insideProgress = { done: 0, total: items.length };
          try {
            const res = await api.writeInside(r.historyId, $state.snapshot(items));
            inside = res.written;
            drawnPages = res.drawn;
            if (res.errors.length) {
              await notify({
                title: t("insideTitle"),
                text: t("insideN", { n: res.written }),
                items: res.errors.map(([file, why]) => (file ? `${file} — ${tError(why)}` : tError(why))),
              });
            }
          } catch (e) {
            await notify({ title: t("insideTitle"), text: tError(e) });
          } finally {
            insideProgress = null;
          }
        }
      }
      const srule = seriesRule?.rule;
      if (recorded && srule?.type === "series" && srule.sidecar && srule.source !== "anilist") {
        const items = r.done
          .filter((op) => episodeMatches[op.from] && isVideo(op.to))
          .map((op) => tvSidecarItem(op.from, op.to))
          .concat(tvMeta);
        if (items.length) {
          tmdbProgress = { done: 0, total: items.length, kind: "saveTv" };
          saved += await writeMeta(() =>
            api.writeTvSidecars(settings.tmdbKey, tmdbLang(), r.historyId, items, srule.fanart, srule.folders),
          );
        }
      }
      let subsSaved = 0;
      if (recorded && subsWanted !== null) {
        const pairs: [string, string][] = [
          ...r.done.filter((op) => isVideo(op.to) && subsFound[op.from]).map((op): [string, string] => [op.from, op.to]),
          ...subsUnchanged.map((p): [string, string] => [p, p]),
        ];
        const items = subsWrites(pairs);
        if (items.length) {
          subsProgress = { done: 0, total: items.reduce((n, i) => n + i.langs.length, 0), kind: "save" };
          try {
            const res = await api.writeSubtitles(subsAuth(), r.historyId, $state.snapshot(items));
            subsSaved = res.created.length;
            if (res.remaining !== null) subsRemaining = res.remaining;
            // Downloaded (or tried): searched again if wanted.
            for (const [from] of pairs) delete subsFound[from];
            if (res.errors.length) {
              await notify({
                title: t("subsTitle"),
                text: t("subsSavedN", { n: subsSaved }),
                items: res.errors.map(([file, why]) => (file ? `${file} — ${tError(why)}` : tError(why))),
              });
            }
          } catch (e) {
            await notify({ title: t("subsTitle"), text: tError(e) });
          } finally {
            subsProgress = null;
          }
        }
      }
      followMatches(r);
      if (settings.disableRulesAfterApply) for (const s of steps) s.enabled = false;
      // The list now holds what was sent: sending it again would send it
      // once more, so the files stay where they are until chosen again.
      if (preview.changed) sendTo = null;
      overrides = {};
      notice = [
        preview.changed ? t(sent ? (sent.copy ? "copiedN" : "movedN") : "renamedN", { n: r.done.filter((op) => !leftFrom.has(op.from)).length }) : "",
        r.stopped ? t("applyStopped") : "",
        r.done.some((op) => leftFrom.has(op.from)) ? t("leftFoldersN", { n: r.done.filter((op) => leftFrom.has(op.from)).length }) : "",
        r.lists ? t("listsN", { n: r.lists }) : "",
        r.listsRemoved ? t("listsRemovedN", { n: r.listsRemoved }) : "",
        saved ? t("sidecarsN", { n: saved }) : "",
        subsSaved ? t("subsSavedN", { n: subsSaved }) : "",
        keptLists ? t("playlistsKeptN", { n: keptLists }) : "",
        tagged ? t("tagsN", { n: tagged }) : "",
        inside ? t("insideN", { n: inside }) : "",
        drawnPages ? t("insideDrawnN", { n: drawnPages }) : "",
        r.errors.length ? t("errorsN", { n: r.errors.length }) : "",
      ].filter(Boolean).join(" · ");
      await refresh();
      checkIdentical();
      if (!r.errors.length && notice) done(notice);
      await showErrors(r, t("resultTitle"), `${t("renamedN", { n: r.done.length })}\n${t("errorsN", { n: r.errors.length })}:`);
    } catch (e) {
      notice = `${t("error")}: ${tError(e)}`;
    } finally {
      // Its own notice says it stopped.
      stopping = false;
      applyProgress = null;
      busy = false;
      task = null;
    }
  }

  async function openHistory() {
    history = await api.history();
  }

  async function undo(id: string) {
    busy = true;
    task = "undo";
    try {
      const r = await api.undo(id);
      foundUndo = [];
      followMatches(r);
      const restored = r.done.filter((op) => !op.copy).length;
      notice = [restored || !r.copiesRemoved ? t("restoredN", { n: restored }) : "", r.copiesRemoved ? t("copiesRemovedN", { n: r.copiesRemoved }) : ""]
        .filter(Boolean)
        .join(" · ");
      if (history) history = await api.history();
      if (r.kept?.length) await notify({ title: t("copiesKeptTitle"), text: t("copiesKeptText"), items: r.kept });
      await refresh();
      checkIdentical();
      if (!r.errors.length) done(notice);
      await showErrors(
        r,
        r.done.length ? t("resultTitle") : t("undoFailed"),
        `${restored ? `${t("restoredN", { n: restored })}\n` : ""}${t("undoFailedHint")}`,
      );
    } catch (e) {
      await notify({ title: t("undoFailed"), text: tError(e) });
    } finally {
      busy = false;
      task = null;
    }
  }

  /** Looks for identical files in the background after the list changes.
   *  A newer search replaces an older one, which then returns null. */
  async function checkIdentical() {
    try {
      const found = await api.findIdentical();
      if (!found) return;
      const byGroup = (groups: string[][]) => {
        const map: Record<string, Copies> = {};
        groups.forEach((g, group) => {
          for (const p of g) map[p] = { group, others: g.filter((x) => x !== p) };
        });
        return map;
      };
      copies = byGroup(found.identical);
      copyGroups = found.identical;
      alike = byGroup(found.songs);
      if (!found.identical.length) identicalOnly = false;
      if (!found.songs.length) alikeOnly = false;
      identicalProgress = null;
      await refresh();
      checkSimilar(byGroup);
    } catch (e) {
      identicalProgress = null;
      notice = `${t("error")}: ${tError(e)}`;
    }
  }

  /** Opens the comparison window at the group of `path`. */
  function openCompare(kind: "similar" | "identical", path: string) {
    const groups = kind === "similar" ? similarGroups : copyGroups;
    const start = groups.findIndex((g) => g.includes(path));
    if (start >= 0) compare = { kind, start };
  }

  async function closeCompare(trashed: string[]) {
    compare = null;
    if (!trashed.length) return;
    const gone = new Set(trashed);
    selected = new Set([...selected].filter((p) => !gone.has(p)));
    notice = t("compareTrashedN", { n: trashed.length });
    await refresh();
    checkIdentical();
  }

  /** Then looks for pictures that look alike; a newer search for copies
   *  stops it (it then returns null). */
  async function checkSimilar(byGroup: (groups: string[][]) => Record<string, Copies>) {
    try {
      const found = await api.findSimilar();
      if (!found) return;
      const before = JSON.stringify(Object.keys(similar));
      similar = byGroup(found);
      similarGroups = found;
      if (!found.length) similarOnly = false;
      similarProgress = null;
      if (JSON.stringify(Object.keys(similar)) !== before) await refresh();
    } catch (e) {
      similarProgress = null;
      notice = `${t("error")}: ${tError(e)}`;
    }
  }

  /** Looks up the visible files without a movie yet on TMDB; files picked
   *  with confidence are done, the rest go to the review window. */
  async function searchMovies() {
    if (!settings.tmdbKey.trim()) {
      const ok = await confirm({ title: t("rule.movie"), text: t("tmdbNeedsKey"), ok: t("options"), cancel: t("cancel") });
      if (ok) showOptions = true;
      return;
    }
    const items = preview.items
      .filter((i) => (i.isDir || isScreen(i.original)) && movies[i.path]?.status !== "auto" && movies[i.path]?.status !== "chosen")
      .map((i) => ({ path: i.path, name: i.isDir ? readName(i) : splitExt(readName(i))[0] }));
    if (!items.length) {
      notice = t("tmdbNoFiles");
      return;
    }
    tmdbProgress = { done: 0, total: items.length, kind: "search" };
    try {
      const found = await api.tmdbLookup(settings.tmdbKey, tmdbLang(), items);
      // Nothing was really searched (bad key, no connection): mark nothing.
      const failed = found.find((f) => f.error);
      if (failed && found.every((f) => f.error || !f.guess.title)) {
        await notify({ title: t("rule.movie"), text: tError(failed.error) });
        return;
      }
      for (const f of found) {
        movies[f.path] = {
          guess: f.guess,
          candidates: f.candidates,
          choice: f.auto ? f.candidates[0] : null,
          status: f.auto ? "auto" : f.candidates.length ? "choose" : "none",
          error: f.error,
        };
      }
      if (found.some((f) => !f.auto && f.candidates.length)) openReview();
    } catch (e) {
      await notify({ title: t("rule.movie"), text: tError(e) });
    } finally {
      tmdbProgress = null;
    }
  }

  /** Looks up the visible books without a choice yet on Open Library;
   *  books picked with confidence are done, the rest go to the review. */
  async function searchBooks() {
    const items = preview.items
      .filter((i) => !i.isDir && isBook(i.original) && books[i.path]?.status !== "auto" && books[i.path]?.status !== "chosen")
      .map((i) => ({ path: i.path, name: splitExt(readName(i))[0] }));
    if (!items.length) {
      notice = t("bookNoFiles");
      return;
    }
    bookProgress = { done: 0, total: items.length, kind: "search" };
    try {
      const found = await api.bookLookup(tmdbLang(), items);
      // Nothing was really searched (no connection): mark nothing.
      const failed = found.find((f) => f.error);
      if (failed && found.every((f) => f.error || (!f.guess.title && !f.guess.isbns.length))) {
        await notify({ title: t("rule.book"), text: tError(failed.error) });
        return;
      }
      for (const f of found) {
        books[f.path] = {
          guess: f.guess,
          candidates: f.candidates,
          choice: f.auto ? f.candidates[0] : null,
          status: f.auto ? "auto" : f.candidates.length ? "choose" : "none",
          error: f.error,
        };
      }
      if (found.some((f) => !f.auto && f.candidates.length)) openBookReview();
    } catch (e) {
      await notify({ title: t("rule.book"), text: tError(e) });
    } finally {
      bookProgress = null;
    }
  }

  /** Opens the review on the books still without a choice (or on one). */
  function openBookReview(start?: string) {
    const paths = preview.items.map((i) => i.path);
    const pending = [
      ...paths.filter((p) => books[p]?.status === "choose"),
      ...paths.filter((p) => books[p]?.status === "none"),
    ];
    const queue = start ? [start, ...pending.filter((p) => p !== start)] : pending;
    bookReview = queue.length ? queue : null;
  }

  /** The user's pick for a book; undecided files with the same guess (the
   *  same book in other formats) get it too. */
  function chooseBook(path: string, choice: BookCandidate | null, candidates?: BookCandidate[]) {
    const b = books[path];
    if (!b) return;
    const set = (p: string) => {
      books[p] = { ...books[p], choice, status: choice ? "chosen" : "none", candidates: candidates ?? books[p].candidates };
    };
    set(path);
    for (const [p, o] of Object.entries(books)) {
      const same = o.guess.title === b.guess.title && o.guess.author === b.guess.author;
      if (p !== path && same && (o.status === "choose" || o.status === "none")) set(p);
    }
  }

  /** Movie, episode and book choices follow their files when renamed or restored. */
  function followMatches(r: ApplyReport) {
    if (!r.done.length) return;
    // All the renames at once (two files may swap names), then the folders
    // renamed along, which carry what is inside them.
    const exact = new Map(r.done.map((op) => [op.from, op.to]));
    const inside = (p: string, dir: string) => p.startsWith(`${dir}/`) || p.startsWith(`${dir}\\`);
    const follow = (p: string) => {
      let q = exact.get(p) ?? p;
      for (const op of r.done) if (inside(q, op.from)) q = op.to + q.slice(op.from.length);
      return q;
    };
    function moved<T>(m: Record<string, T>): Record<string, T> {
      return Object.fromEntries(Object.entries(m).map(([p, v]) => [follow(p), v]));
    }
    books = moved(books);
    comics = moved(comics);
    movies = moved(movies);
    series = moved(series);
    subsFound = moved(subsFound);
    for (const a of albums) {
      a.files = a.files.map(follow);
      a.matches = moved(a.matches);
    }
  }

  /** The chosen catalogue (GCD, Metron, Comic Vine, a manga one) for the
   *  visible comics: on Metron, first each file's barcode (recorded in it,
   *  or read in its pages when asked); then one series per group of files
   *  (the series typed, else each series the files say), picked by itself
   *  when only one has that name (and year), else asked; then each file's
   *  issue by its number. */
  async function searchComics(rule: Extract<Rule, { type: "comic" }>) {
    const source = rule.source ?? "gcd";
    const skip = rule.skipPublishers?.trim() ?? "";
    const key = comicKey(source);
    if ((source === "comicvine" || source === "metron") && !key) {
      const ok = await confirm({ title: t("rule.comic"), text: t(source === "metron" ? "mtNeedsKey" : "cvNeedsKey"), ok: t("options"), cancel: t("cancel") });
      if (ok) showOptions = true;
      return;
    }
    // Without an account the GCD takes few searches: said once per session.
    if (source === "gcd" && !gcdWarned && !(settings.gcdEmail.trim() && settings.gcdPassword)) {
      gcdWarned = true;
      const ok = await confirm({ title: t("rule.comic"), text: t("gcdNoAccount"), ok: t("options"), cancel: t("gcdAnonymous") });
      if (ok) {
        showOptions = true;
        return;
      }
    }
    // A folder of pictures too, when it becomes a CBZ.
    const files = preview.items.filter((i) => (i.isDir ? i.convert : isComic(i.original)));
    if (!files.length) {
      notice = t("comicNoFiles");
      return;
    }
    comicProgress = t("comicReading");
    const lang = settings.lang === "pt" ? "pt-br" : settings.lang;
    try {
      const owns = await api.comicGuesses(
        files.map((f) => f.path),
        files.map((f) => (f.lookupName ? (f.isDir ? f.lookupName : splitExt(f.lookupName)[0]) : null)),
        !!rule.byFolder,
      );
      const run = stopRun;
      // On Metron, a barcode tells the issue itself.
      const coded = new Set<string>();
      if (source === "metron") {
        let codes = owns.map((o) => o?.extra?.gtin?.replace(/\D/g, "") || null);
        if (rule.barcode && codes.some((c) => !c)) {
          comicProgress = t("comicBarcodeReading");
          const unread = files.flatMap((f, i) => (!codes[i] && owns[i] ? [i] : []));
          const read = await api.comicBarcodes(unread.map((i) => files[i].path));
          if (stopRun !== run) return;
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
          if (stopRun !== run) return;
          comicProgress = t("comicBarcodeLooking", { n: `${n + 1}/${withCode.length}` });
          const found = await api.comicByCode(key, codes[i]!);
          if (!found) continue;
          const vkey = `${source}:${found.volume.id}`;
          comicVolumes[vkey] = { volume: found.volume, issues: found.issues };
          // The code tells the issue: its number too, when the name has none.
          const own = owns[i]!;
          const number = found.issues.find((x) => x.id === found.issue)?.number ?? "";
          const exact = own.number || !number ? own : { ...own, number };
          comics[files[i].path] = { volumeKey: vkey, own: exact, choice: found.issue, status: "auto" };
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
        if (!coded.size) notice = t("comicNoFiles");
        return;
      }
      for (const [k, g] of groups) {
        if (stopRun !== run) break;
        // Loose chapters are no issues in the catalogues of issues.
        if (!seriesOnly(source) && g.owns.every((o) => !o.number && o.volume == null && !!o.chapter)) continue;
        const picked = `${source}:${k}`;
        let volume: ComicVolume | null = pickedComics[picked] ?? null;
        // A `ComicInfo.xml` link to a Comic Vine issue tells the series.
        const linked = source === "comicvine" ? g.owns.find((o) => o.id) : undefined;
        if (!volume && linked) {
          try {
            const id = await api.comicVolumeOf(key, linked.id);
            volume = comicVolumes[`${source}:${id}`]?.volume ?? { id, name: g.query, startYear: null, publisher: "", count: 0, cover: "", country: "", language: "", format: "", writer: "" };
          } catch {
            volume = null;
          }
        }
        if (!volume) {
          comicProgress = t("comicSearching", { series: g.query });
          let found = await api.comicSearch(source, key, g.query, lang, null, "", skip);
          // `Series: Story` may be a story in `Series`: the series alone.
          const head = g.query.split(/\s*[:–]\s*|\s+-\s+/)[0].trim();
          if (!found.length && head && head !== g.query.trim()) found = await api.comicSearch(source, key, head, lang, null, "", skip);
          if (stopRun !== run) break;
          const year = g.owns.find((o) => o.seriesYear)?.seriesYear ?? null;
          // A common name (`Saga`) may leave the series out of what the
          // catalogue answers: asked again with the files' year (theirs, or
          // their issues'), whose series come first.
          const dated = year ?? g.owns.find((o) => o.year)?.year ?? null;
          if (dated && !found.some((v) => fold(v.name) === k)) {
            const more = await api.comicSearch(source, key, g.query, lang, dated, "", skip).catch(() => []);
            if (stopRun !== run) break;
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
                  comicPick = { source, skip, query: g.query, files: g.paths.length, highest, candidates: sorted, path: g.paths[at], comic: g.owns[at], resolve };
                });
          comicPick = null;
          if (!volume) continue;
        }
        pickedComics[picked] = volume;
        // The manga catalogues have series, not a publisher's volumes: the
        // file's numbers stay. MangaUpdates tells a series' authors and
        // volumes only when asked for it.
        if (seriesOnly(source)) {
          const vkey = `${source}:${volume.id}`;
          if (source === "mangaupdates") {
            const full = await api.comicIssues(source, key, volume.id, g.owns).catch(() => null);
            if (stopRun !== run) break;
            if (full) volume = { ...volume, ...Object.fromEntries(Object.entries(full.volume).filter(([, x]) => x !== "" && x !== null && x !== 0)) };
          }
          comicVolumes[vkey] = { volume, issues: [] };
          for (const [i, p] of g.paths.entries()) comics[p] = { volumeKey: vkey, own: g.owns[i], choice: "", status: "auto" };
          continue;
        }
        if (stopRun !== run) break;
        comicProgress = t("comicLoading", { series: volume.name });
        const res = await api.comicIssues(source, key, volume.id, g.owns);
        const vkey = `${source}:${res.volume.id}`;
        // The search's own details where the series' lack them (a cover).
        comicVolumes[vkey] = { volume: { ...volume, ...Object.fromEntries(Object.entries(res.volume).filter(([, x]) => x !== "" && x !== null && x !== 0)) }, issues: res.issues };
        g.paths.forEach((p, i) => {
          const found = res.found[i];
          const own = g.owns[i];
          // A loose chapter is no issue there: the series is all there is.
          const chapterOnly = !own.number && !!own.chapter;
          // A file without its issue still takes the series ("": the series
          // alone, with its own numbers), so a series' files stay together;
          // the review can still pick its issue.
          comics[p] = {
            volumeKey: vkey,
            own,
            choice: found ?? "",
            status: found != null || chapterOnly ? "auto" : "choose",
          };
        });
      }
      if (Object.values(comics).some((c) => c.status === "choose")) openComicReview();
    } catch (e) {
      const err = String(e);
      // A refused account is fixed in the options.
      if (err.startsWith("gcdBadLogin") || err.startsWith("mtBadKey")) {
        if (await confirm({ title: t("rule.comic"), text: tError(e), ok: t("options"), cancel: t("close") })) showOptions = true;
      } else {
        await notify({ title: t("rule.comic"), text: tError(e) });
      }
    } finally {
      comicProgress = null;
      comicPick = null;
    }
  }

  /** Opens the review on the comics still without an issue (or on one). */
  function openComicReview(start?: string) {
    const paths = preview.items.map((i) => i.path);
    const pending = [
      ...paths.filter((p) => comics[p]?.status === "choose"),
      ...paths.filter((p) => comics[p]?.status === "none"),
    ];
    const queue = start ? [start, ...pending.filter((p) => p !== start)] : pending;
    comicReview = queue.length ? queue : null;
  }

  function chooseComic(path: string, issue: string | null) {
    const c = comics[path];
    if (c) comics[path] = { ...c, choice: issue, status: issue == null ? "none" : "chosen" };
  }

  /** Looks up the visible songs' albums on MusicBrainz; albums already
   *  settled keep their release, doubtful ones go to the review window. */
  async function searchAlbums(rule: Extract<Rule, { type: "music" }>) {
    const settled = new Set(
      albums.filter((a) => a.status === "auto" || a.status === "chosen").flatMap((a) => a.files),
    );
    const paths = preview.items
      // Songs without a title can still be identified by their sound.
      .filter((i) => !i.isDir && (!i.untagged || rule.listen) && isAudio(i.original) && !settled.has(i.path))
      .map((i) => i.path);
    if (!paths.length) {
      notice = t("mbNoFiles");
      return;
    }
    mbProgress = { done: 0, total: 0, kind: "search" };
    try {
      const found = await api.mbLookup(paths, rule.fromName, rule.listen);
      if (!found.length) {
        notice = t("mbNoFiles");
        return;
      }
      const failed = found.find((f) => f.error);
      if (failed && found.every((f) => f.error)) {
        await notify({ title: t("rule.music"), text: tError(failed.error) });
        return;
      }
      const looked = new Set(found.flatMap((f) => f.files));
      albums = [
        ...albums.filter((a) => !a.files.some((p) => looked.has(p))),
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
      openAlbumReview();
    } catch (e) {
      await notify({ title: t("rule.music"), text: tError(e) });
    } finally {
      mbProgress = null;
    }
  }

  /** Opens the review on the albums without a release (or on one). */
  function openAlbumReview(start?: number) {
    const pending = [
      ...albums.flatMap((a, i) => (a.status === "choose" ? [i] : [])),
      ...albums.flatMap((a, i) => (a.status === "none" ? [i] : [])),
    ];
    const queue = start === undefined ? pending : [start, ...pending.filter((i) => i !== start)];
    albumReview = queue.length ? queue : null;
  }

  /** The release the user chose for an album: its songs are matched to
   *  the tracks; `null` keeps the tags. */
  async function chooseAlbum(index: number, choice: MbCandidate | null, candidates?: MbCandidate[]) {
    const a = albums[index];
    const rule = musicRule?.rule;
    if (!choice || rule?.type !== "music") {
      albums[index] = { ...a, choice: null, matches: {}, tracks: [], status: "none" };
      return;
    }
    const res = await api.mbChoose(choice.id, a.files, rule.fromName);
    albums[index] = {
      ...a,
      choice,
      candidates: candidates ?? a.candidates,
      matches: Object.fromEntries(res.matches),
      tracks: res.tracks,
      status: "chosen",
    };
  }

  /** Where each album with a release ends up after renaming, for its cover. */
  function coverItems(r: ApplyReport): { dir: string; release: string }[] {
    const moved = new Map(r.done.map((op) => [op.from, op.to]));
    const parent = (p: string) => p.slice(0, Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\")));
    const items = new Map<string, string>();
    for (const a of albums) {
      if (!a.choice || !a.files.some((p) => preview.items.some((i) => i.path === p) || moved.has(p))) continue;
      items.set(parent(moved.get(a.files[0]) ?? a.files[0]), a.choice.id);
    }
    return [...items].map(([dir, release]) => ({ dir, release }));
  }

  /** Finds the show (asking when unsure), then each visible file's episode;
   *  doubtful files go to the review window. With no show typed, each show
   *  the file names (or folders) say is looked up in turn. */
  async function searchSeries(rule: Extract<Rule, { type: "series" }>) {
    // AniList: anime, with no key; its shows are kept apart from TMDB's
    // (their ids would meet), under negative numbers.
    const anime = rule.source === "anilist";
    const source = anime ? "anilist" : "tmdb";
    const showKey = (id: number) => (anime ? -id : id);
    if (!anime && !settings.tmdbKey.trim()) {
      const ok = await confirm({ title: t("rule.series"), text: t("tmdbNeedsKey"), ok: t("options"), cancel: t("cancel") });
      if (ok) showOptions = true;
      return;
    }
    const all = preview.items
      .filter((i) => !i.isDir && isScreen(i.original))
      .map((i) => ({ path: i.path, name: readName(i), isDir: i.isDir }));
    if (!all.length) {
      notice = t("tvNoFiles");
      return;
    }
    const key = settings.tmdbKey;
    const lang = tmdbLang();
    const typed = rule.show.trim();
    tvProgress = t("tvFinding");
    try {
      let groups = [all];
      let loose = 0;
      if (!typed) {
        const g = await api.tvGroups(all);
        const byPath = new Map(all.map((i) => [i.path, i]));
        groups = g.groups.map((list) => list.map((p) => byPath.get(p)!));
        loose = g.loose.length;
        if (!groups.length) groups = [all];
      }
      const found: Candidate[] = [];
      let searched = false;
      let review = false;
      const run = stopRun;
      for (const group of groups) {
        if (stopRun !== run) break;
        tvProgress = t("tvFinding");
        const res = await (anime ? api.animeSearch(typed, group) : api.tvSearch(key, lang, typed, group)).catch((e) => {
          // A group whose show cannot be told is skipped, not the others.
          if (groups.length > 1 && String(e) === "tvNoShow") return null;
          throw e;
        });
        if (!res) continue;
        if (stopRun !== run) break;
        const picked = `${source}:${res.query}`;
        let show: Candidate | null = pickedShows[picked] ?? null;
        if (!show) {
          show = res.auto
            ? res.candidates[0]
            : await new Promise<Candidate | null>((resolve) => (showPick = { source, query: res.query, candidates: res.candidates, resolve }));
          showPick = null;
          if (!show) continue;
          pickedShows[picked] = show;
        }
        const sid = showKey(show.id);
        if (!found.some((c) => c.id === show.id)) found.push(show);
        // Files already settled for this show keep their episode.
        const items = group.filter((i) => {
          const e = series[i.path];
          return !(e && e.showId === sid && (e.status === "auto" || e.status === "chosen"));
        });
        if (!items.length) continue;
        if (stopRun !== run) break;
        searched = true;
        tvProgress = t("tvLoading", { show: show.title });
        const look = anime
          ? await api.animeLookup(show.id, items, typed || res.query)
          : await api.tvLookup(key, lang, show.id, items, typed || res.query);
        shows[sid] = look.show;
        if (look.files.some((f) => !f.choice.length)) review = true;
        for (const f of look.files) {
          series[f.path] = {
            showId: sid,
            guess: f.guess,
            alternatives: f.alternatives,
            conflict: f.conflict,
            choice: f.choice.length ? f.choice : null,
            status: f.choice.length ? "auto" : f.alternatives.length ? "choose" : "none",
          };
        }
      }
      if (found.length) searchedShows = found;
      if (!searched && found.length) notice = t("tvNoFiles");
      else if (loose) notice = t("tvLoose", { n: loose });
      if (review) openEpisodeReview();
    } catch (e) {
      await notify({ title: t("rule.series"), text: tError(e) });
    } finally {
      tvProgress = null;
    }
  }

  /** Opens the review on the files still without an episode (or on one). */
  function openEpisodeReview(start?: string) {
    const paths = preview.items.map((i) => i.path);
    const pending = [
      ...paths.filter((p) => series[p]?.status === "choose"),
      ...paths.filter((p) => series[p]?.status === "none"),
    ];
    const queue = start ? [start, ...pending.filter((p) => p !== start)] : pending;
    episodeReview = queue.length ? queue : null;
  }

  /** The user's pick for a file; undecided files read the same way (its
   *  subtitles, say) get it too. */
  function chooseEpisode(path: string, choice: EpKey[] | null) {
    const e = series[path];
    if (!e) return;
    const guess = JSON.stringify(e.guess);
    for (const [p, o] of Object.entries(series)) {
      const undecided = o.status === "choose" || o.status === "none";
      if (p === path || (undecided && o.showId === e.showId && JSON.stringify(o.guess) === guess)) {
        series[p] = { ...o, choice, status: choice ? "chosen" : "none" };
      }
    }
  }

  /** Opens the review on the files still without a movie (or on one). */
  function openReview(start?: string) {
    // Files with options first; those without results after.
    const paths = preview.items.map((i) => i.path);
    const pending = [
      ...paths.filter((p) => movies[p]?.status === "choose"),
      ...paths.filter((p) => movies[p]?.status === "none"),
    ];
    const queue = start ? [start, ...pending.filter((p) => p !== start)] : pending;
    review = queue.length ? queue : null;
  }

  /** The user's pick for a file; undecided files with the same guess (its
   *  subtitles, say) get it too. */
  function chooseMovie(path: string, choice: Candidate | null, candidates?: Candidate[]) {
    const m = movies[path];
    if (!m) return;
    const set = (p: string) => {
      movies[p] = { ...movies[p], choice, status: choice ? "chosen" : "none", candidates: candidates ?? movies[p].candidates };
    };
    set(path);
    for (const [p, o] of Object.entries(movies)) {
      const same = o.guess.title === m.guess.title && o.guess.year === m.guess.year;
      if (p !== path && same && (o.status === "choose" || o.status === "none")) set(p);
    }
  }

  /** Writes .nfo files and images (undone with the rename); tmdbProgress
   *  must be set. Returns how many files were created. */
  async function writeMeta(write: () => Promise<{ created: string[]; errors: [string, string][] }>): Promise<number> {
    try {
      const res = await write();
      if (res.errors.length) {
        await notify({
          title: t("sidecarsTitle"),
          text: t("sidecarsN", { n: res.created.length }),
          items: res.errors.map(([file, why]) => (file ? `${file} — ${tError(why)}` : tError(why))),
        });
      }
      return res.created.length;
    } catch (e) {
      await notify({ title: t("sidecarsTitle"), text: tError(e) });
      return 0;
    } finally {
      tmdbProgress = null;
    }
  }

  /** Saves the list and the hand-edited names before an edit, for Ctrl+Z. */
  /** What the lookups found per file, kept along each list checkpoint
   *  that takes files out (null: the edit left them as they were). */
  type Found = { movies: typeof movies; books: typeof books; comics: typeof comics; comicEdits: typeof comicEdits; songEdits: typeof songEdits; series: typeof series; albums: AlbumState[]; pickedShows: typeof pickedShows; pickedComics: typeof pickedComics; searchedShows: Candidate[] };
  let foundUndo: (Found | null)[] = [];
  const found = (): Found => ({ movies, books, comics, comicEdits, songEdits, series, albums, pickedShows, pickedComics, searchedShows });
  const remember = (keep: Found | null = null) => {
    if (foundUndo.length === 100) foundUndo.shift();
    foundUndo.push(keep);
    return api.checkpoint($state.snapshot(overrides));
  };
  const dropCheckpoint = () => {
    foundUndo.pop();
    return api.dropCheckpoint();
  };
  /** Forgets what was found for files taken out of the list (all of it,
   *  with the shows and series picked, when the list is cleared), so a
   *  file added again starts fresh. */
  function forget(paths: Set<string> | null) {
    if (!paths) {
      subsFound = {};
      movies = {};
      books = {};
      comics = {};
      comicEdits = {};
      songEdits = {};
      series = {};
      albums = [];
      pickedShows = {};
      pickedComics = {};
      searchedShows = [];
      return;
    }
    const keep = <T,>(m: Record<string, T>) => Object.fromEntries(Object.entries(m).filter(([p]) => !paths.has(p)));
    movies = keep(movies);
    subsFound = keep(subsFound);
    books = keep(books);
    comics = keep(comics);
    comicEdits = keep(comicEdits);
    songEdits = keep(songEdits);
    series = keep(series);
    albums = albums
      .map((a) => ({ ...a, files: a.files.filter((p) => !paths.has(p)), matches: keep(a.matches) }))
      .filter((a) => a.files.length);
  }

  /** Undoes the last edit to the list (Ctrl+Z). Renames are undone in the History. */
  async function undoListEdit() {
    const prev = await api.undoList();
    if (prev === null) {
      foundUndo = [];
      notice = t("nothingToUndo");
      return;
    }
    const was = foundUndo.pop();
    if (was) ({ movies, books, comics, comicEdits, songEdits, series, albums, pickedShows, pickedComics, searchedShows } = was);
    overrides = prev;
    await refresh();
    checkIdentical();
    const paths = new Set(preview.items.map((i) => i.path));
    selected = new Set([...selected].filter((p) => paths.has(p)));
    notice = t("listEditUndone");
  }

  async function clearHistory() {
    const ok = await confirm({
      title: t("clearHistory"),
      text: t("clearHistoryText"),
      ok: t("clearHistoryOk"),
      cancel: t("cancel"),
      danger: true,
    });
    if (!ok) return;
    try {
      await api.clearHistory();
    } catch (e) {
      await notify({ title: t("clearHistory"), text: tError(e) });
    }
    history = await api.history();
  }

  async function deleteEntry(id: string) {
    const ok = await confirm({
      title: t("deleteEntry"),
      text: t("deleteEntryText"),
      ok: t("delete"),
      cancel: t("cancel"),
      danger: true,
    });
    if (!ok) return;
    try {
      await api.deleteHistory(id);
    } catch (e) {
      await notify({ title: t("deleteEntry"), text: tError(e) });
    }
    history = await api.history();
  }

  const presetFilter = () => [{ name: t("presetFile"), extensions: ["json"] }];

  async function savePreset() {
    const path = await save({ title: t("saveRules"), filters: presetFilter(), defaultPath: t("defaultPresetName") });
    if (path) {
      await api.savePreset(path, payload());
      notice = t("rulesSaved");
    }
  }

  /** Saves the list as shown to a CSV file. */
  async function exportCsv() {
    const path = await save({
      title: t("exportCsv"),
      filters: [{ name: t("csvFile"), extensions: ["csv"] }],
      defaultPath: t("defaultCsvName"),
    });
    if (!path) return;
    // The optional columns shown go too.
    const extra = DETAIL_COLUMNS.filter((c) => settings.columns.includes(c));
    const details = extra.length ? await api.fileDetails(preview.items.map((i) => i.path)) : {};
    await saveCsv(path, t("exportCsv"), t("csvSaved"), [
      [t("csv.folder"), t("csv.original"), t("csv.new"), t("csv.dest"), t("csv.status"), t("csv.problem"), ...extra.map((c) => t(COL_LABEL[c]))],
      ...preview.items.map((i) => [
        i.dir,
        i.original,
        i.newName,
        i.folder ?? "",
        t(`csv.${i.status}`),
        i.problem ? t(`problem.${i.problem}`) : "",
        ...extra.map((c) => detailText(c, details[i.path])),
      ]),
    ]);
  }

  /** Saves the missing items to a CSV file, by show (or series, album,
   *  collection), season (or disc) and number. */
  async function exportMissing() {
    // In the show's folder, named after the show when there is one.
    const names = [...new Set(ghosts.map((g) => g.group))];
    const kinds = [...new Set(ghosts.map((g) => g.kind))];
    const base = t(kinds.length === 1 ? MISSING[kinds[0]].file : "defaultMissingItemsName");
    const file = `${names.length === 1 ? names[0].replace(/[\\/:*?"<>|]/g, " ").trim() + " - " : ""}${base}.csv`;
    const byPath = new Map(preview.items.map((i) => [i.path, i.dir]));
    const dir = missingFolder(ghosts, (p) => byPath.get(p));
    const path = await save({
      title: t("exportMissing"),
      filters: [{ name: t("csvFile"), extensions: ["csv"] }],
      defaultPath: dir ? `${dir}${/[\\/]$/.test(dir) ? "" : dir.includes("/") ? "/" : "\\"}${file}` : file,
    });
    if (!path) return;
    const list = [...ghosts].sort(
      (a, b) => a.group.localeCompare(b.group) || a.season - b.season || a.number - b.number,
    );
    // Each kind alone with its own columns; mixed, the columns all share.
    const columns: Record<Ghost["kind"], [string[], (g: Ghost) => string[]]> = {
      episode: [
        [t("csv.show"), t("csv.season"), t("csv.episode"), t("csv.code"), t("csv.title"), t("csv.airDate")],
        (g) => [g.group, String(g.season), String(g.number), g.code, g.title, g.date],
      ],
      issue: [[t("csv.series"), t("csv.number"), t("csv.title"), t("csv.coverDate")], (g) => [g.group, g.code.replace(/^#/, ""), g.title, g.date]],
      track: [[t("csv.album"), t("csv.disc"), t("csv.track"), t("csv.title")], (g) => [g.group, String(g.season), String(g.position ?? ""), g.title]],
      movie: [[t("csv.collection"), t("csv.title"), t("csv.releaseDate")], (g) => [g.group, g.title, g.date]],
      book: [[t("csv.series"), t("csv.volume")], (g) => [g.group, g.code]],
    };
    const [head, row]: [string[], (g: Ghost) => string[]] =
      kinds.length === 1
        ? columns[kinds[0]]
        : [[t("csv.group"), t("csv.code"), t("csv.title"), t("csv.date")], (g) => [g.group, g.code, g.title, g.date]];
    await saveCsv(path, t("exportMissing"), t("missingSaved"), [head, ...list.map(row)]);
  }

  /** Writes rows as CSV (`;` between fields where a comma is the decimal
   *  mark, as spreadsheets there expect). */
  async function saveCsv(path: string, title: string, done: string, rows: string[][]) {
    const sep = settings.lang === "en" ? "," : ";";
    const cell = (v: string) => (/[",;\n\r]/.test(v) ? `"${v.replaceAll('"', '""')}"` : v);
    try {
      // The mark at the start tells spreadsheets the file is UTF-8.
      await api.saveText(path, "\ufeff" + rows.map((r) => r.map(cell).join(sep)).join("\r\n") + "\r\n");
      notice = done;
    } catch (e) {
      await notify({ title, text: tError(e) });
    }
  }

  async function loadPreset() {
    const path = await open({ title: t("openRules"), filters: presetFilter() });
    if (typeof path !== "string") return;
    try {
      steps = loadSteps(await api.loadPreset(path), t("rulesLoaded"));
    } catch (e) {
      await notify({ title: t("openRules"), text: tError(e) });
    }
  }

  function onkeydown(e: KeyboardEvent) {
    // An open dialog takes the keyboard; Esc closes only it.
    if (dialog.current) {
      if (e.key === "Escape") closeDialog(false);
      return;
    }
    if (e.key === "Escape") {
      addMenu = ruleMenu = sendMenu = false;
      history = null;
      showOptions = false;
      help = null;
      review = null;
      episodeReview = null;
      albumReview = null;
      bookReview = null;
      comicReview = null;
      showPick?.resolve(null);
      comicPick?.resolve(null);
    }
    if (e.key === "F1") {
      e.preventDefault();
      help ??= "manual";
      return;
    }
    if (history || showOptions || help || review || episodeReview || albumReview || bookReview || comicReview || comicPick || showPick || compare || busy || working) return;
    const mod = e.ctrlKey || e.metaKey;
    const key = e.key.toLowerCase();
    // These work even while typing in a rule.
    if (mod && key === "enter") {
      e.preventDefault();
      applyAll();
      return;
    }
    if (mod && key === "o" && !e.shiftKey) {
      e.preventDefault();
      pickFiles();
      return;
    }
    // Inside a text field, keys keep their usual meaning (Ctrl+Z undoes typing).
    if ((e.target as HTMLElement).closest("input, select, textarea")) return;
    if (e.key === "Delete") removeSelected();
    if (mod && key === "a") {
      e.preventDefault();
      selected = new Set(preview.items.map((i) => i.path));
    }
    if (mod && key === "z" && !e.shiftKey) {
      e.preventDefault();
      undoListEdit();
    }
    if (e.key === "F2" && selected.size === 1) {
      e.preventDefault();
      table?.edit([...selected][0]);
    }
  }

  /** The webview's own menu (Back, Reload…) makes no sense in the app;
   *  text fields keep theirs, for cut, copy and paste. */
  function oncontextmenu(e: MouseEvent) {
    if (!(e.target as HTMLElement).closest("input, textarea")) e.preventDefault();
  }

  function onpointerdown(e: PointerEvent) {
    if (!(e.target as Element).closest(".menu-host")) addMenu = ruleMenu = sendMenu = false;
  }

  onMount(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((e) => {
      dragging = e.payload.type === "enter" || e.payload.type === "over";
      if (e.payload.type === "drop") addPaths(e.payload.paths);
    });
    const unlistenTmdb = listen<{ done: number; total: number }>("tmdb-progress", (e) => {
      if (tmdbProgress) tmdbProgress = { ...tmdbProgress, ...e.payload };
    });
    const unlistenMb = listen<{ done: number; total: number; kind?: "listen" | "tags" }>("mb-progress", (e) => {
      // After listening, the albums heard are looked up.
      if (mbProgress)
        mbProgress = { ...e.payload, kind: e.payload.kind ?? (mbProgress.kind === "listen" ? "search" : mbProgress.kind) };
    });
    const unlistenSubs = listen<{ done: number; total: number }>("subs-progress", (e) => {
      if (subsProgress) subsProgress = { ...subsProgress, ...e.payload };
    });
    const unlistenBooks = listen<{ done: number; total: number }>("book-progress", (e) => {
      if (bookProgress) bookProgress = { ...bookProgress, ...e.payload };
    });
    const unlistenInside = listen<{ done: number; total: number }>("inside-progress", (e) => {
      if (insideProgress) insideProgress = e.payload;
    });
    const unlistenApply = listen<{ done: number; total: number }>("apply-progress", (e) => {
      if (task === "rename") applyProgress = e.payload;
    });
    const unlistenProgress = listen<{ done: number; total: number }>("identical-progress", (e) => {
      identicalProgress = e.payload.done < e.payload.total ? e.payload.done / e.payload.total : null;
    });
    const unlistenPhotos = listen<{ done: number; total: number }>("photos-progress", (e) => {
      photosProgress = e.payload.done < e.payload.total ? e.payload.done / e.payload.total : null;
    });
    const unlistenSimilar = listen<{ done: number; total: number }>("similar-progress", (e) => {
      similarProgress = e.payload.done < e.payload.total ? e.payload.done / e.payload.total : null;
    });
    (async () => {
      await loadSettings();
      // The password store may wait to be unlocked: the program does not.
      loadSecrets();
      if (settings.rememberRules) steps = loadSteps(settings.rules);
      // A rename the program could not finish (it was closed, the
      // computer went off) is put back first.
      try {
        const r = await api.recover();
        const renames = r.restored || r.renamed || r.stuck.length;
        if (renames || r.undone || r.tagged)
          await notify({
            title: t(renames || r.undone ? "recoverTitle" : "recoverTagsTitle"),
            text: [
              renames ? t("recoverText", { restored: r.restored, renamed: r.renamed }) : "",
              r.undone ? t("recoverUndone", { undone: r.undone }) : "",
              r.tagged ? t("recoverTags", { n: r.tagged }) : "",
              r.stuck.length ? t("recoverStuck") : "",
            ].filter(Boolean).join("\n\n"),
            items: r.stuck.map((s) => `${s.from} → ${s.to}`),
          });
      } catch {
        // Nothing to recover.
      }
      ready = true;
      await refresh();
      checkIdentical();
    })();
    return () => {
      unlisten.then((f) => f());
      unlistenProgress.then((f) => f());
      unlistenSimilar.then((f) => f());
      unlistenPhotos.then((f) => f());
      unlistenApply.then((f) => f());
      unlistenTmdb.then((f) => f());
      unlistenMb.then((f) => f());
      unlistenBooks.then((f) => f());
      unlistenSubs.then((f) => f());
      unlistenInside.then((f) => f());
    };
  });
</script>

<svelte:window {onkeydown} {onpointerdown} {oncontextmenu} />

{#if ready}
<div class="app" class:busy>
  <header class="toolbar">
    <div class="group menu-host">
      <button class="primary-soft" disabled={!!working} onclick={() => (addMenu = !addMenu)}>{t("add")} ▾</button>
      {#if addMenu}
        <div class="menu">
          <button onclick={pickFiles}>{t("addFiles")}</button>
          <button onclick={() => pickFolders()}>{t("addFolders")}</button>
          <button onclick={() => pickFolders(true)}>{t("addFolderItems")}</button>
        </div>
      {/if}
    </div>
    <div class="group filter-box">
      <input
        class="filter"
        class:invalid={!!preview.filterError}
        type="search"
        bind:value={filter}
        placeholder={t("filterPlaceholder")}
        title={preview.filterError ? tError(preview.filterError) : t("filterTitle")}
        spellcheck="false"
      />
    </div>
    <div class="group">
      <button class="icon" title={canMove || !selected.size ? t("moveUp") : t("moveDisabled")} disabled={!canMove || !!working} onclick={() => move(-1)}>⤒</button>
      <button class="icon" title={canMove || !selected.size ? t("moveDown") : t("moveDisabled")} disabled={!canMove || !!working} onclick={() => move(1)}>⤓</button>
      <button disabled={!selected.size || !!working} onclick={removeSelected}>{t("remove")}</button>
      <button disabled={!preview.total || !!working} onclick={clearAll}>{t("clear")}</button>
    </div>
    <div class="spacer"></div>
    <div class="group">
      <button onclick={openHistory}>{t("history")}</button>
      <button class="icon help-btn" title="{t("help")} (F1)" aria-label={t("help")} onclick={() => (help = "manual")}>?</button>
      <button class="icon" title={t("options")} aria-label={t("options")} onclick={() => (showOptions = true)}>
        <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
          <path fill="currentColor" fill-rule="evenodd" stroke="currentColor" stroke-width="1" stroke-linejoin="round" d="M12.15 3.76L11.96 1.50L8.04 1.50L7.85 3.76L5.67 5.02L3.62 4.05L1.66 7.45L3.52 8.74L3.52 11.26L1.66 12.55L3.62 15.95L5.67 14.98L7.85 16.24L8.04 18.50L11.96 18.50L12.15 16.24L14.33 14.98L16.38 15.95L18.34 12.55L16.48 11.26L16.48 8.74L18.34 7.45L16.38 4.05L14.33 5.02ZM13 10a3 3 0 1 0-6 0a3 3 0 1 0 6 0Z" />
        </svg>
      </button>
    </div>
    <div class="group menu-host">
      <button class="send" class:on={!!sendTo} title={sendTo ? sendTo.dir : t("sendHint")} disabled={!!working} onclick={() => (sendMenu = !sendMenu)}>
        {sendTo ? t(sendTo.copy ? "sendCopyShort" : "sendMoveShort", { dir: basename(sendTo.dir) }) : t("sendHere")} ▾
      </button>
      {#if sendMenu}
        <div class="menu right">
          <button onclick={() => ((sendTo = null), (sendMenu = false))}>{t("sendHere")}</button>
          <button onclick={() => pickSendTo(false)}>{t("sendMoveTo")}</button>
          <button onclick={() => pickSendTo(true)}>{t("sendCopyTo")}</button>
        </div>
      {/if}
      <button class="primary" title="{MOD}+Enter" disabled={(!preview.changed && !metaOnly.length && !episodeMetaOnly.length && !bookMetaOnly.length && !coverOnly && !playlistOnly.length && !tagsOnly.length && !comicInfoOnly.length && !epubOnly.length && !subsOnly.length) || busy || !!working} onclick={applyAll}>
        {t(sendTo ? (sendTo.copy ? "copyAction" : "moveAction") : "rename")} {preview.changed + metaOnly.length + episodeMetaOnly.length + bookMetaOnly.length + coverOnly + playlistOnly.length + tagsOnly.length + comicInfoOnly.length + epubOnly.length + subsOnly.length || ""}
      </button>
    </div>
  </header>

  <main>
    <!-- The rules wait too: a change now would change the names being worked on. -->
    <aside class="rules" inert={!!working} class:waiting={!!working}>
      <div class="rules-head">
        <h2>{t("rules")}</h2>
        <div class="group menu-host">
          <button class="primary-soft" onclick={() => (ruleMenu = !ruleMenu)}>{t("addRule")}</button>
          {#if ruleMenu}
            <div class="menu">
              {#each ruleTypes as type (type)}
                {@const locked = !!lookupType && isLookup(type)}
                <button disabled={locked} title={locked ? t("oneLookup", { rule: t(`rule.${lookupType}`) }) : undefined} onclick={() => addRule(type)}>{t(`rule.${type}`)}</button>
              {/each}
            </div>
          {/if}
          <button class="icon" title={t("openRules")} onclick={loadPreset}>📂</button>
          <button class="icon" title={t("saveRules")} disabled={!steps.length} onclick={savePreset}>💾</button>
        </div>
      </div>
      <div class="rule-list">
        {#each steps as step, i (step.id)}
          <RuleCard
            bind:step={steps[i]}
            error={ruleError(i)}
            {sample}
            first={i === 0}
            last={i === steps.length - 1}
            onmove={(d) => moveStep(i, d)}
            onremove={() => steps.splice(i, 1)}
            subs={step.rule.type === "movie" || step.rule.type === "series"
              ? {
                  progress: subsProgress ? t(subsProgress.kind === "search" ? "subsSearching" : "subsWriting", subsProgress) : null,
                  summary: subsCounts.subs || subsCounts.missing
                    ? [subsCounts.subs ? t("subsSummary", { subs: subsCounts.subs, videos: subsCounts.videos }) : "", subsCounts.missing ? t("subsMissingN", { n: subsCounts.missing }) : ""].filter(Boolean).join(" · ")
                    : null,
                  found: subsCounts.videos + subsCounts.missing,
                  onsearch: () => searchSubs(),
                  onreview: () => (subsReview = true),
                }
              : undefined}
            lookup={step.rule.type === "movie"
              ? {
                  ...movieCounts,
                  progress: tmdbProgress && tmdbProgress.kind !== "saveTv"
                    ? t(tmdbProgress.kind === "search" ? "tmdbSearching" : "sidecarsWriting", tmdbProgress)
                    : null,
                  onsearch: searchMovies,
                  onreview: () => openReview(),
                }
              : step.rule.type === "series"
                ? {
                    ...seriesCounts,
                    show: searchedShows.length ? searchedShows.map((c) => `${c.title}${c.year ? ` (${c.year})` : ""}`).join(", ") : null,
                    progress: tvProgress ?? (tmdbProgress?.kind === "saveTv" ? t("sidecarsWriting", tmdbProgress) : null),
                    onsearch: () => searchSeries(step.rule as Extract<Rule, { type: "series" }>),
                    onreview: () => openEpisodeReview(),
                  }
                : step.rule.type === "music"
                  ? {
                      ...albumCounts,
                      progress: mbProgress
                        ? t({ search: "mbSearching", listen: "mbListening", save: "coversWriting", tags: "tagsWriting" }[mbProgress.kind] as "mbSearching", mbProgress)
                        : null,
                      onsearch: () => searchAlbums(step.rule as Extract<Rule, { type: "music" }>),
                      onreview: () => openAlbumReview(),
                      edit: {
                        n: selectedSongs.length,
                        typed: Object.keys(songEdits).length,
                        label: t("editButton", { n: selectedSongs.length }),
                        hint: t("songEditButtonHint"),
                        onclick: openSongEdit,
                      },
                    }
                  : step.rule.type === "comic"
                    ? {
                        ...comicCounts,
                        show: null,
                        progress: comicProgress,
                        onsearch: () => searchComics(step.rule as Extract<Rule, { type: "comic" }>),
                        onreview: () => openComicReview(),
                        edit: {
                          n: selectedComics.length,
                          typed: Object.keys(comicEdits).length,
                          label: t("editButton", { n: selectedComics.length }),
                          hint: t("comicEditButtonHint"),
                          onclick: openComicEdit,
                        },
                      }
                  : step.rule.type === "book"
                    ? {
                        ...bookCounts,
                        progress: bookProgress ? t(bookProgress.kind === "search" ? "olSearching" : "sidecarsWriting", bookProgress) : null,
                        onsearch: searchBooks,
                        onreview: () => openBookReview(),
                      }
                    : undefined}
            summary={step.rule.type === "music" && step.enabled && untaggedCount
              ? t(step.rule.listen ? "musicUntaggedListenN" : "musicUntaggedN", { n: untaggedCount })
              : step.rule.type === "photo" && step.enabled && undatedCount
                ? t("photoUndatedN", { n: undatedCount })
                : null}
          />
        {:else}
          <p class="empty">{t("emptyRules")}</p>
        {/each}
      </div>
    </aside>

    <section class="files" class:dragging aria-busy={!!working}>
      {#if working}
        <!-- Undoing happens in the History window: over it too. -->
        <div class="working" class:above={task === "undo"} role="status" aria-live="polite">
          <div class="working-box">
            <span class="working-label"><span class="spinner" aria-hidden="true"></span>{working.label}</span>
            {#if working.total}
              <progress max={working.total} value={working.done}></progress>
            {:else}
              <progress></progress>
            {/if}
            {#if working.stop}
              <button disabled={stopping} onclick={stopSearch}>{stopping ? t("stopping") : t("stop")}</button>
            {/if}
          </div>
        </div>
      {/if}
      {#if preview.total}
        <FileTable
          bind:this={table}
          items={preview.items}
          bind:selected
          {sortKey}
          descending={sortDesc}
          onsort={sortBy}
          onoverride={setOverride}
          {copies}
          {alike}
          {similar}
          oncompare={openCompare}
          {matches}
          ghosts={settings.showMissing ? ghosts : []}
          {outside}
          available={availableColumns}
          onmatch={(p) => {
            if (seriesRule && series[p]) openEpisodeReview(p);
            else if (movieRule && movies[p]) openReview(p);
            else if (musicRule && albumOf.has(p)) openAlbumReview(albumOf.get(p));
            else if (bookRule && books[p]) openBookReview(p);
            else if (comicRule && comics[p]) openComicReview(p);
          }}
        />
        {#if !preview.items.length}<p class="no-match">{t("noMatch")}</p>{/if}
      {:else}
        <div class="drop">
          <p class="big">{t("dropTitle")}</p>
          <p>{t("dropHint")}</p>
        </div>
      {/if}
    </section>
  </main>

  <footer>
    <span>
      {preview.items.length === preview.total
        ? t("files", { n: preview.total })
        : t("shownOf", { shown: preview.items.length, total: preview.total })}
    </span>
    <span class="ok">{t("changed", { n: preview.changed })}</span>
    {#if preview.conflicts}<span class="bad">{t("conflicts", { n: preview.conflicts })}</span>{/if}
    {#if preview.leftFolders.length}
      <span
        class="warn"
        title={preview.leftFolders.map((f) => t("confirmLeftover", { n: f.files, from: basename(f.from), to: basename(f.to) })).join("\n")}
      >{t("leftoverCount", { n: preview.leftFolders.length })}</span>
    {/if}
    {#if selected.size}<span>{t("selected", { n: selected.size })}</span>{/if}
    {#if ghosts.length}
      <span class="missing">
        <input type="checkbox" bind:checked={settings.showMissing} title={t("showMissingHint")} aria-label={t("showMissingHint")} />
        <button class="link faint" title={t("exportMissingHint")} onclick={exportMissing}>{missingCount}</button>
      </span>
    {/if}
    {#if photosProgress !== null}<span>{t("photosReading", { p: Math.floor(photosProgress * 100) })}</span>{/if}
    {#if identicalProgress !== null}
      <span>{t("identicalScanning", { p: Math.floor(identicalProgress * 100) })}</span>
    {:else if identicalCount}
      <button
        class="link"
        class:on={identicalOnly}
        aria-pressed={identicalOnly}
        title={t("identicalFilterHint")}
        onclick={() => (identicalOnly = !identicalOnly)}
      >
        <svg class="copy-icon" viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M5 1.5h7A1.5 1.5 0 0 1 13.5 3v8.5H12V3H5zM2.5 4.5A1.5 1.5 0 0 1 4 3h6a1.5 1.5 0 0 1 1.5 1.5v9A1.5 1.5 0 0 1 10 15H4a1.5 1.5 0 0 1-1.5-1.5zM4 4.5v9h6v-9z" /></svg>
        {t("identicalCount", { n: identicalCount })}
      </button>
    {/if}
    {#if alikeCount && identicalProgress === null}
      <button
        class="link"
        class:on={alikeOnly}
        aria-pressed={alikeOnly}
        title={t("alikeFilterHint")}
        onclick={() => (alikeOnly = !alikeOnly)}
      >♪ {t("alikeCount", { n: alikeCount })}</button>
    {/if}
    {#if similarProgress !== null && identicalProgress === null}
      <span>{t("similarScanning", { p: Math.floor(similarProgress * 100) })}</span>
    {:else if similarCount && identicalProgress === null}
      <button
        class="link"
        class:on={similarOnly}
        aria-pressed={similarOnly}
        title={t("similarFilterHint")}
        onclick={() => (similarOnly = !similarOnly)}
      >
        <svg class="copy-icon" viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" fill-rule="evenodd" d="M2.5 2h11A1.5 1.5 0 0 1 15 3.5v9a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9A1.5 1.5 0 0 1 2.5 2zm0 1.5v7.3l3.2-3.2 2.6 2.6 1.9-1.9 3.3 3.3V3.5zM10.8 4.6a1.4 1.4 0 1 1 0 2.8 1.4 1.4 0 0 1 0-2.8z" /></svg>
        {t("similarCount", { n: similarCount })}
      </button>
    {/if}
    {#if (similarOnly && similarGroups.length) || (identicalOnly && copyGroups.length)}
      <button class="link" title={t("compareHint")} onclick={() => (compare = { kind: similarOnly ? "similar" : "identical", start: 0 })}>{t("compare")}</button>
    {/if}
    {#if preview.items.length}
      <button class="link" title={t("exportCsvHint")} onclick={exportCsv}>{t("exportCsv")}</button>
    {/if}
    <span class="spacer"></span>
    <span class="notice">{notice}</span>
    <button class="link" title={t("donateHint")} onclick={() => openUrl("https://ko-fi.com/rafaellgc")}>♥ {t("donate")}</button>
  </footer>
</div>
{/if}

{#if toast}
  <div class="toast" role="status" aria-live="polite">
    <span class="tick" aria-hidden="true">✓</span>{toast}
    <button class="icon" title={t("close")} aria-label={t("close")} onclick={() => (toast = null)}>✕</button>
  </div>
{/if}

{#if history}
  <div class="overlay">
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <h2>{t("historyTitle")}</h2>
      {#if history.length}
        <ul>
          {#each history as h (h.id)}
            <li>
              <span class="when">{new Date(h.created).toLocaleString(LOCALES[settings.lang])}</span>
              <span class="muted">
                {[
                  h.count || (!h.extra && !h.tagged) ? t("files", { n: h.count }) : "",
                  !h.count && h.extra ? t("metadataN", { n: h.extra }) : "",
                  h.tagged ? t("taggedN", { n: h.tagged }) : "",
                ].filter(Boolean).join(" · ")}
              </span>
              {#if h.undone}
                <span class="muted">{t("undone")}</span>
              {:else if h.failed || h.reverted}
                <span class="failed" title={t("undoFailedHint")}>
                  {h.reverted ? t("partlyUndone", { n: h.reverted, total: h.count }) : t("undoFailed")}
                </span>
                <button onclick={() => undo(h.id)} disabled={busy} title={t("undoFailedHint")}>{t("retry")}</button>
              {:else}
                <button onclick={() => undo(h.id)} disabled={busy}>{t("undo")}</button>
              {/if}
              <button class="icon danger" title={t("deleteEntry")} aria-label={t("deleteEntry")} onclick={() => deleteEntry(h.id)} disabled={busy}>✕</button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="muted">{t("noHistory")}</p>
      {/if}
      <div class="modal-foot">
        {#if history.length}
          <button class="danger-text" onclick={clearHistory} disabled={busy}>{t("clearHistory")}</button>
          <span class="spacer"></span>
        {/if}
        <button onclick={() => (history = null)}>{t("close")}</button>
      </div>
    </div>
  </div>
{/if}

{#if subsReview}
  <SubtitleReview
    rows={preview.items
      .filter((i) => subsFound[i.path])
      .map((i) => {
        const s = subsFound[i.path];
        return { path: i.path, name: i.original, candidates: s.candidates, langs: subsLangs.map((l) => ({ lang: l, have: s.have.includes(l), pick: s.pick[l] ?? -1 })) };
      })}
    remaining={subsRemaining}
    onpick={(path, lang, index) => (subsFound[path].pick[lang] = index)}
    onclose={() => (subsReview = false)}
  />
{/if}

{#if review}
  <MovieReview
    queue={review}
    {movies}
    names={Object.fromEntries(preview.items.map((i) => [i.path, i.original]))}
    onchoose={chooseMovie}
    onclose={() => (review = null)}
  />
{/if}

{#if episodeReview}
  <EpisodeReview
    queue={episodeReview}
    {series}
    {shows}
    names={Object.fromEntries(preview.items.map((i) => [i.path, i.original]))}
    onchoose={chooseEpisode}
    onclose={() => (episodeReview = null)}
  />
{/if}

{#if bookReview}
  <BookReview
    queue={bookReview}
    {books}
    names={Object.fromEntries(preview.items.map((i) => [i.path, i.original]))}
    onchoose={chooseBook}
    onclose={() => (bookReview = null)}
  />
{/if}

{#if comicReview}
  <ComicReview
    queue={comicReview}
    {comics}
    volumes={comicVolumes}
    names={Object.fromEntries(preview.items.map((i) => [i.path, i.original]))}
    onchoose={chooseComic}
    onclose={() => (comicReview = null)}
  />
{/if}

{#if comicPick}
  <ComicPicker source={comicPick.source} skip={comicPick.skip} query={comicPick.query} files={comicPick.files} highest={comicPick.highest} candidates={comicPick.candidates} path={comicPick.path} comic={comicPick.comic} onpick={(v) => comicPick?.resolve(v)} />
{/if}

{#if comicEdit}
  <DetailsEdit
    heading={t("comicEditTitleN", { n: comicEdit.comics.length })}
    hint={t("detailsEditHint")}
    fields={COMIC_FIELDS}
    rows={comicEdit.comics.map((m) => asText(m, COMIC_FIELDS))}
    onsave={saveComicEdit}
    onclose={() => (comicEdit = null)}
  />
{/if}
{#if songEdit}
  <DetailsEdit
    heading={t("songEditTitleN", { n: songEdit.songs.length })}
    hint={t("detailsEditHint")}
    fields={SONG_FIELDS}
    rows={songEdit.songs.map((m) => asText(m, SONG_FIELDS))}
    onsave={saveSongEdit}
    onclose={() => (songEdit = null)}
  />
{/if}

{#if albumReview}
  <AlbumReview queue={albumReview} {albums} onchoose={chooseAlbum} onclose={() => (albumReview = null)} />
{/if}

{#if compare}
  <PictureCompare kind={compare.kind} groups={compare.kind === "similar" ? similarGroups : copyGroups} start={compare.start} onclose={closeCompare} />
{/if}

{#if showPick}
  <ShowPicker source={showPick.source} query={showPick.query} candidates={showPick.candidates} onpick={(c) => showPick?.resolve(c)} />
{/if}

{#if help}
  <Help bind:tab={help} onclose={() => (help = null)} />
{/if}

{#if showOptions}
  <Options onclose={() => (showOptions = false)} />
{/if}

<Dialog />

<style>
  .app { display: flex; flex-direction: column; height: 100vh; }
  .app.busy { cursor: progress; }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    flex-wrap: wrap;
  }
  .group { display: flex; align-items: center; gap: 6px; }
  .menu-host { position: relative; }
  .spacer { flex: 1; }
  .filter-box { flex: 0 1 280px; }
  .filter { width: 100%; }
  .filter.invalid { border-color: var(--danger); outline-color: var(--danger); }
  main { flex: 1; display: flex; min-height: 0; gap: 10px; padding: 10px; }
  .rules { width: clamp(280px, 30vw, 340px); flex: none; display: flex; flex-direction: column; min-height: 0; transition: opacity 0.15s; }
  .rules.waiting { opacity: 0.55; cursor: progress; }
  .rules-head { display: flex; align-items: center; justify-content: space-between; padding: 0 2px 8px; }
  h2 { margin: 0; font-size: 15px; }
  .rule-list { flex: 1; overflow-y: auto; overflow-x: hidden; display: flex; flex-direction: column; gap: 8px; padding-right: 2px; }
  .empty { color: var(--muted); font-size: 13px; margin: 4px 2px; }
  .menu {
    position: absolute;
    left: 0;
    top: calc(100% + 4px);
    min-width: 230px;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: var(--shadow);
    padding: 4px;
    z-index: 5;
  }
  .menu button { border: none; background: none; text-align: left; }
  .menu.right { left: auto; right: 0; }
  .send { max-width: 240px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .send.on { border-color: var(--accent); color: var(--accent); }
  .menu button:hover { background: var(--hover); }
  .files { flex: 1; display: flex; min-width: 0; border-radius: 8px; position: relative; }
  .files.dragging { outline: 2px dashed var(--accent); outline-offset: 2px; }
  .working {
    position: absolute;
    inset: 0;
    z-index: 5;
    display: grid;
    place-items: center;
    border-radius: 8px;
    background: color-mix(in srgb, var(--bg) 55%, transparent);
    cursor: progress;
  }
  .working-box {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    min-width: 260px;
    padding: 18px 24px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    box-shadow: 0 6px 24px rgb(0 0 0 / 0.15);
    font-size: 14px;
  }
  .working progress { width: 100%; accent-color: var(--accent); }
  .working.above { position: fixed; z-index: 15; border-radius: 0; }
  .working-label { display: inline-flex; align-items: center; gap: 8px; }
  /* Small and faint: it only shows that something is going on. */
  .spinner {
    width: 12px;
    height: 12px;
    border: 2px solid var(--border);
    border-top-color: var(--muted);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .spinner { animation-duration: 3s; } }

  .no-match { position: absolute; top: 60px; left: 0; right: 0; text-align: center; color: var(--muted); margin: 0; }
  .drop {
    flex: 1;
    display: grid;
    place-content: center;
    text-align: center;
    border: 2px dashed var(--border);
    border-radius: 8px;
    color: var(--muted);
  }
  .drop .big { font-size: 18px; color: var(--text); margin: 0 0 4px; }
  .toast {
    position: fixed;
    right: 16px;
    bottom: 40px;
    z-index: 50;
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: min(480px, calc(100vw - 32px));
    padding: 8px 14px;
    font-size: 13px;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 3px solid var(--accent);
    border-radius: 6px;
    box-shadow: var(--shadow);
    animation: toast-in 0.18s ease-out;
  }
  .toast .tick { color: var(--accent); font-weight: 700; }
  .toast button.icon { margin: -4px -8px -4px 4px; font-size: 11px; border-color: transparent; background: transparent; }
  @keyframes toast-in {
    from { opacity: 0; transform: translateY(6px); }
  }
  @media (prefers-reduced-motion: reduce) {
    .toast { animation: none; }
  }
  .modal-foot .spacer { flex: 1; }
  button.danger-text { color: var(--danger); }
  footer {
    display: flex;
    gap: 16px;
    padding: 6px 12px;
    font-size: 12px;
    border-top: 1px solid var(--border);
    background: var(--surface);
    color: var(--muted);
  }
  footer .ok { color: var(--accent); }
  footer .faint { color: var(--muted); font-style: italic; }
  footer .missing { display: inline-flex; align-items: center; gap: 5px; }
  footer .bad { color: var(--danger); }
  footer .warn { padding: 0 6px; border-radius: 4px; background: var(--warn); color: black; cursor: help; }
  footer .link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: auto;
    padding: 0 6px;
    border: none;
    border-radius: 4px;
    background: none;
    font-size: 12px;
    color: var(--accent);
  }
  footer .link:hover:not(:disabled) { background: var(--hover); }
  footer .link.on { background: var(--accent-soft); font-weight: 600; }
  .copy-icon { width: 12px; height: 12px; }
  .modal ul { list-style: none; margin: 0; padding: 0; overflow-y: auto; }
  .modal li { display: flex; align-items: center; gap: 12px; padding: 6px 0; border-bottom: 1px solid var(--row-line); }
  .modal li > :first-child { flex: 1; }
  /* The date on one line; a long summary beside it wraps instead. */
  .modal li > .when { white-space: nowrap; }
  .muted { color: var(--muted); }
  .help-btn { font-size: 16px; font-weight: 700; }
  .failed { color: var(--danger); font-size: 13px; }
</style>
