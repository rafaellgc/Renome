<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type WorkGroup } from "$lib/api";
  import { Books } from "$lib/page/books.svelte";
  import { Music } from "$lib/page/music.svelte";
  import { Comics, clearsInfo } from "$lib/page/comics.svelte";
  import { Movies } from "$lib/page/movies.svelte";
  import { Series } from "$lib/page/series.svelte";
  import { Subtitles } from "$lib/page/subtitles.svelte";
  import { Photos } from "$lib/page/photos.svelte";
  import { asText, counts, extOf, isEpub, moved, without, type PageCtx } from "$lib/page/common";
  import Dialog from "$lib/Dialog.svelte";
  import Icon from "$lib/Icon.svelte";
  import RuleSets from "$lib/RuleSets.svelte";
  import { dialog as dialogBox, menu as menuNav } from "$lib/modal";
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
  import { missingBooks, missingEpisodes, missingFolder, missingIssues, missingMovies, missingTracks, notTypedIn, today, type Ghost } from "$lib/missing";
  import MovieReview from "$lib/MovieReview.svelte";
  import Options from "$lib/Options.svelte";
  import RuleCard from "$lib/RuleCard.svelte";
  import ShowPicker from "$lib/ShowPicker.svelte";
  import SubtitleReview from "$lib/SubtitleReview.svelte";
  import { LOCALES, MOD, t, tError, type Key } from "$lib/i18n";
  import { RULE_TYPES, blockedBy, isKindRule, oneLookup, isAudio, isBook, isComic, isScreen, isTaggableVideo, isVideo, newRule, newStep } from "$lib/rules";
  import { SECRET_FIELDS, loadSecrets, loadSettings, saveSecrets, saveSettings, settings, tmdbLang } from "$lib/settings.svelte";
  import type {
    ApplyReport, Copies, MovieStatus, HistoryEntry, OpError, Preview, PreviewItem, Rule, RuleType, Step, View, ViewSort,
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
  /** Of those, the movies, episodes, comics and books (each file's group). */
  let works = $state<Record<string, WorkGroup>>({});
  const worksCount = $derived(Object.keys(works).length);
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

  /** What the rule modules read from the page. */
  const ctx: PageCtx = {
    get items() {
      return preview.items;
    },
    get selected() {
      return selected;
    },
    get steps() {
      return steps;
    },
    readName: (i) => readName(i),
    say: (text) => (notice = text),
    get stopRun() {
      return stopRun;
    },
    openOptions: () => (showOptions = true),
  };
  /** The book rule's lookups, choices and typed details. */
  const books = new Books(ctx);
  /** The music rule's albums, releases and typed tags. */
  const music = new Music(ctx);
  /** The comic rule's series, issues and typed details. */
  const comics = new Comics(ctx);
  /** The movie rule's TMDB results and choices. */
  const movies = new Movies(ctx);
  /** The TV show rule's shows and episodes. */
  const tv = new Series(ctx);
  /** Subtitles for the videos those two identified. */
  const subs = new Subtitles(ctx, movies, tv);
  /** Dates typed in for the photo rule. */
  const photos = new Photos(ctx);
  /** Details being written inside comics and books. */
  let insideProgress = $state<{ done: number; total: number } | null>(null);
  /** Subtitles being fixed or taken out of videos (`part`: how far into
   *  the video being read). */
  let subToolsProgress = $state<{ done: number; total: number; part: number } | null>(null);
  /** The movie or TV show rule writes details inside videos (TMDB only). */
  const videoTagRule = $derived.by(() => {
    const m = movies.rule?.rule;
    if (m?.type === "movie" && m.videoTags) return "movie" as const;
    const s = tv.rule?.rule;
    if (s?.type === "series" && s.videoTags && s.source !== "anilist") return "series" as const;
    return null;
  });
  /** What goes inside a video once renamed: `key` is its path before. */
  function videoTagItem(key: string, path: string) {
    if (videoTagRule === "movie") {
      const c = movies.state[key]?.choice;
      return c && isTaggableVideo(path) ? { path, movie: c.id } : null;
    }
    if (videoTagRule === "series") {
      const m = tv.matches[key];
      return m && m.id > 0 && isTaggableVideo(path) ? { path, show: m.id, season: m.season, episodes: m.episodes } : null;
    }
    return null;
  }
  /** Matched videos that keep their names but get their details inside. */
  const videoTagOnly = $derived(
    videoTagRule ? preview.items.filter((i) => i.status === "unchanged" && !i.isDir && videoTagItem(i.path, i.path)) : [],
  );
  /** Subtitles and videos the subtitle rule works on that keep their names. */
  const subToolsOnly = $derived(
    preview.items.filter((i) => i.status === "unchanged" && ((i.subFix && !i.subFix.noFps) || i.subExtract)),
  );
  /** A long operation under way: the list waits for it, with what it is
   *  doing and how far it got (no count: still finding out). Searches can
   *  be stopped (`stop`). */
  const working = $derived.by((): { label: string; done?: number; total?: number; stop?: boolean } | null => {
    const counted = (label: string, p: { done: number; total: number }, stop: boolean) => ({ label, done: p.done, total: p.total, stop });
    if (insideProgress) return counted(t("insideWriting", insideProgress), insideProgress, false);
    if (subToolsProgress) {
      const p = subToolsProgress;
      return { label: t("subToolsWorking", { done: p.done, total: p.total }), done: p.done + Math.min(p.part, 0.99), total: p.total, stop: true };
    }
    if (subs.progress)
      return counted(t(subs.progress.kind === "search" ? "subsSearching" : "subsWriting", subs.progress), subs.progress, true);
    if (books.progress)
      return counted(t(books.progress.kind === "search" ? "olSearching" : "sidecarsWriting", books.progress), books.progress, books.progress.kind === "search");
    if (movies.progress)
      return counted(t(movies.progress.kind === "search" ? "tmdbSearching" : "sidecarsWriting", movies.progress), movies.progress, movies.progress.kind === "search");
    if (music.progress)
      return counted(
        t({ search: "mbSearching", listen: "mbListening", save: "coversWriting", tags: "tagsWriting" }[music.progress.kind] as "mbSearching", music.progress),
        music.progress,
        music.progress.kind === "search" || music.progress.kind === "listen",
      );
    if (tv.progress) return { label: tv.progress, stop: true };
    if (comics.progress && !comics.pick) return { label: comics.progress, stop: true };
    if (task === "rename" && applyProgress?.total)
      return {
        label: t(sendTo?.copy ? "workingCopy" : "workingMove", { p: Math.floor((applyProgress.done / applyProgress.total) * 100) }),
        done: applyProgress.done,
        total: applyProgress.total,
        stop: true,
      };
    if (task === "undo")
      return undoProgress?.total
        ? {
            label: undoProgress.kind === "tags" ? t("undoTags", undoProgress) : t("undoFiles", { p: Math.floor((undoProgress.done / undoProgress.total) * 100) }),
            done: undoProgress.done,
            total: undoProgress.total,
            stop: true,
          }
        : { label: t("workingUndo"), stop: true };
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
  const photoRule = $derived(steps.find((s) => s.enabled && s.rule.type === "photo"));
  /** Photos and videos the photo rule leaves as they are (no date). */
  const undatedCount = $derived(photoRule ? preview.items.filter((i) => i.photoDate === "none").length : 0);

  /** Aired episodes missing from the seasons the listed videos cover,
   *  issues from the series the listed comics are from, tracks from the
   *  albums, released movies from the collections and volumes between a
   *  book series' files. */
  const ghosts = $derived(notTypedIn(unique([
    ...(tv.rule?.rule.type === "series" && tv.rule.rule.missing
      ? missingEpisodes(tv.shows, tv.matches, preview.items, sortDesc, today())
      : []),
    ...(comics.rule?.rule.type === "comic" && comics.rule.rule.missing
      ? missingIssues(comics.volumes, comics.state, preview.items, sortDesc, today())
      : []),
    ...(music.rule?.rule.type === "music" && music.rule.rule.missing ? missingTracks(music.albums, preview.items, sortDesc) : []),
    ...(movies.rule?.rule.type === "movie" && movies.rule.rule.missing
      ? missingMovies(movies.sets, movies.setOf, Object.fromEntries(Object.entries(movies.matches).map(([p, m]) => [p, m.id])), preview.items, sortDesc, today())
      : []),
    ...(books.rule?.rule.type === "book" && books.rule.rule.missing ? missingBooks(books.state, preview.items, sortDesc) : []),
  ]), preview.items));
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
  /** The ✓ ? ✕ marks next to the new names. */
  const matches = $derived.by(() => {
    const out: Record<string, { status: MovieStatus; tip: string }> = {};
    const tips = {
      movie: { auto: "movieAuto", chosen: "movieChosen", choose: "movieChoose", none: "movieNone" },
      episode: { auto: "episodeAuto", chosen: "episodeChosen", choose: "episodeChoose", none: "episodeNone" },
    } as const;
    if (movies.rule) for (const [p, m] of Object.entries(movies.state)) out[p] = { status: m.status, tip: t(tips.movie[m.status]) };
    if (tv.rule) for (const [p, e] of Object.entries(tv.state)) out[p] = { status: e.status, tip: t(tips.episode[e.status]) };
    if (books.rule) {
      const tip = { auto: "bookAuto", chosen: "bookChosen", choose: "bookChoose", none: "bookNone" } as const;
      for (const [p, b] of Object.entries(books.state)) out[p] = { status: b.status, tip: t(tip[b.status]) };
    }
    if (comics.rule) {
      const tip = { auto: "comicAuto", chosen: "comicChosen", choose: "comicChoose", none: "comicNoneTip" } as const;
      for (const [p, c] of Object.entries(comics.state)) out[p] = { status: c.status, tip: t(tip[c.status]) };
    }
    if (music.rule) {
      const tip = { auto: "albumAuto", chosen: "albumChosen", choose: "albumChoose", none: "albumNone" } as const;
      for (const a of music.albums) {
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
        else if (i.photoDate === "typed") out[i.path] = { status: "chosen", tip: t("photoTypedTip") };
      }
    }
    return out;
  });

  let addMenu = $state(false);
  /** Another folder the files go to (moved or copied); for this session only. */
  let sendTo = $state<{ dir: string; copy: boolean } | null>(null);
  let sendMenu = $state(false);
  /** Bytes copied so far, while copying. */
  /** How far an undo is: songs whose tags are back, then bytes moved
   *  back from another disk. */
  let undoProgress = $state<{ done: number; total: number; kind: "tags" | "files" } | null>(null);
  let applyProgress = $state<{ done: number; total: number } | null>(null);
  let ruleMenu = $state(false);
  let table = $state<FileTable>();

  /** Files the lookup rules on leave alone (a `.zip` in a show's folder),
   *  with why: tagged in the list, which can still name them by hand. */
  const outside = $derived.by(() => {
    const kinds: [boolean, (i: PreviewItem) => boolean, Key][] = [
      [!!movies.rule, (i) => i.isDir || isScreen(i.original), "outsideMovie"],
      [!!tv.rule, (i) => !i.isDir && isScreen(i.original), "outsideSeries"],
      // Songs bring their lyrics, cue sheets, playlists and covers along.
      [!!music.rule, (i) => !i.isDir && (isAudio(i.original) || /\.(lrc|cue|m3u8?|pls|jpe?g|png|webp)$/i.test(i.original)), "outsideMusic"],
      [!!books.rule, (i) => !i.isDir && (isBook(i.original) || !!i.bookTo), "outsideBook"],
      [!!comics.rule, (i) => (i.isDir ? i.convert : isComic(i.original)), "outsideComic"],
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

  const view = $derived<View>({ filter, sortByNewName: sortKey === "newName", descending: sortDesc, identicalOnly, alikeOnly, similarOnly, movies: movies.matches, episodes: tv.matches, songs: music.matches, books: books.matches, comics: comics.matches, dates: photos.dates, leftoverSuffix: t("leftoverSuffix"), sendTo });
  /** Example name for rule cards: the first file in natural name order, so
   *  it does not jump around while the preview re-sorts. */
  const collator = new Intl.Collator(undefined, { numeric: true });
  /** "+ Rule" menu in alphabetical order of the current language. */
  /** The lookup rule in the stack (on or off): the others wait until it
   *  is removed. */
  const lookupType = $derived(steps.find((s) => isKindRule(s.rule.type))?.rule.type ?? null);
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
  /** The kinds of file a rule is made for, then those that edit text. */
  const MEDIA_RULES: RuleType[] = ["movie", "series", "music", "book", "comic", "photo", "subtitle"];
  const ruleGroups = $derived([
    { label: "ruleGroupMedia", types: ruleTypes.filter((r) => MEDIA_RULES.includes(r)) },
    { label: "ruleGroupText", types: ruleTypes.filter((r) => !MEDIA_RULES.includes(r)) },
  ] as const);
  /** What the Rename button does: names and the files that only get data. */
  const applyCount = $derived(preview.changed + movies.metaOnly.length + tv.metaOnly.length + books.metaOnly.length + music.coverOnly + music.playlistOnly.length + music.tagsOnly.length + comics.infoOnly.length + books.epubOnly.length + books.convertOnly.length + subs.only.length + subToolsOnly.length + videoTagOnly.length);
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
      if (run === previewRun) {
        preview = p;
        // An empty list has nothing selected, whatever finished last.
        if (!p.total && selected.size) selected = new Set();
      }
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
      settings.textSize, settings.cacheDays, JSON.stringify(settings.ruleSets), JSON.stringify(steps)];
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
    // Every secret field, so none is left out when one is added.
    const deps = SECRET_FIELDS.map((k) => settings[k]);
    if (!ready || !deps) return;
    clearTimeout(secretsTimer);
    secretsTimer = setTimeout(() => saveSecrets().catch(() => {}), 600);
  });
  /** Saves the secrets now, without waiting for the pause after typing
   *  (the Options window closed, the program closing). */
  function flushSecrets() {
    clearTimeout(secretsTimer);
    return saveSecrets().catch(() => {});
  }
  $effect(() => {
    if (!showOptions) flushSecrets();
  });
  // Closing the program right after typing a key still keeps it.
  getCurrentWindow()
    .onCloseRequested(() => flushSecrets())
    .catch(() => {});

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
    notice = "";
    await refresh();
    checkIdentical();
  }

  async function clearAll() {
    await remember(found());
    // "N files added" and the like no longer say anything.
    notice = "";
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
    if (blockedBy(steps, type)) return;
    steps.push(newStep(newRule(type)));
    ruleMenu = false;
  }

  /** Loaded steps with one lookup rule at most; the notice says which
   *  were left out, else `done` (when given). */
  function loadSteps(loaded: Step[], done = ""): Step[] {
    const { steps: kept, dropped } = oneLookup(loaded);
    if (dropped.length) notice = t("lookupsDropped", { rules: [...new Set(dropped)].map((r) => t(`rule.${r}`)).join(", ") });
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
    const meta = movies.metaOnly.map((i) => ({ path: i.path, id: movies.state[i.path].choice!.id }));
    const tvMeta = tv.metaOnly.map((i) => tv.sidecarItem(i.path, i.path));
    const listOnly = music.playlistOnly;
    const retagOnly = music.tagsOnly.map((i) => i.path);
    const subsUnchanged = subs.only.map((i) => i.path);
    const subsPlanned = subs.wanted === null ? 0 : preview.items.reduce((n, i) => n + (i.status !== "conflict" ? subs.chosen(subs.found[i.path]) : 0), 0);
    const bookMeta = books.metaOnly.map((i) => books.sidecarItem(i.path, i.path));
    /** Books renamed with the format they are converted to. */
    const bookTo = new Map(preview.items.filter((i) => i.bookTo).map((i) => [i.path, i.bookTo!]));
    /** Comics renamed `.cbz` to be converted. */
    const converting = new Set(preview.items.filter((i) => i.convert).map((i) => i.path));
    /** Comics renamed with the e-book format they are made into. */
    const comicTo = new Map(preview.items.filter((i) => i.comicTo).map((i) => [i.path, i.comicTo!]));
    const cleared = new Set(preview.items.filter((i) => i.junk).map((i) => i.path));
    /** What the subtitle rule does to each file: fix a subtitle, take out a video's tracks. */
    const strule = steps.find((s) => s.enabled && s.rule.type === "subtitle")?.rule;
    // Taken now: the preview changes once files are renamed.
    const toolsOnly = subToolsOnly;
    const tagOnly = videoTagOnly;
    const subJobs = new Map(
      preview.items
        .filter((i) => i.status !== "conflict" && ((i.subFix && !i.subFix.noFps) || i.subExtract))
        .map((i) => [i.path, { fix: !!i.subFix && !i.subFix.noFps, extract: !!i.subExtract }]),
    );
    const insideOnly = [
      ...comics.infoOnly.map((i) => comics.insideItem(i.path, i.path, i.convert)),
      ...books.epubOnly.map((i) => books.writeItem(i.path, i.path)),
      ...books.convertOnly.map((i) => books.convertItem(i.path, i.path, i.bookTo!)),
    ];
    if (!preview.changed && !meta.length && !tvMeta.length && !bookMeta.length && !music.coverOnly && !listOnly.length && !retagOnly.length && !insideOnly.length && !subsUnchanged.length && !toolsOnly.length && !tagOnly.length) return;
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
        music.coverOnly ? t("confirmCovers", { n: music.coverOnly }) : "",
        listOnly.length ? t("confirmPlaylists", { n: listOnly.length }) : "",
        retagOnly.length ? t("confirmTags", { n: retagOnly.length }) : "",
        insideOnly.length ? t("confirmInside", { n: insideOnly.length }) : "",
        toolsOnly.length ? t("confirmSubTools", { n: toolsOnly.length }) : "",
        tagOnly.length ? t("confirmInside", { n: tagOnly.length }) : "",
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
      const rule = movies.rule?.rule;
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
          .filter((op) => movies.state[op.from]?.choice && isVideo(op.to))
          .map((op) => ({ path: op.to, id: movies.state[op.from].choice!.id }))
          .concat(meta);
        if (items.length) {
          movies.progress = { done: 0, total: items.length, kind: "save" };
          saved += await writeMeta(() => api.writeSidecars(settings.tmdbKey, tmdbLang(), r.historyId, items, rule.fanart, rule.folder));
        }
      }
      const mrule = music.rule?.rule;
      if (recorded && mrule?.type === "music" && mrule.cover) {
        const items = music.coverItems(r);
        if (items.length) {
          music.progress = { done: 0, total: items.length, kind: "save" };
          try {
            saved += await writeMeta(() => api.writeCovers(r.historyId, items));
          } finally {
            music.progress = null;
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
          .filter((op) => music.matches[op.from] && isAudio(op.to))
          .map((op) => ({ path: op.to, song: music.matches[op.from] }))
          .concat(retagOnly.map((p) => ({ path: p, song: music.matches[p] })));
        if (items.length) {
          music.progress = { done: 0, total: items.length, kind: "tags" };
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
            music.progress = null;
          }
        }
      }
      const brule = books.rule?.rule;
      if (recorded && brule?.type === "book" && (brule.cover || brule.opf)) {
        const items = r.done
          .filter((op) => books.has(op.from) && isBook(op.to))
          .map((op) => books.sidecarItem(op.from, op.to))
          .concat(bookMeta);
        if (items.length) {
          books.progress = { done: 0, total: items.length, kind: "save" };
          try {
            saved += await writeMeta(() => api.writeBookSidecars(r.historyId, $state.snapshot(items), brule.cover, brule.opf, brule.folders));
          } finally {
            books.progress = null;
          }
        }
      }
      let inside = 0;
      let drawnPages = 0;
      {
        const crule = comics.rule?.rule;
        const items = [
          ...(crule?.type === "comic" && (crule.writeInfo || crule.convert || crule.cleanInfo)
            ? r.done
                .filter(
                  (op) =>
                    extOf(op.to) === "cbz" &&
                    ((crule.convert && converting.has(op.from)) ||
                      (crule.writeInfo && comics.matches[op.from]) ||
                      clearsInfo(crule, cleared.has(op.from))),
                )
                .map((op) => comics.insideItem(op.from, op.to, converting.has(op.from)))
            : []),
          ...(crule?.type === "comic" && crule.ebook
            ? r.done.filter((op) => comicTo.has(op.from)).map((op) => comics.ebookItem(op.from, op.to, comicTo.get(op.from)!))
            : []),
          ...(brule?.type === "book"
            ? r.done.filter((op) => !bookTo.has(op.from) && books.writes(op.from, op.to)).map((op) => books.writeItem(op.from, op.to))
            : []),
          ...(brule?.type === "book" && brule.convertTo
            ? r.done.filter((op) => bookTo.has(op.from)).map((op) => books.convertItem(op.from, op.to, bookTo.get(op.from)!))
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
      const srule = tv.rule?.rule;
      if (recorded && srule?.type === "series" && srule.sidecar && srule.source !== "anilist") {
        const items = r.done
          .filter((op) => tv.matches[op.from] && isVideo(op.to))
          .map((op) => tv.sidecarItem(op.from, op.to))
          .concat(tvMeta);
        if (items.length) {
          movies.progress = { done: 0, total: items.length, kind: "saveTv" };
          saved += await writeMeta(() =>
            api.writeTvSidecars(settings.tmdbKey, tmdbLang(), r.historyId, items, srule.fanart, srule.folders),
          );
        }
      }
      if (recorded && videoTagRule) {
        const items = [
          ...r.done.map((op) => videoTagItem(op.from, op.to)),
          ...tagOnly.map((i) => videoTagItem(i.path, i.path)),
        ].filter((i) => i !== null);
        if (items.length) {
          insideProgress = { done: 0, total: items.length };
          try {
            const vr = videoTagRule === "movie" ? movies.rule?.rule : tv.rule?.rule;
            const clean = (vr?.type === "movie" || vr?.type === "series") && !!vr.videoClean;
            const tracks = (vr?.type === "movie" || vr?.type === "series") && !!vr.videoTracks;
            const res = await api.writeVideoTags(settings.tmdbKey, tmdbLang(), r.historyId, items, { cover: true, clean, tracks });
            inside += res.written;
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
      let subTools = { written: 0, created: 0 };
      if (recorded && strule?.type === "subtitle" && subJobs.size) {
        const items = [
          ...r.done.filter((op) => subJobs.has(op.from)).map((op) => ({ path: op.to, ...subJobs.get(op.from)! })),
          ...toolsOnly.map((i) => ({ path: i.path, ...subJobs.get(i.path)! })),
        ];
        if (items.length) {
          subToolsProgress = { done: 0, total: items.length, part: 0 };
          try {
            const tools = { toSrt: strule.toSrt, utf8: strule.utf8, ads: strule.ads, hearing: strule.hearing, shiftMs: strule.shiftMs, fpsFrom: strule.fpsFrom, fpsTo: strule.fpsTo };
            const res = await api.subtitleTools(r.historyId, items, tools, strule.original ?? "keep", strule.extract ? strule.extractLangs : "");
            subTools = res;
            if (res.errors.length) {
              await notify({
                title: t("subToolsTitle"),
                text: t("subToolsN", { n: res.written, m: res.created }),
                items: res.errors.map(([file, why]) => (file ? `${file} — ${tError(why)}` : tError(why))),
              });
            }
          } catch (e) {
            await notify({ title: t("subToolsTitle"), text: tError(e) });
          } finally {
            subToolsProgress = null;
          }
        }
      }
      let subsSaved = 0;
      if (recorded && subs.wanted !== null) {
        const pairs: [string, string][] = [
          ...r.done.filter((op) => isVideo(op.to) && subs.found[op.from]).map((op): [string, string] => [op.from, op.to]),
          ...subsUnchanged.map((p): [string, string] => [p, p]),
        ];
        const items = subs.writes(pairs);
        if (items.length) {
          subs.progress = { done: 0, total: items.reduce((n, i) => n + i.langs.length, 0), kind: "save" };
          try {
            const res = await api.writeSubtitles(subs.auth(), r.historyId, $state.snapshot(items));
            subsSaved = res.created.length;
            if (res.remaining !== null) subs.remaining = res.remaining;
            // Downloaded (or tried): searched again if wanted.
            for (const [from] of pairs) delete subs.found[from];
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
            subs.progress = null;
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
        subTools.written || subTools.created ? t("subToolsN", { n: subTools.written, m: subTools.created }) : "",
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
    undoProgress = null;
    try {
      const r = await api.undo(id);
      foundUndo = [];
      followMatches(r);
      const restored = r.done.filter((op) => !op.copy).length;
      notice = [
        restored || !r.copiesRemoved ? t("restoredN", { n: restored }) : "",
        r.copiesRemoved ? t("copiesRemovedN", { n: r.copiesRemoved }) : "",
        r.stopped ? t("undoStopped") : "",
      ]
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
      // Its own notice says it stopped.
      stopping = false;
      undoProgress = null;
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
      alike = byGroup([...found.songs, ...found.works.map((g) => g.items.map((i) => i.path))]);
      const w: Record<string, WorkGroup> = {};
      for (const g of found.works) for (const i of g.items) w[i.path] = g;
      works = w;
      if (!found.identical.length) identicalOnly = false;
      if (!found.songs.length && !found.works.length) alikeOnly = false;
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
    for (const m of [movies, tv, subs, books, comics, music, photos]) m.follow(follow);
  }

  /** Writes .nfo files and images (undone with the rename); movies.progress
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
      movies.progress = null;
    }
  }

  /** Saves the list and the hand-edited names before an edit, for Ctrl+Z. */
  /** What the lookups found per file, kept along each list checkpoint
   *  that takes files out (null: the edit left them as they were). */
  type Found = { movies: ReturnType<Movies["snapshot"]>; tv: ReturnType<Series["snapshot"]>; books: ReturnType<Books["snapshot"]>; music: ReturnType<Music["snapshot"]>; comics: ReturnType<Comics["snapshot"]>; photos: ReturnType<Photos["snapshot"]> };
  let foundUndo: (Found | null)[] = [];
  const found = (): Found => ({ movies: movies.snapshot(), tv: tv.snapshot(), books: books.snapshot(), music: music.snapshot(), comics: comics.snapshot(), photos: photos.snapshot() });
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
    for (const m of [movies, tv, subs, books, comics, music, photos]) m.forget(paths);
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
    if (was) {
      movies.restore(was.movies);
      tv.restore(was.tv);
      comics.restore(was.comics);
      books.restore(was.books);
      music.restore(was.music);
      photos.restore(was.photos);
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
      movies.review = null;
      tv.review = null;
      music.review = null;
      books.review = null;
      comics.review = null;
      subs.review = false;
      comics.edit = null;
      photos.edit = null;
      books.edit = null;
      music.edit = null;
      tv.pick?.resolve(null);
      comics.pick?.resolve(null);
    }
    if (e.key === "F1") {
      e.preventDefault();
      help ??= "manual";
      return;
    }
    if (history || showOptions || help || movies.review || tv.review || music.review || books.review || comics.review || comics.pick || tv.pick || compare || subs.review || comics.edit || photos.edit || books.edit || music.edit || busy || working) return;
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
      if (movies.progress) movies.progress = { ...movies.progress, ...e.payload };
    });
    const unlistenMb = listen<{ done: number; total: number; kind?: "listen" | "tags" }>("mb-progress", (e) => {
      // After listening, the albums heard are looked up.
      if (music.progress)
        music.progress = { ...e.payload, kind: e.payload.kind ?? (music.progress.kind === "listen" ? "search" : music.progress.kind) };
    });
    const unlistenSubs = listen<{ done: number; total: number }>("subs-progress", (e) => {
      if (subs.progress) subs.progress = { ...subs.progress, ...e.payload };
    });
    const unlistenBooks = listen<{ done: number; total: number }>("book-progress", (e) => {
      if (books.progress) books.progress = { ...books.progress, ...e.payload };
    });
    const unlistenInside = listen<{ done: number; total: number }>("inside-progress", (e) => {
      if (insideProgress) insideProgress = e.payload;
    });
    const unlistenSubTools = listen<{ done: number; total: number; part: number }>("subtools-progress", (e) => {
      if (subToolsProgress) subToolsProgress = e.payload;
    });
    const unlistenApply = listen<{ done: number; total: number }>("apply-progress", (e) => {
      if (task === "rename") applyProgress = e.payload;
    });
    const unlistenUndo = listen<{ done: number; total: number; kind: "tags" | "files" }>("undo-progress", (e) => {
      if (task === "undo") undoProgress = e.payload;
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
      unlistenUndo.then((f) => f());
      unlistenTmdb.then((f) => f());
      unlistenMb.then((f) => f());
      unlistenBooks.then((f) => f());
      unlistenSubs.then((f) => f());
      unlistenInside.then((f) => f());
      unlistenSubTools.then((f) => f());
    };
  });
</script>

<svelte:window {onkeydown} {onpointerdown} {oncontextmenu} />

{#if ready}
<div class="app" class:busy>
  <header class="toolbar">
    <div class="group menu-host">
      <button class="primary-soft with-icon" disabled={!!working} aria-haspopup="menu" aria-expanded={addMenu} onclick={() => (addMenu = !addMenu)}><Icon name="plus" size={15} />{t("add")}<Icon name="chevron-down" size={14} /></button>
      {#if addMenu}
        <div class="menu" role="menu" use:menuNav>
          <button role="menuitem" onclick={pickFiles}><Icon name="file-plus" />{t("addFiles")}</button>
          <button role="menuitem" onclick={() => pickFolders()}><Icon name="folder-plus" />{t("addFolders")}</button>
          <button role="menuitem" onclick={() => pickFolders(true)}><Icon name="folder" />{t("addFolderItems")}</button>
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
      <button class="icon" title={canMove || !selected.size ? t("moveUp") : t("moveDisabled")} disabled={!canMove || !!working} onclick={() => move(-1)}><Icon name="to-top" /></button>
      <button class="icon" title={canMove || !selected.size ? t("moveDown") : t("moveDisabled")} disabled={!canMove || !!working} onclick={() => move(1)}><Icon name="to-bottom" /></button>
      <button class="with-icon squeeze" title={t("remove")} disabled={!selected.size || !!working} onclick={removeSelected}><Icon name="list-minus" /><span>{t("remove")}</span></button>
      <button class="with-icon squeeze" title={t("clear")} disabled={!preview.total || !!working} onclick={clearAll}><Icon name="list-x" /><span>{t("clear")}</span></button>
    </div>
    <div class="spacer"></div>
    <div class="group menu-host">
      <button class="icon" title={t("history")} aria-label={t("history")} onclick={openHistory}><Icon name="history" size={18} /></button>
      <button class="send with-icon" class:on={!!sendTo} title={sendTo ? sendTo.dir : t("sendHint")} disabled={!!working} aria-haspopup="menu" aria-expanded={sendMenu} onclick={() => (sendMenu = !sendMenu)}>
        <span class="send-label">{sendTo ? t(sendTo.copy ? "sendCopyShort" : "sendMoveShort", { dir: basename(sendTo.dir) }) : t("sendHere")}</span><Icon name="chevron-down" size={14} />
      </button>
      {#if sendMenu}
        <div class="menu right" role="menu" use:menuNav>
          <button role="menuitemradio" aria-checked={!sendTo} onclick={() => ((sendTo = null), (sendMenu = false))}><span class="tick">{#if !sendTo}<Icon name="check" size={14} />{/if}</span>{t("sendHere")}</button>
          <button role="menuitemradio" aria-checked={!!sendTo && !sendTo.copy} onclick={() => pickSendTo(false)}><span class="tick">{#if sendTo && !sendTo.copy}<Icon name="check" size={14} />{/if}</span>{t("sendMoveTo")}</button>
          <button role="menuitemradio" aria-checked={!!sendTo?.copy} onclick={() => pickSendTo(true)}><span class="tick">{#if sendTo?.copy}<Icon name="check" size={14} />{/if}</span>{t("sendCopyTo")}</button>
        </div>
      {/if}
      <button class="primary" title="{MOD}+Enter" disabled={(!preview.changed && !movies.metaOnly.length && !tv.metaOnly.length && !books.metaOnly.length && !music.coverOnly && !music.playlistOnly.length && !music.tagsOnly.length && !comics.infoOnly.length && !books.epubOnly.length && !books.convertOnly.length && !subs.only.length && !subToolsOnly.length && !videoTagOnly.length) || busy || !!working} onclick={applyAll}>
        {t(sendTo ? (sendTo.copy ? "copyAction" : "moveAction") : "rename")}{#if applyCount}<span class="count">{applyCount.toLocaleString(LOCALES[settings.lang])}</span>{/if}
      </button>
    </div>
    <span class="divider" aria-hidden="true"></span>
    <div class="group">
      <button class="icon" title={t("options")} aria-label={t("options")} onclick={() => (showOptions = true)}><Icon name="settings" size={18} /></button>
      <button class="icon" title="{t("help")} (F1)" aria-label={t("help")} onclick={() => (help = "manual")}><Icon name="help" size={18} /></button>
    </div>
  </header>

  <main>
    <!-- The rules wait too: a change now would change the names being worked on. -->
    <aside class="rules" inert={!!working} class:waiting={!!working}>
      <div class="rules-head">
        <h2>{t("rules")}</h2>
        <div class="group menu-host">
          <button class="primary-soft with-icon" aria-haspopup="menu" aria-expanded={ruleMenu} onclick={() => (ruleMenu = !ruleMenu)}><Icon name="plus" size={15} />{t("addRule")}</button>
          {#if ruleMenu}
            <div class="menu rules-menu" role="menu" use:menuNav>
              {#each ruleGroups as g (g.label)}
                <p class="menu-group">{t(g.label)}</p>
                {#if lookupType && g.label === "ruleGroupMedia"}<p class="menu-note">{t("oneLookup", { rule: t(`rule.${lookupType}`) })}</p>{/if}
                {#each g.types as type (type)}
                  {@const block = blockedBy(steps, type)}
                  {@const locked = !!block}
                  {@const sub = block && "twice" in block ? t("ruleOnce") : block && "clash" in block ? t("ruleClash", { rule: t(`rule.${block.clash}`) }) : MEDIA_RULES.includes(type) ? t(`ruleSub.${type}` as Key) : ""}
                  <button role="menuitem" disabled={locked} onclick={() => addRule(type)}>
                    <span class="item-text">{t(`rule.${type}`)}{#if sub}<small>{sub}</small>{/if}</span>
                  </button>
                {/each}
              {/each}
            </div>
          {/if}
          <RuleSets
            current={() => (steps.length ? payload() : null)}
            onload={(set) => (steps = loadSteps(set.steps, t("ruleSetLoaded", { name: set.name })))}
            onimport={loadPreset}
            onexport={savePreset}
            onnotice={(text) => (notice = text)}
          />
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
                  progress: subs.progress ? t(subs.progress.kind === "search" ? "subsSearching" : "subsWriting", subs.progress) : null,
                  summary: subs.counts.subs || subs.counts.missing
                    ? [subs.counts.subs ? t("subsSummary", { subs: subs.counts.subs, videos: subs.counts.videos }) : "", subs.counts.missing ? t("subsMissingN", { n: subs.counts.missing }) : ""].filter(Boolean).join(" · ")
                    : null,
                  found: subs.counts.videos + subs.counts.missing,
                  onsearch: () => subs.search(),
                  onreview: () => (subs.review = true),
                }
              : undefined}
            lookup={step.rule.type === "movie"
              ? {
                  ...movies.counts,
                  progress: movies.progress && movies.progress.kind !== "saveTv"
                    ? t(movies.progress.kind === "search" ? "tmdbSearching" : "sidecarsWriting", movies.progress)
                    : null,
                  onsearch: () => movies.search(),
                  onreview: () => movies.openReview(),
                }
              : step.rule.type === "series"
                ? {
                    ...tv.counts,
                    show: tv.searched.length ? tv.searched.map((c) => `${c.title}${c.year ? ` (${c.year})` : ""}`).join(", ") : null,
                    progress: tv.progress ?? (movies.progress?.kind === "saveTv" ? t("sidecarsWriting", movies.progress) : null),
                    onsearch: () => tv.search(step.rule as Extract<Rule, { type: "series" }>),
                    onreview: () => tv.openReview(),
                  }
                : step.rule.type === "music"
                  ? {
                      ...music.counts,
                      progress: music.progress
                        ? t({ search: "mbSearching", listen: "mbListening", save: "coversWriting", tags: "tagsWriting" }[music.progress.kind] as "mbSearching", music.progress)
                        : null,
                      onsearch: () => music.search(step.rule as Extract<Rule, { type: "music" }>),
                      onreview: () => music.openReview(),
                      edit: {
                        n: music.selected.length,
                        typed: Object.keys(music.edits).length,
                        label: t("editButton", { n: music.selected.length }),
                        hint: t("songEditButtonHint"),
                        onclick: () => music.openEdit(),
                      },
                    }
                  : step.rule.type === "comic"
                    ? {
                        ...comics.counts,
                        show: null,
                        progress: comics.progress,
                        onsearch: () => comics.search(step.rule as Extract<Rule, { type: "comic" }>),
                        onreview: () => comics.openReview(),
                        edit: {
                          n: comics.selected.length,
                          typed: Object.keys(comics.edits).length,
                          label: t("editButton", { n: comics.selected.length }),
                          hint: t("comicEditButtonHint"),
                          onclick: () => comics.openEdit(),
                        },
                      }
                  : step.rule.type === "book"
                    ? {
                        ...books.counts,
                        progress: books.progress ? t(books.progress.kind === "search" ? "olSearching" : "sidecarsWriting", books.progress) : null,
                        onsearch: () => books.search(),
                        onreview: () => books.openReview(),
                        edit: {
                          n: books.selected.length,
                          typed: Object.keys(books.edits).length,
                          label: t("editButton", { n: books.selected.length }),
                          hint: t("bookEditButtonHint"),
                          onclick: () => books.openEdit(),
                        },
                      }
                    : undefined}
            edit={step.rule.type === "photo"
              ? {
                  n: photos.selected.length,
                  typed: Object.keys(photos.dates).length,
                  label: t("photoEditButton", { n: photos.selected.length }),
                  hint: t("photoEditButtonHint"),
                  onclick: () => photos.openEdit(),
                }
              : undefined}
            summary={step.rule.type === "music" && step.enabled && music.untagged
              ? t(step.rule.listen ? "musicUntaggedListenN" : "musicUntaggedN", { n: music.untagged })
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
          {works}
          {similar}
          oncompare={openCompare}
          {matches}
          ghosts={settings.showMissing ? ghosts : []}
          {outside}
          available={availableColumns}
          onmatch={(p) => {
            if (tv.rule && tv.state[p]) tv.openReview(p);
            else if (movies.rule && movies.state[p]) movies.openReview(p);
            else if (music.rule && music.albumOf.has(p)) music.openReview(music.albumOf.get(p));
            else if (books.rule && books.state[p]) books.openReview(p);
            else if (comics.rule && comics.state[p]) comics.openReview(p);
          }}
        />
        {#if !preview.items.length}<p class="no-match">{t("noMatch")}</p>{/if}
      {:else}
        <div class="drop">
          <span class="drop-icon"><Icon name="folder-open" size={40} /></span>
          <p class="big">{t("dropTitle")}</p>
          <p>{t("dropHint")}</p>
          <div class="drop-actions">
            <button class="primary with-icon" onclick={pickFiles}><Icon name="file-plus" />{t("addFiles")}</button>
            <button class="with-icon" onclick={() => pickFolders()}><Icon name="folder-plus" />{t("addFolders")}</button>
          </div>
        </div>
      {/if}
    </section>
  </main>

  <footer>
    {#if preview.total}
      <span>
        {preview.items.length === preview.total
          ? t("files", { n: preview.total })
          : t("shownOf", { shown: preview.items.length, total: preview.total })}
      </span>
      <span class="ok">{t("changed", { n: preview.changed })}</span>
    {/if}
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
        title={t(worksCount ? "repeatedFilterHint" : "alikeFilterHint")}
        onclick={() => (alikeOnly = !alikeOnly)}
      >{worksCount === alikeCount ? `≡ ${t("repeatedCount", { n: alikeCount })}` : worksCount ? `♪≡ ${t("repeatedCount", { n: alikeCount })}` : `♪ ${t("alikeCount", { n: alikeCount })}`}</button>
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
    <button class="link" title={t("donateHint")} onclick={() => openUrl("https://ko-fi.com/rafaellgc")}><Icon name="heart" size={13} />{t("donate")}</button>
  </footer>
</div>
{/if}

{#if toast}
  <div class="toast" role="status" aria-live="polite">
    <span class="tick" aria-hidden="true"><Icon name="check" size={15} /></span>{toast}
    <button class="icon" title={t("close")} aria-label={t("close")} onclick={() => (toast = null)}><Icon name="x" size={14} /></button>
  </div>
{/if}

{#if history}
  <div class="overlay">
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="modal md" use:dialogBox onclick={(e) => e.stopPropagation()}>
      <header class="modal-head">
        <h2>{t("historyTitle")}</h2>
        <button class="icon close" title={t("close")} aria-label={t("close")} onclick={() => (history = null)}><Icon name="x" size={18} /></button>
      </header>
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
              <button class="icon danger" title={t("deleteEntry")} aria-label={t("deleteEntry")} onclick={() => deleteEntry(h.id)} disabled={busy}><Icon name="trash" size={15} /></button>
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

{#if subs.review}
  <SubtitleReview
    rows={preview.items
      .filter((i) => subs.found[i.path])
      .map((i) => {
        const s = subs.found[i.path];
        return { path: i.path, name: i.original, candidates: s.candidates, langs: subs.langs.map((l) => ({ lang: l, have: s.have.includes(l), inside: s.inside.includes(l), pick: s.pick[l] ?? -1 })) };
      })}
    remaining={subs.remaining}
    onpick={(path, lang, index) => (subs.found[path].pick[lang] = index)}
    onclose={() => (subs.review = false)}
  />
{/if}

{#if movies.review}
  <MovieReview
    queue={movies.review}
    movies={movies.state}
    names={Object.fromEntries(preview.items.map((i) => [i.path, i.original]))}
    onchoose={(p, c, cs) => movies.choose(p, c, cs)}
    onclose={() => (movies.review = null)}
  />
{/if}

{#if tv.review}
  <EpisodeReview
    queue={tv.review}
    series={tv.state}
    shows={tv.shows}
    names={Object.fromEntries(preview.items.map((i) => [i.path, i.original]))}
    onchoose={(p, c) => tv.choose(p, c)}
    onclose={() => (tv.review = null)}
  />
{/if}

{#if books.review}
  <BookReview
    queue={books.review}
    books={books.state}
    names={Object.fromEntries(preview.items.map((i) => [i.path, i.original]))}
    onchoose={(p, c, cs) => books.choose(p, c, cs)}
    onclose={() => (books.review = null)}
  />
{/if}

{#if comics.review}
  <ComicReview
    queue={comics.review}
    comics={comics.state}
    volumes={comics.volumes}
    names={Object.fromEntries(preview.items.map((i) => [i.path, i.original]))}
    onchoose={(p, i) => comics.choose(p, i)}
    onclose={() => (comics.review = null)}
  />
{/if}

{#if comics.pick}
  <ComicPicker source={comics.pick.source} skip={comics.pick.skip} query={comics.pick.query} files={comics.pick.files} highest={comics.pick.highest} candidates={comics.pick.candidates} path={comics.pick.path} comic={comics.pick.comic} onpick={(v) => comics.pick?.resolve(v)} />
{/if}

{#if comics.edit}
  <DetailsEdit
    heading={t("comicEditTitleN", { n: comics.edit.comics.length })}
    hint={t("detailsEditHint")}
    fields={comics.fields}
    rows={comics.edit.comics.map((m) => asText(m, comics.fields))}
    onsave={(c) => comics.saveEdit(c)}
    onclose={() => (comics.edit = null)}
  />
{/if}
{#if photos.edit}
  <DetailsEdit
    heading={t("photoEditTitleN", { n: photos.edit.rows.length })}
    hint={t("photoEditHint")}
    fields={photos.fields}
    rows={photos.edit.rows}
    onsave={(c) => photos.saveEdit(c)}
    onclose={() => (photos.edit = null)}
  />
{/if}
{#if books.edit}
  <DetailsEdit
    heading={t("bookEditTitleN", { n: books.edit.books.length })}
    hint={t("detailsEditHint")}
    fields={books.fields}
    rows={books.edit.books.map((m) => books.text(m))}
    onsave={(c) => books.saveEdit(c)}
    onclose={() => (books.edit = null)}
  />
{/if}
{#if music.edit}
  <DetailsEdit
    heading={t("songEditTitleN", { n: music.edit.songs.length })}
    hint={t("detailsEditHint")}
    fields={music.fields}
    rows={music.edit.songs.map((m) => asText(m, music.fields))}
    onsave={(c) => music.saveEdit(c)}
    onclose={() => (music.edit = null)}
  />
{/if}

{#if music.review}
  <AlbumReview queue={music.review} albums={music.albums} onchoose={(i, c, cs) => music.choose(i, c, cs)} onclose={() => (music.review = null)} />
{/if}

{#if compare}
  <PictureCompare kind={compare.kind} groups={compare.kind === "similar" ? similarGroups : copyGroups} start={compare.start} onclose={closeCompare} />
{/if}

{#if tv.pick}
  <ShowPicker source={tv.pick.source} query={tv.pick.query} candidates={tv.pick.candidates} onpick={(c) => tv.pick?.resolve(c)} />
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
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    flex-wrap: wrap;
  }
  .group { display: flex; align-items: center; gap: 6px; }
  /* Narrow windows (or large text): these keep only their icons. */
  @media (max-width: 1080px) {
    .squeeze { width: 30px; padding: 0; justify-content: center; }
    .squeeze span { display: none; }
    .filter-box { flex-basis: 140px; min-width: 110px; }
    .toolbar { gap: 8px; flex-wrap: nowrap; }
    .filter-box { flex: 1 1 140px; }
    .send { max-width: 160px; }
  }
  .divider { width: 1px; align-self: stretch; margin: 2px 0; background: var(--border); }
  .spacer { flex: 1; }
  .filter-box { flex: 0 1 280px; min-width: 140px; }
  .filter { width: 100%; }
  .filter.invalid { border-color: var(--danger); outline-color: var(--danger); }
  main { flex: 1; display: flex; min-height: 0; gap: 10px; padding: 10px; }
  .rules { width: clamp(280px, 30vw, 340px); flex: none; display: flex; flex-direction: column; min-height: 0; transition: opacity 0.15s; }
  .rules.waiting { opacity: 0.55; cursor: progress; }
  .rules-head { display: flex; align-items: center; justify-content: space-between; padding: 0 2px 8px; }
  h2 { margin: 0; font-size: var(--fs-lg); }
  .rule-list { flex: 1; overflow-y: auto; overflow-x: hidden; display: flex; flex-direction: column; gap: 8px; padding-right: 2px; }
  .empty { color: var(--muted); font-size: var(--fs-md); margin: 4px 2px; }
  .rules-menu { min-width: 260px; max-height: calc(100vh - 140px); overflow-y: auto; }
  .rules-menu button { flex: none; height: auto; min-height: 30px; padding: 4px 10px; }
  .menu-note { margin: 0 10px 4px; font-size: var(--fs-xs); color: var(--warn-text); max-width: 260px; }
  .item-text { display: flex; flex-direction: column; }
  .item-text small { font-size: var(--fs-xs); color: var(--muted); white-space: normal; }
  .send-label { overflow: hidden; text-overflow: ellipsis; }
  .send { max-width: 240px; white-space: nowrap; }
  .send.on { border-color: var(--accent); color: var(--accent); }
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
    font-size: var(--fs-base);
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
  .drop .big { font-size: var(--fs-xl); font-weight: 600; color: var(--text); margin: 0 0 4px; }
  .drop p { margin: 0; }
  .drop-icon { display: inline-flex; justify-content: center; margin-bottom: 12px; color: var(--accent); opacity: 0.85; }
  .drop-actions { display: flex; gap: 8px; justify-content: center; margin-top: 18px; }
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
    font-size: var(--fs-md);
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 3px solid var(--accent);
    border-radius: 6px;
    box-shadow: var(--shadow);
    animation: toast-in 0.18s ease-out;
  }
  .toast .tick { display: inline-flex; color: var(--success); }
  .toast button.icon { margin: -4px -8px -4px 4px; font-size: var(--fs-xs); border-color: transparent; background: transparent; }
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
    align-items: center;
    gap: 16px;
    font-variant-numeric: tabular-nums;
    padding: 6px 12px;
    font-size: var(--fs-sm);
    border-top: 1px solid var(--border);
    background: var(--surface);
    color: var(--muted);
  }
  footer .ok { color: var(--accent); }
  footer .faint { color: var(--muted); font-style: italic; }
  footer .missing { display: inline-flex; align-items: center; gap: 5px; }
  footer .bad { color: var(--danger); }
  footer .warn { padding: 0 6px; border-radius: var(--r-sm); background: var(--warn-soft); color: var(--warn-text); cursor: help; }
  footer .link {
    display: inline-flex;
    transition: background-color 0.12s;
    align-items: center;
    gap: 4px;
    height: auto;
    padding: 0 6px;
    border: none;
    border-radius: 4px;
    background: none;
    font-size: var(--fs-sm);
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
  .failed { color: var(--danger); font-size: var(--fs-md); }
</style>
