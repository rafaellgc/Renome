<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api } from "$lib/api";
  import { authorSpellings } from "$lib/authors";
  import Dialog from "$lib/Dialog.svelte";
  import { closeDialog, confirm, dialog, notify } from "$lib/dialogs.svelte";
  import FileTable from "$lib/FileTable.svelte";
  import Help from "$lib/Help.svelte";
  import AlbumReview from "$lib/AlbumReview.svelte";
  import BookReview from "$lib/BookReview.svelte";
  import ComicPicker from "$lib/ComicPicker.svelte";
  import ComicReview from "$lib/ComicReview.svelte";
  import GcdLogin from "$lib/GcdLogin.svelte";
  import EpisodeReview from "$lib/EpisodeReview.svelte";
  import MovieReview from "$lib/MovieReview.svelte";
  import Options from "$lib/Options.svelte";
  import RuleCard from "$lib/RuleCard.svelte";
  import ShowPicker from "$lib/ShowPicker.svelte";
  import { LOCALES, MOD, t, tError } from "$lib/i18n";
  import { RULE_TYPES, isAudio, isBook, isComic, isVideo, newRule, newStep, splitExt } from "$lib/rules";
  import { loadSettings, saveSettings, settings, tmdbLang } from "$lib/settings.svelte";
  import type {
    AlbumState, ApplyReport, BookCandidate, BookMatch, BookState, Candidate, ComicIssue, ComicMatch, ComicSource, ComicState, ComicVolume, Copies, MbCandidate, SongMatch, EpKey, EpisodeMatch, EpisodeState, MovieMatch, MovieState, MovieStatus, HistoryEntry,
    OpError, Preview, Rule, RuleType, Show, Step, View, ViewSort,
  } from "$lib/types";

  let ready = $state(false);
  let steps = $state<Step[]>([]);
  let overrides = $state<Record<string, string>>({});
  let preview = $state<Preview>({ items: [], ruleErrors: [], filterError: null, total: 0, changed: 0, conflicts: 0 });
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
  let task = $state<"add" | "rename" | "undo" | null>(null);
  let dragging = $state(false);
  let history = $state<HistoryEntry[] | null>(null);
  let showOptions = $state(false);
  let help = $state<"manual" | "about" | "licenses" | null>(null);
  /** Identical copies of each file in the list. */
  let copies = $state<Record<string, Copies>>({});
  let identicalOnly = $state(false);
  /** Search progress (0–1) while reading whole files; null when idle. */
  let identicalProgress = $state<number | null>(null);
  const identicalCount = $derived(Object.keys(copies).length);

  /** TMDB results and choices per file, for the movie rule. */
  let movies = $state<Record<string, MovieState>>({});
  /** Files in the movie review window; null when closed. */
  let review = $state<string[] | null>(null);
  let tmdbProgress = $state<{ done: number; total: number; kind: "search" | "save" | "saveTv" } | null>(null);
  const movieRule = $derived(steps.find((s) => s.enabled && s.rule.type === "movie"));
  const movieMatches = $derived(
    Object.fromEntries(
      Object.entries(movies)
        .filter(([, m]) => m.choice)
        .map(([p, m]): [string, MovieMatch] => {
          const c = m.choice!;
          return [p, { id: c.id, title: c.title, originalTitle: c.originalTitle, year: c.year }];
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

  /** Open Library results and choices per file, for the book rule. */
  let books = $state<Record<string, BookState>>({});
  /** Files in the book review window; null when closed. */
  let bookReview = $state<string[] | null>(null);
  let bookProgress = $state<{ done: number; total: number; kind: "search" | "save" } | null>(null);
  /** A long operation under way: the list waits for it, with what it is
   *  doing and how far it got (no count: still finding out). */
  const working = $derived.by((): { label: string; done?: number; total?: number } | null => {
    const counted = (label: string, p: { done: number; total: number }) => ({ label, done: p.done, total: p.total });
    if (bookProgress) return counted(t(bookProgress.kind === "search" ? "olSearching" : "sidecarsWriting", bookProgress), bookProgress);
    if (tmdbProgress) return counted(t(tmdbProgress.kind === "search" ? "tmdbSearching" : "sidecarsWriting", tmdbProgress), tmdbProgress);
    if (mbProgress)
      return counted(
        t({ search: "mbSearching", listen: "mbListening", save: "coversWriting", tags: "tagsWriting" }[mbProgress.kind] as "mbSearching", mbProgress),
        mbProgress,
      );
    if (tvProgress) return { label: tvProgress };
    if (comicProgress && !comicPick && !gcdLogin) return { label: comicProgress };
    if (task) return { label: t({ add: "workingAdd", rename: "workingRename", undo: "workingUndo" }[task] as "workingAdd") };
    return null;
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
  /** What the cover and metadata.opf of a book need; `key` is the path before renaming. */
  function bookSidecarItem(key: string, path: string) {
    const c = books[key].choice!;
    return { path, book: bookMatches[key], cover: c.cover, language: c.exact ? c.language : books[key].guess.language };
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
  let comicPick = $state<{ source: ComicSource; query: string; files: number; candidates: ComicVolume[]; resolve: (v: ComicVolume | null) => void } | null>(null);
  /** The Grand Comics Database sign-in, while asked. */
  let gcdLogin = $state<{ resolve: (how: "in" | "anonymous" | null) => void } | null>(null);
  /** How this session uses the GCD: signed in, without an account, or not asked yet. */
  let gcdSession = $state<"in" | "anonymous" | null>(null);
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
    };
  }
  const comicMatches = $derived(
    Object.fromEntries(
      Object.entries(comics).flatMap(([p, c]): [string, ComicMatch][] => {
        const v = comicVolumes[c.volumeKey];
        if (!v || c.choice == null) return [];
        // "": the series alone (MangaDex).
        const issue = c.choice === "" ? undefined : v.issues.find((i) => i.id === c.choice);
        return c.choice === "" || issue ? [[p, withIssue(c.own, v.volume, issue)]] : [];
      }),
    ),
  );
  const comicCounts = $derived(counts(comics));
  /** TMDB episodes per file, for the series rule. */
  let series = $state<Record<string, EpisodeState>>({});
  /** Shows fetched, by id. */
  let shows = $state<Record<number, Show>>({});
  /** The show picked last, and the text it was found by. */
  let pickedShow = $state<{ query: string; show: Candidate } | null>(null);
  /** Files in the episode review window; null when closed. */
  let episodeReview = $state<string[] | null>(null);
  /** The show picker, while open. */
  let showPick = $state<{ query: string; candidates: Candidate[]; resolve: (c: Candidate | null) => void } | null>(null);
  let tvProgress = $state<string | null>(null);
  const seriesRule = $derived(steps.find((s) => s.enabled && s.rule.type === "series"));
  const musicRule = $derived(steps.find((s) => s.enabled && s.rule.type === "music"));
  const untaggedCount = $derived(musicRule ? preview.items.filter((i) => i.untagged).length : 0);

  /** Albums looked up on MusicBrainz, for the music rule. */
  let albums = $state<AlbumState[]>([]);
  /** Albums in the review window (indexes); null when closed. */
  let albumReview = $state<number[] | null>(null);
  let mbProgress = $state<{ done: number; total: number; kind: "search" | "listen" | "save" | "tags" } | null>(null);
  /** Each song's details from its chosen release. */
  const songMatches = $derived(
    Object.fromEntries(albums.flatMap((a) => (a.choice ? Object.entries(a.matches) : []))) as Record<string, SongMatch>,
  );
  /** The album each song is in. */
  const albumOf = $derived(new Map(albums.flatMap((a, i) => a.files.map((p): [string, number] => [p, i]))));
  /** Albums whose songs keep their names but still get a cover (an
   *  existing cover is kept when writing). */
  const coverOnly = $derived.by(() => {
    if (musicRule?.rule.type !== "music" || !musicRule.rule.cover) return 0;
    const status = new Map(preview.items.map((i) => [i.path, i.status]));
    return albums.filter((a) => a.choice && a.files.every((p) => status.get(p) === "unchanged")).length;
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
  const episodeMatches = $derived(
    Object.fromEntries(
      Object.entries(series)
        .filter(([, e]) => e.choice && shows[e.showId])
        .map(([p, e]): [string, EpisodeMatch] => {
          const show = shows[e.showId];
          const keys = e.choice!;
          return [p, {
            id: show.id,
            show: show.name,
            originalShow: show.originalName,
            year: show.year,
            season: keys[0][0],
            episodes: keys.map((k) => k[1]),
            titles: keys.map((k) => show.episodes.find((x) => x.season === k[0] && x.number === k[1])?.name ?? ""),
          }];
        }),
    ),
  );
  const seriesCounts = $derived(counts(series));
  /** Episodes whose names are already right but that still get metadata. */
  const episodeMetaOnly = $derived(
    seriesRule?.rule.type === "series" && seriesRule.rule.sidecar
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
      for (const a of albums) for (const p of a.files) out[p] = { status: a.status, tip: t(tip[a.status]) };
      for (const i of preview.items) if (i.untagged) out[i.path] = { status: "none", tip: t("musicUntaggedTip") };
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
  let ruleMenu = $state(false);
  let table = $state<FileTable>();

  const view = $derived<View>({ filter, sortByNewName: sortKey === "newName", descending: sortDesc, identicalOnly, movies: movieMatches, episodes: episodeMatches, songs: songMatches, books: bookMatches, comics: comicMatches });
  /** Example name for rule cards: the first file in natural name order, so
   *  it does not jump around while the preview re-sorts. */
  const collator = new Intl.Collator(undefined, { numeric: true });
  /** "+ Rule" menu in alphabetical order of the current language. */
  const ruleTypes = $derived(
    [...RULE_TYPES].sort((a, b) => t(`rule.${a}`).localeCompare(t(`rule.${b}`), LOCALES[settings.lang])),
  );
  const sample = $derived(
    preview.items.reduce<string>((min, i) => (!min || collator.compare(i.original, min) < 0 ? i.original : min), ""),
  );
  const canMove = $derived(selected.size > 0 && sortKey !== "newName");

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
        else if (typeof v === "number" && k !== "start" && k !== "step") rule[k] = Math.max(0, Math.trunc(v));
      }
      return { ...s, rule } as Step;
    });
  }

  async function refresh() {
    try {
      preview = await api.preview(payload(), $state.snapshot(overrides), $state.snapshot(view));
    } catch (e) {
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
      settings.disableRulesAfterApply, settings.warnWindows, settings.rememberRules, settings.tmdbKey, settings.tmdbLang,
      JSON.stringify(steps)];
    if (!ready || !deps) return;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      settings.rules = settings.rememberRules ? payload() : [];
      saveSettings();
    }, 400);
  });

  $effect(() => {
    document.documentElement.lang = LOCALES[settings.lang];
  });

  /** Re-sorts the list in Rust by the chosen column. */
  async function applySort() {
    if (sortKey === "manual") return;
    if (sortKey === "newName") await api.sortFiles("name", false);
    else await api.sortFiles(sortKey, sortDesc);
  }

  async function sortBy(key: Exclude<ViewSort, "manual">) {
    if (key === sortKey) sortDesc = !sortDesc;
    else {
      sortKey = key;
      sortDesc = false;
    }
    await applySort();
    await refresh();
  }

  async function addPaths(paths: string[], foldersAsItems = false) {
    if (!paths.length) return;
    busy = true;
    task = "add";
    try {
      await remember();
      const r = await api.addPaths(paths, { recursive: true, includeHidden: settings.includeHidden, foldersAsItems });
      if (!r.added.length) await api.dropCheckpoint();
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

  async function removeSelected() {
    if (!selected.size) return;
    await remember();
    await api.removeFiles([...selected]);
    selected = new Set();
    await refresh();
    checkIdentical();
  }

  async function clearAll() {
    await remember();
    await api.clearFiles();
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
    steps.push(newStep(newRule(type)));
    ruleMenu = false;
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

  const basename = (p: string) => p.split("/").pop() ?? p;
  const errText = (e: OpError) => {
    if (e.kind === "history") return `${t("err.history")} (${e.detail})`;
    const why =
      e.kind === "io" ? e.detail || t("err.io") : e.kind === "tags" ? `${t("err.tags")}: ${tError(e.detail)}` : t(`err.${e.kind}`);
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
    const bookMeta = bookMetaOnly.map((i) => bookSidecarItem(i.path, i.path));
    if (!preview.changed && !meta.length && !tvMeta.length && !bookMeta.length && !coverOnly && !listOnly.length && !retagOnly.length) return;
    if (settings.confirmApply) {
      const lines = [
        preview.changed ? t("confirmText", { n: preview.changed }) : "",
        meta.length + tvMeta.length ? t("confirmMeta", { n: meta.length + tvMeta.length }) : "",
        bookMeta.length ? t("confirmBookMeta", { n: bookMeta.length }) : "",
        coverOnly ? t("confirmCovers", { n: coverOnly }) : "",
        listOnly.length ? t("confirmPlaylists", { n: listOnly.length }) : "",
        retagOnly.length ? t("confirmTags", { n: retagOnly.length }) : "",
        preview.conflicts ? t("confirmConflicts", { n: preview.conflicts }) : "",
      ].filter(Boolean);
      const ok = await confirm({
        title: t("confirmTitle"),
        text: `${lines.join("\n")}\n\n${t("confirmUndoHint")}`,
        ok: t("confirmOk"),
        cancel: t("cancel"),
      });
      if (!ok) return;
    }
    busy = true;
    task = "rename";
    try {
      const rule = movieRule?.rule;
      const r: ApplyReport = preview.changed
        ? await api.apply(payload(), $state.snapshot(overrides), $state.snapshot(view))
        : { done: [], errors: [], historyId: null, lists: 0, listsRemoved: 0 };
      let saved = 0;
      if (rule?.type === "movie" && rule.sidecar) {
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
      if (mrule?.type === "music" && mrule.cover) {
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
      if (mrule?.type === "music" && mrule.playlist) {
        const parent = (p: string) => p.slice(0, Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\")));
        const dirs = [...new Set(r.done.filter((op) => isAudio(op.to)).map((op) => parent(op.to)).concat(listOnly))];
        if (dirs.length) saved += await writeMeta(() => api.writePlaylists(r.historyId, dirs));
      }
      let tagged = 0;
      if (mrule?.type === "music" && mrule.writeTags) {
        // Every matched song that moved, and those that keep their names.
        const items = r.done
          .filter((op) => songMatches[op.from] && isAudio(op.to))
          .map((op) => ({ path: op.to, song: songMatches[op.from] }))
          .concat(retagOnly.map((p) => ({ path: p, song: songMatches[p] })));
        if (items.length) {
          mbProgress = { done: 0, total: items.length, kind: "tags" };
          try {
            const res = await api.writeTags(r.historyId, $state.snapshot(items), mrule.embedCover, mrule.cleanTags);
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
      if (brule?.type === "book" && (brule.cover || brule.opf)) {
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
      const srule = seriesRule?.rule;
      if (srule?.type === "series" && srule.sidecar) {
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
      followMatches(r);
      if (settings.disableRulesAfterApply) for (const s of steps) s.enabled = false;
      overrides = {};
      notice = [
        preview.changed ? t("renamedN", { n: r.done.length }) : "",
        r.lists ? t("listsN", { n: r.lists }) : "",
        r.listsRemoved ? t("listsRemovedN", { n: r.listsRemoved }) : "",
        saved ? t("sidecarsN", { n: saved }) : "",
        tagged ? t("tagsN", { n: tagged }) : "",
        r.errors.length ? t("errorsN", { n: r.errors.length }) : "",
      ].filter(Boolean).join(" · ");
      await refresh();
      checkIdentical();
      if (!r.errors.length && notice) done(notice);
      await showErrors(r, t("resultTitle"), `${t("renamedN", { n: r.done.length })}\n${t("errorsN", { n: r.errors.length })}:`);
    } catch (e) {
      notice = `${t("error")}: ${tError(e)}`;
    } finally {
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
      followMatches(r);
      notice = t("restoredN", { n: r.done.length });
      if (history) history = await api.history();
      await refresh();
      checkIdentical();
      if (!r.errors.length) done(notice);
      await showErrors(
        r,
        r.done.length ? t("resultTitle") : t("undoFailed"),
        `${r.done.length ? `${t("restoredN", { n: r.done.length })}\n` : ""}${t("undoFailedHint")}`,
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
      const groups = await api.findIdentical();
      if (!groups) return;
      const map: Record<string, Copies> = {};
      groups.forEach((g, group) => {
        for (const p of g) map[p] = { group, others: g.filter((x) => x !== p) };
      });
      copies = map;
      if (!groups.length) identicalOnly = false;
      identicalProgress = null;
      await refresh();
    } catch (e) {
      identicalProgress = null;
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
      .filter((i) => movies[i.path]?.status !== "auto" && movies[i.path]?.status !== "chosen")
      .map((i) => ({ path: i.path, name: i.isDir ? i.original : splitExt(i.original)[0] }));
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
      .map((i) => ({ path: i.path, name: splitExt(i.original)[0] }));
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
    for (const op of r.done) {
      if (books[op.from]) {
        books[op.to] = books[op.from];
        delete books[op.from];
      }
      if (comics[op.from]) {
        comics[op.to] = comics[op.from];
        delete comics[op.from];
      }
      if (movies[op.from]) {
        movies[op.to] = movies[op.from];
        delete movies[op.from];
      }
      if (series[op.from]) {
        series[op.to] = series[op.from];
        delete series[op.from];
      }
    }
    const moved = new Map(r.done.map((op) => [op.from, op.to]));
    for (const a of albums) {
      a.files = a.files.map((p) => moved.get(p) ?? p);
      a.matches = Object.fromEntries(Object.entries(a.matches).map(([p, m]) => [moved.get(p) ?? p, m]));
    }
  }

  /** Comic Vine for the visible comics: one series per group of files
   *  (the series typed, else each series the files say), picked by itself
   *  when only one has that name (and year), else asked; then each file's
   *  issue by its number. Files left without one keep the name they say. */
  async function searchComics(rule: Extract<Rule, { type: "comic" }>) {
    const source = rule.source ?? "gcd";
    const key = settings.comicVineKey.trim();
    if (source === "comicvine" && !key) {
      const ok = await confirm({ title: t("rule.comic"), text: t("cvNeedsKey"), ok: t("options"), cancel: t("cancel") });
      if (ok) showOptions = true;
      return;
    }
    // The GCD wants an account: asked once per session, kept in memory only.
    if (source === "gcd" && gcdSession === null) {
      const chosen = await new Promise<"in" | "anonymous" | null>((resolve) => (gcdLogin = { resolve }));
      gcdLogin = null;
      if (!chosen) return;
      gcdSession = chosen;
    }
    const files = preview.items.filter((i) => !i.isDir && isComic(i.original));
    if (!files.length) {
      notice = t("comicNoFiles");
      return;
    }
    comicProgress = t("comicReading");
    const lang = settings.lang === "pt" ? "pt-br" : settings.lang;
    try {
      const owns = await api.comicGuesses(files.map((f) => f.path), !!rule.byFolder);
      const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();
      const groups = new Map<string, { query: string; paths: string[]; owns: ComicMatch[] }>();
      files.forEach((f, i) => {
        const own = owns[i];
        if (!own) return;
        const query = rule.series.trim() || own.series;
        const g = groups.get(fold(query)) ?? { query, paths: [], owns: [] };
        g.paths.push(f.path);
        g.owns.push(own);
        groups.set(fold(query), g);
      });
      if (!groups.size) {
        notice = t("comicNoFiles");
        return;
      }
      for (const [k, g] of groups) {
        // Loose chapters are no issues in the catalogues of issues.
        if (source !== "mangadex" && g.owns.every((o) => !o.number && o.volume == null && !!o.chapter)) continue;
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
          let found = await api.comicSearch(source, key, g.query, lang);
          // `Series: Story` may be a story in `Series`: the series alone.
          const head = g.query.split(/\s*[:–]\s*|\s+-\s+/)[0].trim();
          if (!found.length && head && head !== g.query.trim()) found = await api.comicSearch(source, key, head, lang);
          const year = g.owns.find((o) => o.seriesYear)?.seriesYear ?? null;
          // The highest number among the files: a series with fewer issues
          // is not theirs.
          const highest = Math.max(
            0,
            ...g.owns.map((o) => Number.parseFloat(o.number || (o.chapter ? "" : String(o.volume ?? ""))) || 0),
          );
          const fits = (v: ComicVolume) => !v.count || v.count >= Math.floor(highest);
          const sorted = [...found.filter(fits), ...found.filter((v) => !fits(v))];
          const same = sorted.filter((v) => fold(v.name) === k && (year == null || v.startYear === year) && fits(v));
          // The same name in another language (a French edition for a
          // Brazilian reader, when there is one in Portuguese) is not
          // picked by itself.
          const mine = (v: ComicVolume) => !v.language || v.language.toLowerCase().startsWith(settings.lang);
          const foreign = same.length === 1 && !mine(same[0]) && found.some(mine);
          volume =
            same.length === 1 && !foreign
              ? same[0]
              : await new Promise<ComicVolume | null>((resolve) => (comicPick = { source, query: g.query, files: g.paths.length, candidates: sorted, resolve }));
          comicPick = null;
          if (!volume) continue;
        }
        pickedComics[picked] = volume;
        // MangaDex has series, not a publisher's volumes: the file's
        // numbers stay.
        if (source === "mangadex") {
          const vkey = `${source}:${volume.id}`;
          comicVolumes[vkey] = { volume, issues: [] };
          for (const [i, p] of g.paths.entries()) comics[p] = { volumeKey: vkey, own: g.owns[i], choice: "", status: "auto" };
          continue;
        }
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
      // A refused sign-in is asked again next time.
      if (err.startsWith("gcdBadLogin")) {
        gcdSession = null;
        api.gcdSignOut();
      }
      await notify({ title: t("rule.comic"), text: tError(e) });
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
      albums[index] = { ...a, choice: null, matches: {}, status: "none" };
      return;
    }
    const res = await api.mbChoose(choice.id, a.files, rule.fromName);
    albums[index] = {
      ...a,
      choice,
      candidates: candidates ?? a.candidates,
      matches: Object.fromEntries(res.matches),
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
   *  doubtful files go to the review window. */
  async function searchSeries(rule: Extract<Rule, { type: "series" }>) {
    if (!settings.tmdbKey.trim()) {
      const ok = await confirm({ title: t("rule.series"), text: t("tmdbNeedsKey"), ok: t("options"), cancel: t("cancel") });
      if (ok) showOptions = true;
      return;
    }
    const all = preview.items.map((i) => ({ path: i.path, name: i.original, isDir: i.isDir }));
    if (!all.length) return;
    const key = settings.tmdbKey;
    const lang = tmdbLang();
    tvProgress = t("tvFinding");
    try {
      // The same text as last time: the same show, no need to ask again.
      let show = pickedShow && pickedShow.query === rule.show.trim() && rule.show.trim() ? pickedShow.show : null;
      if (!show) {
        const found = await api.tvSearch(key, lang, rule.show, all);
        show = found.auto
          ? found.candidates[0]
          : await new Promise<Candidate | null>((resolve) => (showPick = { query: found.query, candidates: found.candidates, resolve }));
        showPick = null;
        if (!show) return;
        pickedShow = { query: rule.show.trim(), show };
      }
      // Files already settled for this show keep their episode.
      const items = all.filter((i) => {
        const e = series[i.path];
        return !(e && e.showId === show.id && (e.status === "auto" || e.status === "chosen"));
      });
      if (!items.length) {
        notice = t("tvNoFiles");
        return;
      }
      tvProgress = t("tvLoading", { show: show.title });
      const res = await api.tvLookup(key, lang, show.id, items);
      shows[show.id] = res.show;
      for (const f of res.files) {
        series[f.path] = {
          showId: show.id,
          guess: f.guess,
          alternatives: f.alternatives,
          conflict: f.conflict,
          choice: f.choice.length ? f.choice : null,
          status: f.choice.length ? "auto" : f.alternatives.length ? "choose" : "none",
        };
      }
      if (res.files.some((f) => !f.choice.length)) openEpisodeReview();
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
  const remember = () => api.checkpoint($state.snapshot(overrides));

  /** Undoes the last edit to the list (Ctrl+Z). Renames are undone in the History. */
  async function undoListEdit() {
    const prev = await api.undoList();
    if (prev === null) {
      notice = t("nothingToUndo");
      return;
    }
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

  async function loadPreset() {
    const path = await open({ title: t("openRules"), filters: presetFilter() });
    if (typeof path !== "string") return;
    try {
      steps = (await api.loadPreset(path)).map((s) => newStep(s.rule, s.enabled));
      notice = t("rulesLoaded");
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
      addMenu = ruleMenu = false;
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
      gcdLogin?.resolve(null);
    }
    if (e.key === "F1") {
      e.preventDefault();
      help ??= "manual";
      return;
    }
    if (history || showOptions || help || review || episodeReview || albumReview || bookReview || comicReview || comicPick || gcdLogin || showPick || busy || working) return;
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
    if (!(e.target as Element).closest(".menu-host")) addMenu = ruleMenu = false;
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
    const unlistenBooks = listen<{ done: number; total: number }>("book-progress", (e) => {
      if (bookProgress) bookProgress = { ...bookProgress, ...e.payload };
    });
    const unlistenProgress = listen<{ done: number; total: number }>("identical-progress", (e) => {
      identicalProgress = e.payload.done < e.payload.total ? e.payload.done / e.payload.total : null;
    });
    (async () => {
      await loadSettings();
      if (settings.rememberRules) steps = settings.rules.map((s) => newStep(s.rule, s.enabled));
      ready = true;
      await refresh();
      checkIdentical();
    })();
    return () => {
      unlisten.then((f) => f());
      unlistenProgress.then((f) => f());
      unlistenTmdb.then((f) => f());
      unlistenMb.then((f) => f());
      unlistenBooks.then((f) => f());
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
      <button class="primary" title="{MOD}+Enter" disabled={(!preview.changed && !metaOnly.length && !episodeMetaOnly.length && !coverOnly && !playlistOnly.length && !tagsOnly.length) || busy || !!working} onclick={applyAll}>
        {t("rename")} {preview.changed + metaOnly.length + episodeMetaOnly.length + coverOnly + playlistOnly.length + tagsOnly.length || ""}
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
                <button onclick={() => addRule(type)}>{t(`rule.${type}`)}</button>
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
                    show: pickedShow ? `${pickedShow.show.title}${pickedShow.show.year ? ` (${pickedShow.show.year})` : ""}` : null,
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
                    }
                  : step.rule.type === "comic"
                    ? {
                        ...comicCounts,
                        show: null,
                        progress: comicProgress,
                        onsearch: () => searchComics(step.rule as Extract<Rule, { type: "comic" }>),
                        onreview: () => openComicReview(),
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
          {matches}
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
    {#if selected.size}<span>{t("selected", { n: selected.size })}</span>{/if}
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
    <span class="spacer"></span>
    <span class="notice">{notice}</span>
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
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="overlay" onclick={() => (history = null)}>
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <h2>{t("historyTitle")}</h2>
      {#if history.length}
        <ul>
          {#each history as h (h.id)}
            <li>
              <span>{new Date(h.created).toLocaleString(LOCALES[settings.lang])}</span>
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
  <ComicPicker source={comicPick.source} query={comicPick.query} files={comicPick.files} candidates={comicPick.candidates} onpick={(v) => comicPick?.resolve(v)} />
{/if}

{#if gcdLogin}
  <GcdLogin onclose={(how) => gcdLogin?.resolve(how)} />
{/if}

{#if albumReview}
  <AlbumReview queue={albumReview} {albums} onchoose={chooseAlbum} onclose={() => (albumReview = null)} />
{/if}

{#if showPick}
  <ShowPicker query={showPick.query} candidates={showPick.candidates} onpick={(c) => showPick?.resolve(c)} />
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
  footer .bad { color: var(--danger); }
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
  .muted { color: var(--muted); }
  .help-btn { font-size: 16px; font-weight: 700; }
  .failed { color: var(--danger); font-size: 13px; }
</style>
