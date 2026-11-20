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
  import { Comics } from "$lib/page/comics.svelte";
  import { Movies } from "$lib/page/movies.svelte";
  import { Series } from "$lib/page/series.svelte";
  import { Subtitles } from "$lib/page/subtitles.svelte";
  import { Photos } from "$lib/page/photos.svelte";
  import type { PageCtx } from "$lib/page/common";
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
  import type { Ghost } from "$lib/missing";
  import MovieReview from "$lib/MovieReview.svelte";
  import Options from "$lib/Options.svelte";
  import RuleCard from "$lib/RuleCard.svelte";
  import ShowPicker from "$lib/ShowPicker.svelte";
  import SubtitleReview from "$lib/SubtitleReview.svelte";
  import { LOCALES, MOD, t, tError, type Key } from "$lib/i18n";
  import { TABLE, isKindRule, setRuleTable, type Blocked, newRule, newStep } from "$lib/rules";
  import { loadSecrets, refreshLangs, saveSecrets, saveSettings, secretFields, settings, tmdbLang } from "$lib/settings.svelte";
  import type {
    AfterCounts, AfterReport, ApplyReport, Candidate, Copies, MovieStatus, HistoryEntry, OpError, Preview, PreviewItem, Rule, RuleType, Step, View, ViewSort,
  } from "$lib/types";
  import type { DetailColumn, UpdateFound } from "$lib/types";

  let ready = $state(false);
  let steps = $state<Step[]>([]);
  let overrides = $state<Record<string, string>>({});
  let preview = $state<Preview>({ items: [], ruleErrors: [], filterError: null, total: 0, changed: 0, conflicts: 0, leftFolders: [], trash: [], trashFolders: [], hints: { sample: "", files: 0, pairs: 0, names: [], pairNames: [], steps: [] } });
  // Files that stay because the same file is already where they go
  // (counted apart from the conflicts).
  const sameThere = $derived(preview.items.filter((i) => i.problem === "sameThere").length);
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
  /** The work after the renames under way: its stage (see
   *  `flow::after`), how far it got and, for a video being read, how far
   *  into it (`part`). */
  let afterProgress = $state<{ stage: string; done: number; total: number; part?: number } | null>(null);
  /** The rules that are on, as saved: what is written after the renames reads its options from them. */
  const onRules = () => steps.filter((s) => s.enabled).map((s) => $state.snapshot(s.rule));
  /** What the lookups chose, by each file's path before renaming: what is
   *  written after the renames is decided from it (by the shared flow). */
  function chosen() {
    const pick = <T, U>(o: Record<string, T>, f: (v: T) => U | null | undefined) =>
      Object.fromEntries(Object.entries(o).flatMap(([k, v]) => {
        const u = f(v);
        return u === null || u === undefined ? [] : [[k, u]];
      }));
    return $state.snapshot({
      movies: pick(movies.state, (m) => m.choice?.id),
      episodes: tv.matches,
      songs: music.matches,
      albums: music.albums.filter((a) => a.choice).map((a): [string, string[]] => [a.choice!.id, a.files]),
      books: books.matches,
      bookFound: pick(books.state, (b) => (b.choice ? { guess: b.guess, choice: b.choice } : null)),
      bookTyped: Object.keys(books.edits),
      bookParts: books.parts,
      comics: { ...comics.built, ...pick(comics.edits, (c) => ({ comic: c, web: "", manga: false })) },
      subtitles: subs.found,
      subLangs: subs.langs,
    });
  }
  const NO_COUNTS: AfterCounts = { sidecars: 0, bookMeta: 0, covers: 0, playlists: 0, tags: 0, inside: 0, videoTags: 0, subTools: 0, subtitles: 0, besides: 0 };
  /** What renaming would write besides the renames (asked of the shared flow whenever the list or the choices change). */
  let afterCounts = $state<AfterCounts>(NO_COUNTS);
  $effect(() => {
    const p = preview;
    const rules = onRules();
    const c = chosen();
    if (!p.total || !rules.length) {
      afterCounts = NO_COUNTS;
      return;
    }
    api.afterCounts(rules, c).then((n) => {
      if (p === preview) afterCounts = n;
    }, () => {});
  });
  /** A long operation under way: the list waits for it, with what it is
   *  doing and how far it got (no count: still finding out). Searches can
   *  be stopped (`stop`). */
  const working = $derived.by((): { label: string; done?: number; total?: number; stop?: boolean } | null => {
    const counted = (label: string, p: { done: number; total: number }, stop: boolean) => ({ label, done: p.done, total: p.total, stop });
    if (afterProgress) {
      const p = afterProgress;
      const label = ({ covers: "coversWriting", tags: "tagsWriting", audioTags: "tagsWriting", inside: "insideWriting", subTools: "subToolsWorking", subtitles: "subsWriting" } as Record<string, string>)[p.stage] ?? "sidecarsWriting";
      return { label: t(label as "insideWriting", { done: p.done, total: p.total }), done: p.done + Math.min(p.part ?? 0, 0.99), total: p.total, stop: p.stage === "subTools" || p.stage === "subtitles" };
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
  let ghosts = $state<Ghost[]>([]);
  /** What the lookups found, for what is missing (the shared flow works it out). */
  function known() {
    const songs = Object.assign({}, ...music.albums.filter((a) => a.choice).map((a) => a.matches));
    return $state.snapshot({
      shows: Object.values(tv.shows),
      episodes: tv.matches,
      comics: Object.entries(comics.state).map(([path, c]) => ({ path, volumeKey: c.volumeKey, own: c.own, choice: c.choice })),
      volumes: comics.volumes,
      albums: music.albums.filter((a) => a.choice && a.tracks?.length).map((a) => ({ dir: a.dir, release: a.choice!.id, tracks: a.tracks, files: a.files })),
      songs,
      movies: Object.fromEntries(Object.entries(movies.matches).map(([p, m]) => [p, m.id])),
      setOf: movies.setOf,
      sets: Object.values(movies.sets),
      bookGuesses: Object.fromEntries(Object.entries(books.state).map(([p, b]) => [p, b.guess])),
    });
  }
  $effect(() => {
    const p = preview;
    const rules = onRules();
    const k = known();
    const desc = sortDesc;
    if (!p.total || !rules.some((r) => TABLE.lookups.includes(r.type))) {
      ghosts = [];
      return;
    }
    api.missingItems(rules, k, desc).then((g) => {
      if (p === preview) ghosts = g;
    }, () => {});
  });
  /** How many are missing, in the words for their kind. */
  const MISSING = { episode: "missingEpisodes", issue: "missingIssues", track: "missingTracks", movie: "missingMovies", book: "missingBooks" } as const;
  const missingCount = $derived.by(() => {
    const kinds = [...new Set(ghosts.map((g) => g.kind))];
    return t(kinds.length === 1 ? MISSING[kinds[0]] : "missingItems", { n: ghosts.length });
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
        const key = a.status === "auto" && a.choice?.discogs ? "albumAutoDiscogs" : tip[a.status];
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
  /** How far an undo is: songs whose tags are back, then bytes moved
   *  back from another disk. */
  let undoProgress = $state<{ done: number; total: number; kind: "tags" | "files" } | null>(null);
  let applyProgress = $state<{ done: number; total: number } | null>(null);
  let ruleMenu = $state(false);
  let table = $state<FileTable>();

  /** Files the lookup rules on leave alone (a `.zip` in a show's folder),
   *  with why: tagged in the list, which can still name them by hand. */
  const outside = $derived.by(() => {
    // Which files the rule leaves alone, the Rust core says (`outside`).
    const key = { movie: "outsideMovie", series: "outsideSeries", music: "outsideMusic", book: "outsideBook", comic: "outsideComic" } as const;
    const out: Record<string, string> = {};
    for (const i of preview.items) if (i.outside) out[i.path] = t(key[i.outside]);
    return out;
  });

  const view = $derived<View>({ filter, sortByNewName: sortKey === "newName", descending: sortDesc, identicalOnly, alikeOnly, similarOnly, movies: movies.matches, episodes: tv.matches, songs: music.matches, books: books.matches, comics: comics.matches, dates: photos.dates, leftoverSuffix: t("leftoverSuffix"), lang: settings.lang, numberClashes: settings.numberClashes, sendTo });
  /** The lookup rule in the stack (on or off): the others wait until it
   *  is removed. */
  const lookupType = $derived(steps.find((s) => isKindRule(s.rule.type))?.rule.type ?? null);
  /** The optional columns that make sense with the rules in the stack
   *  (the Rust catalog says, as for the command line's `--csv`). */
  let availableColumns = $state<DetailColumn[]>([]);
  $effect(() => {
    const rules = $state.snapshot(steps);
    api.columnsFor(rules).then((c) => (availableColumns = c), () => {});
  });
  // Sorted by a column no longer shown: the order stays, as a hand-made one.
  $effect(() => {
    if ((TABLE.columns as readonly string[]).includes(sortKey) && !availableColumns.includes(sortKey as DetailColumn)) {
      sortKey = "manual";
    }
  });
  /** The "+ Rule" menu: the groups the Rust catalog makes, each in
   *  alphabetical order of the current language. */
  const byName = (a: RuleType, b: RuleType) => t(`rule.${a}`).localeCompare(t(`rule.${b}`), LOCALES[settings.lang]);
  const ruleGroups = $derived([
    { label: "ruleGroupMedia", types: [...TABLE.media].sort(byName) },
    { label: "ruleGroupText", types: [...TABLE.text].sort(byName) },
  ] as const);
  /** What the Rename button does: names and the files that only get data. */
  const applyCount = $derived(preview.changed + afterCounts.besides);
  /** Example name for rule cards (the preview's: the first in natural order). */
  const sample = $derived(preview.hints.sample);
  const canMove = $derived(selected.size > 0 && sortKey !== "newName");

  function ruleError(i: number) {
    const e = preview.ruleErrors.find((e) => e.index === i);
    return e ? `${t(e.code)}: ${e.detail}` : null;
  }

  /** The rules as they are (Rust reads their numbers as the core takes
   *  them: `catalog::normalize_steps`, the command line's too). */
  function payload(): Step[] {
    return $state.snapshot(steps) as Step[];
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
    // Every setting, so none is left out when one is added (the rules
    // are written here, so they are not one of them).
    const { rules: _rules, ...rest } = settings;
    const deps = [JSON.stringify(rest), JSON.stringify(steps)];
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
    const deps = secretFields().map((k) => settings[k]);
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
    const reads = (TABLE.columns as readonly string[]).includes(key);
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

  /** The language the rules' defaults were loaded in: new rules take the interface's. */
  let tableLang = "";
  $effect(() => {
    const lang = settings.lang;
    if (tableLang && lang !== tableLang) {
      tableLang = lang;
      api.ruleTable(lang).then(setRuleTable, () => {});
    }
  });
  // The catalogues' languages follow the interface's and TMDB's.
  $effect(() => {
    settings.lang;
    settings.tmdbLang;
    if (ready) refreshLangs().catch(() => {});
  });
  /** Why each rule cannot join the rules there, when it cannot (the Rust catalog decides, as for the command line). */
  let blocks = $state<Record<string, Blocked | null>>({});
  $effect(() => {
    const types = steps.map((s) => s.rule.type);
    api.ruleBlocks(types).then((b) => (blocks = b), () => {});
  });

  function addRule(type: RuleType) {
    if (blocks[type]) return;
    steps.push(newStep(newRule(type)));
    ruleMenu = false;
  }

  /** "2 subtitles fixed, 1 extracted from videos", leaving out what is none. */
  const subToolsText = (fixed: number, extracted: number) =>
    [fixed ? t("subToolsN", { n: fixed }) : "", extracted ? t("subToolsM", { n: extracted }) : ""].filter(Boolean).join(", ");

  /** Loaded steps with one lookup rule at most; the notice says which
   *  were left out, else `done` (when given). */
  async function loadSteps(loaded: Step[], done = "", saved = false): Promise<Step[]> {
    // Made whole and up to date by the Rust catalog (`complete_steps`, the
    // command line's too): one lookup rule, every option, all back on
    // when a saved set had them all off.
    const [kept, dropped] = await api.completeSteps($state.snapshot(loaded) as Step[], settings.lang, saved);
    if (dropped.length) notice = t("lookupsDropped", { rules: [...new Set(dropped)].map((r) => t(`rule.${r}`)).join(", ") });
    else if (done) notice = done;
    return kept;
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
    // Taken now: the list changes once files are renamed.
    const rules = onRules();
    const picked = chosen();
    const counts = await api.afterCounts(rules, picked).catch(() => afterCounts);
    if (!preview.changed && !counts.besides) return;
    // Files left behind in an old folder are always told, listed.
    const left = preview.leftFolders.map((f) => t("confirmLeftoverItem", { n: f.files, from: basename(f.from), to: basename(f.to) }));
    // The release's advertisements going to the trash: always told.
    const ads = preview.changed ? preview.items.filter((i) => i.toTrash).length : 0;
    if (settings.confirmApply || left.length || ads) {
      const lines = [
        preview.changed
          ? sendTo
            ? t(sendTo.copy ? "confirmCopy" : "confirmMove", { n: preview.changed, dir: sendTo.dir })
            : t("confirmText", { n: preview.changed })
          : "",
        counts.sidecars ? t("confirmMeta", { n: counts.sidecars }) : "",
        counts.bookMeta ? t("confirmBookMeta", { n: counts.bookMeta }) : "",
        counts.covers ? t("confirmCovers", { n: counts.covers }) : "",
        counts.playlists ? t("confirmPlaylists", { n: counts.playlists }) : "",
        counts.tags ? t("confirmTags", { n: counts.tags }) : "",
        counts.inside + counts.videoTags ? t("confirmInside", { n: counts.inside + counts.videoTags }) : "",
        counts.subTools ? t("confirmSubTools", { n: counts.subTools }) : "",
        counts.subtitles ? t("confirmSubs", { n: counts.subtitles }) : "",
        preview.conflicts - sameThere ? t("confirmConflicts", { n: preview.conflicts - sameThere }) : "",
        sameThere ? t("confirmSameThere", { n: sameThere }) : "",
        ads ? t("confirmTrashAds", { n: ads, folders: preview.trashFolders.length }) : "",
      ].filter(Boolean);
      const ok = await confirm({
        title: t(sendTo ? (sendTo.copy ? "confirmCopyTitle" : "confirmMoveTitle") : "confirmTitle"),
        text: left.length ? `${lines.join("\n")}\n\n${t("confirmLeftoverHead", { n: left.length, suffix: t("leftoverSuffix") })}` : `${lines.join("\n")}\n\n${t("confirmUndoHint")}`,
        items: left.length ? left : undefined,
        after: left.length ? t("confirmUndoHint") : undefined,
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
      // Old folders renamed for their leftovers: not files.
      const leftFrom = new Set(preview.leftFolders.map((f) => f.from));
      const r: ApplyReport = preview.changed
        ? await api.apply(payload(), $state.snapshot(overrides), $state.snapshot(view))
        : { done: [], errors: [], historyId: null, lists: 0, listsRemoved: 0 };
      // The list's undo starts over (as the core's, when it renamed).
      if (preview.changed) foundUndo = [];
      // Everything written after the renames: decided and done by the
      // shared flow (the command line runs the same).
      let after: AfterReport | null = null;
      try {
        after = await api.afterRename(rules, picked, r.done.map((op): [string, string] => [op.from, op.to]), r.historyId, settings.tmdbKey, tmdbLang(), subs.auth());
      } catch (e) {
        await notify({ title: t("resultTitle"), text: tError(e) });
      } finally {
        afterProgress = null;
      }
      if (after) {
        if (after.remaining !== null) subs.remaining = after.remaining;
        followMatches(after.followed);
        // Downloaded (or tried): searched again if wanted.
        if (counts.subtitles) {
          for (const op of r.done) delete subs.found[op.from];
          for (const i of preview.items) if (i.status === "unchanged") delete subs.found[i.path];
        }
        const told: Record<string, [string, (n: number) => string]> = {
          sidecars: ["sidecarsTitle", (n) => t("sidecarsN", { n })],
          tags: ["tagsTitle", (n) => t("tagsN", { n })],
          audioTags: ["tagsTitle", (n) => t("audioTagsN", { n })],
          inside: ["insideTitle", (n) => t("insideN", { n })],
          subTools: ["subToolsTitle", () => subToolsText(after!.subFixed, after!.subExtracted)],
          subtitles: ["subsTitle", (n) => t("subsSavedN", { n })],
        };
        for (const x of after.troubles) {
          const [title, text] = told[x.stage] ?? ["resultTitle", () => ""];
          await notify({
            title: t(title as "resultTitle"),
            text: text(x.done),
            items: x.errors.map(([file, why]) => (file ? `${file} — ${tError(why)}` : tError(why))),
          });
        }
      }
      const a = after ?? { adsTrashed: 0, adsFailed: 0, sidecars: 0, keptLists: 0, tags: 0, audioTags: 0, inside: 0, drawn: 0, subFixed: 0, subExtracted: 0, subtitles: 0 };
      followMatches(r.moved);
      if (settings.disableRulesAfterApply) for (const s of steps) s.enabled = false;
      // The list now holds what was sent: sending it again would send it
      // once more, so the files stay where they are until chosen again.
      if (preview.changed) sendTo = null;
      overrides = {};
      notice = [
        preview.changed ? t(sent ? (sent.copy ? "copiedN" : "movedN") : "renamedN", { n: r.done.filter((op) => !leftFrom.has(op.from)).length }) : "",
        r.stopped ? t("applyStopped") : "",
        r.done.some((op) => leftFrom.has(op.from)) ? t("leftFoldersN", { n: r.done.filter((op) => leftFrom.has(op.from)).length }) : "",
        a.adsTrashed ? t("adsTrashedN", { n: a.adsTrashed }) : "",
        a.adsFailed ? t("adsFailedN", { n: a.adsFailed }) : "",
        r.lists ? t("listsN", { n: r.lists }) : "",
        r.listsRemoved ? t("listsRemovedN", { n: r.listsRemoved }) : "",
        a.sidecars ? t("sidecarsN", { n: a.sidecars }) : "",
        a.subtitles ? t("subsSavedN", { n: a.subtitles }) : "",
        subToolsText(a.subFixed, a.subExtracted),
        a.keptLists ? t("playlistsKeptN", { n: a.keptLists }) : "",
        a.tags ? t("tagsN", { n: a.tags }) : "",
        a.audioTags ? t("audioTagsN", { n: a.audioTags }) : "",
        a.inside ? t("insideN", { n: a.inside }) : "",
        a.drawn ? t("insideDrawnN", { n: a.drawn }) : "",
        r.errors.length ? t("errorsN", { n: r.errors.length }) : "",
      ].filter(Boolean).join(" · ");
      await refresh();
      checkIdentical();
      if (!r.errors.length && notice) done(notice);
      tellServer(r.done);
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

  /** A newer version found on GitHub, and how far its download went
   *  (0–100) once it is being installed. */
  let update = $state<UpdateFound | null>(null);
  let updating = $state<number | null>(null);

  async function installUpdate() {
    const u = update;
    if (!u || updating !== null) return;
    if (u.way === "page") {
      await openUrl(`https://github.com/rafaellgc/Renome/releases/tag/v${u.version}`);
      return;
    }
    if (busy) {
      await notify({ title: t("updateTitle", { v: u.version }), text: t("updateBusy") });
      return;
    }
    const rules = settings.rememberRules ? t("updateRulesKept") : t("updateRulesLost", { opt: t("rememberRules") });
    const list = preview.total ? ` ${t("updateListEmpty")}` : "";
    const ok = await confirm({
      title: t("updateTitle", { v: u.version }),
      text: `${t("updateText", { v: u.version })}\n\n${rules}${list}\n\n${t("updateNews")}`,
      ok: t("updateInstall"),
      cancel: t("cancel"),
    });
    if (!ok) return;
    // -1 until the first bytes arrive: no percentage to show yet.
    updating = -1;
    const off = await listen<{ done: number; total: number | null }>("update-progress", (e) => {
      if (e.payload.total) updating = Math.min(100, Math.floor((e.payload.done / e.payload.total) * 100));
    });
    try {
      await api.updateInstall();
    } catch (e) {
      await notify({ title: t("updateTitle", { v: u.version }), text: tError(e) });
    } finally {
      off();
      updating = null;
    }
  }

  /** Tells the media servers in the options about the folders where
   *  files were renamed, moved or put back (only those inside their
   *  library folders); says so in the notice. */
  function tellServer(done: { from: string; to: string }[]) {
    if (!done.length || !settings.servers.length) return;
    api
      .serverTell($state.snapshot(settings.servers), settings.serverKeys || "{}", done.map((op) => ({ from: op.from, to: op.to })))
      .then((answers) => {
        const texts = answers
          .map(({ name, folders: a }) =>
            "Ok" in a ? (a.Ok ? t("serverTold", { name, n: a.Ok }) : "") : `${t("serverNotTold", { name })}: ${tError(a.Err)}`,
          )
          .filter(Boolean);
        if (texts.length) notice = [notice, ...texts].filter(Boolean).join(" · ");
      })
      .catch(() => {});
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
      followMatches(r.moved);
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
      tellServer(r.done);
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

  /** What was found per file follows the files renamed or restored
   *  (where each went, Rust says: `apply::Follow`). */
  function followMatches(moved: [string, string][] | undefined) {
    if (!moved?.length) return;
    const to = new Map(moved);
    const follow = (p: string) => to.get(p) ?? p;
    for (const m of [movies, tv, subs, books, comics, music, photos]) m.follow(follow);
  }

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

  /** A CSV cell from Rust (`flow::export`): text, or a key of the texts. */
  const cellText = (c: string | { key: string }) => (typeof c === "string" ? c : t(c.key as Key));

  /** Saves the list as shown to a CSV file (the rows Rust makes, as for
   *  the command line's `--csv`). */
  async function exportCsv() {
    const path = await save({
      title: t("exportCsv"),
      filters: [{ name: t("csvFile"), extensions: ["csv"] }],
      defaultPath: t("defaultCsvName"),
    });
    if (!path) return;
    // The optional columns shown go too.
    const extra = TABLE.columns.filter((c) => availableColumns.includes(c) && settings.columns.includes(c));
    const rows = await api.exportRows(extra, settings.lang);
    await saveCsv(path, t("exportCsv"), t("csvSaved"), rows.map((r) => r.map(cellText)));
  }

  /** Saves the missing items to a CSV file: where, under what name, which
   *  rows, Rust says (`flow::export::missing`, the command line's
   *  `--missing-csv` too). */
  async function exportMissing() {
    const csv = await api.exportMissing();
    const file = `${csv.group ? `${csv.group} - ` : ""}${t(csv.nameKey as Key)}.csv`;
    const dir = csv.dir;
    const sep = dir && dir.includes("\\") && !dir.includes("/") ? "\\" : "/";
    const path = await save({
      title: t("exportMissing"),
      filters: [{ name: t("csvFile"), extensions: ["csv"] }],
      defaultPath: dir ? `${dir}${dir.endsWith(sep) ? "" : sep}${file}` : file,
    });
    if (!path) return;
    await saveCsv(path, t("exportMissing"), t("missingSaved"), csv.rows.map((r) => r.map(cellText)));
  }

  /** Writes rows as CSV (`;` between fields where a comma is the decimal
   *  mark, as spreadsheets there expect). */
  async function saveCsv(path: string, title: string, done: string, rows: string[][]) {
    try {
      await api.saveCsv(path, rows, settings.lang);
      notice = done;
    } catch (e) {
      await notify({ title, text: tError(e) });
    }
  }

  async function loadPreset() {
    const path = await open({ title: t("openRules"), filters: presetFilter() });
    if (typeof path !== "string") return;
    try {
      const [loaded, dropped] = await api.loadPreset(path, settings.lang);
      steps = loaded;
      notice = dropped.length ? t("lookupsDropped", { rules: [...new Set(dropped)].map((r) => t(`rule.${r}`)).join(", ") }) : t("rulesLoaded");
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
      addMenu = sendMenu = ruleMenu = false;
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
      if (afterProgress) afterProgress = { ...afterProgress, done: e.payload.done, total: e.payload.total };
      else if (movies.progress) movies.progress = { ...movies.progress, ...e.payload };
    });
    const unlistenComic = listen<{ stage: string; series?: string; n?: string }>("comic-progress", (e) => comics.onProgress(e.payload));
    const unlistenTv = listen<{ stage: "finding" | "loading"; show?: string }>("tv-progress", (e) => tv.onProgress(e.payload));
    const unlistenAsk = listen<{ id: number; question: { kind: string; source: "tmdb" | "anilist"; query: string; candidates: Candidate[] } }>(
      "flow-ask",
      (e) => {
        const q = e.payload.question;
        if (q.kind === "show") tv.ask(e.payload.id, q);
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        else if (q.kind === "comicSeries") comics.ask(e.payload.id, q as any);
      },
    );
    const unlistenMb = listen<{ done: number; total: number; kind?: "listen" | "tags" }>("mb-progress", (e) => {
      // After listening, the albums heard are looked up.
      if (afterProgress) afterProgress = { ...afterProgress, done: e.payload.done, total: e.payload.total };
      else if (music.progress)
        music.progress = { ...e.payload, kind: e.payload.kind ?? (music.progress.kind === "listen" ? "search" : music.progress.kind) };
    });
    const unlistenSubs = listen<{ done: number; total: number }>("subs-progress", (e) => {
      if (afterProgress) afterProgress = { ...afterProgress, done: e.payload.done, total: e.payload.total };
      else if (subs.progress) subs.progress = { ...subs.progress, ...e.payload };
    });
    const unlistenBooks = listen<{ done: number; total: number }>("book-progress", (e) => {
      if (afterProgress) afterProgress = { ...afterProgress, done: e.payload.done, total: e.payload.total };
      else if (books.progress) books.progress = { ...books.progress, ...e.payload };
    });
    const unlistenInside = listen<{ done: number; total: number }>("inside-progress", (e) => {
      if (afterProgress) afterProgress = { ...afterProgress, done: e.payload.done, total: e.payload.total };
    });
    const unlistenAfter = listen<{ stage: string | null; total?: number }>("after-stage", (e) => {
      afterProgress = e.payload.stage ? { stage: e.payload.stage, done: 0, total: e.payload.total ?? 0 } : null;
    });
    const unlistenSubTools = listen<{ done: number; total: number; part: number }>("subtools-progress", (e) => {
      if (afterProgress) afterProgress = { ...afterProgress, ...e.payload };
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
      // The settings and the rules' table came before the page
      // (`+layout.ts`).
      tableLang = settings.lang;
      // The password store may wait to be unlocked: the program does not.
      loadSecrets();
      if (settings.rememberRules) steps = await loadSteps(settings.rules);
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
      // A moment after opening, so it does not slow the start.
      if (settings.checkUpdates)
        setTimeout(() => api.updateCheck().then((u) => (update = u)).catch(() => {}), 4000);
    })();
    return () => {
      unlisten.then((f) => f());
      unlistenProgress.then((f) => f());
      unlistenSimilar.then((f) => f());
      unlistenPhotos.then((f) => f());
      unlistenApply.then((f) => f());
      unlistenUndo.then((f) => f());
      unlistenTmdb.then((f) => f());
      unlistenTv.then((f) => f());
      unlistenComic.then((f) => f());
      unlistenAsk.then((f) => f());
      unlistenMb.then((f) => f());
      unlistenBooks.then((f) => f());
      unlistenSubs.then((f) => f());
      unlistenInside.then((f) => f());
      unlistenAfter.then((f) => f());
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
      <button class="primary" title="{MOD}+Enter" disabled={!applyCount || busy || !!working} onclick={applyAll}>
        {t(sendTo ? (sendTo.copy ? "copyAction" : "moveAction") : "rename")}<span class="count" class:none={!applyCount} aria-hidden={!applyCount}>{applyCount.toLocaleString(LOCALES[settings.lang])}</span>
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
                  {@const block = blocks[type] ?? null}
                  {@const locked = !!block}
                  {@const sub = block && "twice" in block ? t("ruleOnce") : block && "clash" in block ? t("ruleClash", { rule: t(`rule.${block.clash}`) }) : TABLE.media.includes(type) ? t(`ruleSub.${type}` as Key) : ""}
                  <button role="menuitem" disabled={locked} onclick={() => addRule(type)}>
                    <span class="item-text">{t(`rule.${type}`)}{#if sub}<small>{sub}</small>{/if}</span>
                  </button>
                {/each}
              {/each}
            </div>
          {/if}
          <RuleSets
            current={() => (steps.length ? payload() : null)}
            onload={async (set) => (steps = await loadSteps(set.steps, t("ruleSetLoaded", { name: set.name }), true))}
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
              ? t(step.rule.identifyBySound ? "musicUntaggedListenN" : "musicUntaggedN", { n: music.untagged })
              : step.rule.type === "photo" && step.enabled && undatedCount
                ? t("photoUndatedN", { n: undatedCount })
                : step.rule.type === "list" && step.rule.names.trim()
                  ? step.only?.trim()
                    ? t("listOrderN", { n: preview.hints.steps[i]?.names ?? 0 })
                    : t("listCountN", { names: preview.hints.steps[i]?.names ?? 0, files: step.rule.pairs ? preview.hints.pairs : preview.hints.files })
                  : null}
            currentNames={step.rule.type === "list" && !step.only?.trim() ? (step.rule.pairs ? preview.hints.pairNames : preview.hints.names) : []}
            hint={preview.hints.steps[i] ?? null}
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
    {#if preview.conflicts - sameThere}<span class="bad">{t("conflicts", { n: preview.conflicts - sameThere })}</span>{/if}
    {#if sameThere}<span title={t("problem.sameThere")}>{t("sameThereN", { n: sameThere })}</span>{/if}
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
    {#if update}
      <button class="link update" title={t(update.way === "page" ? "updatePageHint" : "updateHint")} disabled={updating !== null} onclick={installUpdate}>
        {updating === null ? t("updateAvailable", { v: update.version }) : updating < 0 ? t("updateStarting") : t("updateDownloading", { p: updating })}
      </button>
    {/if}
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
    rows={comics.edit.rows}
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
    rows={books.edit.rows}
    onsave={(c) => books.saveEdit(c)}
    onclose={() => (books.edit = null)}
  />
{/if}
{#if music.edit}
  <DetailsEdit
    heading={t("songEditTitleN", { n: music.edit.songs.length })}
    hint={t("detailsEditHint")}
    fields={music.fields}
    rows={music.edit.rows}
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
  .menu-note { margin: 2px 12px 6px; padding-bottom: 8px; border-bottom: 1px solid var(--border); font-size: var(--fs-sm); color: var(--muted); max-width: 260px; }
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
  footer .link.update { background: var(--accent-soft); font-weight: 600; }
  .copy-icon { width: 12px; height: 12px; }
  .modal ul { list-style: none; margin: 0; padding: 0; overflow-y: auto; }
  .modal li { display: flex; align-items: center; gap: 12px; padding: 6px 0; border-bottom: 1px solid var(--row-line); }
  .modal li > :first-child { flex: 1; }
  /* The date on one line; a long summary beside it wraps instead. */
  .modal li > .when { white-space: nowrap; }
  .muted { color: var(--muted); }
  .failed { color: var(--danger); font-size: var(--fs-md); }
</style>
