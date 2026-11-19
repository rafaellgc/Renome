import { invoke } from "@tauri-apps/api/core";
import type { Ghost } from "./missing";
import type { Blocked } from "./rules";
import type { RuleType, AfterCounts, AfterReport, Rule, AlbumLookup, BookTarget, ComicFormat, AlbumQuery, ApplyReport, BookCandidate, BookGuess, BookLookup, BookMatch, Candidate, Details, ComicIssue, ComicMatch, ComicSource, ComicVolume, MbCandidate, SongMatch, HistoryEntry, ListedTrack, EpisodeMatch, EpKey, BookFound, BookPart, MovieFound, PreviewItem, SeriesOutcome, AudioBook, ServerCall, ServerLibrary, UpdateFound, MovieSet, Preview, ScanOptions, Show, SortKey, Step, SubCandidate, SubsAuth, View } from "./types";

type Overrides = Record<string, string>;

/** Covers this many bits apart, or fewer, are the same cover (another
 *  scan or edition of the same art); see `cover_distances`. */
export const SAME_COVER = 12;

/** A comic's or book's own cover: a copy to show, and its fingerprint. */
export interface OwnCover {
  image: string;
  hash: string;
}

/** A file as the comparison window shows it. */
export interface PictureCard {
  path: string;
  size: number;
  /** Unix seconds. */
  modified: number | null;
  /** When the photo was taken, `YYYY-MM-DDTHH:MM:SS`. */
  taken: string | null;
  width: number | null;
  height: number | null;
  /** A smaller copy (`data:` URL), when it is a picture. */
  image: string | null;
}
type TvItem = { path: string; name: string; isDir: boolean };
type SubsItem = { path: string; tmdb: number; season?: number | null; episode?: number | null };

/** The same movie, episode, comic or book in several files. */
export interface WorkGroup {
  kind: "movie" | "episode" | "comic" | "book";
  items: { path: string; size: number; picture?: string; codec?: string }[];
  /** The one worth keeping, when one stands out. */
  best?: string;
}

export const api = {
  /** Each rule new at its defaults in this language, and the rules that name a whole kind of file (the Rust catalog's). */
  ruleTable: (lang: string) => invoke<{ defaults: Record<string, Rule>; kinds: RuleType[] }>("rule_table", { lang }),
  /** Why each rule cannot join the rules there, when it cannot. */
  ruleBlocks: (types: string[]) => invoke<Record<string, Blocked | null>>("rule_blocks", { types }),
  /** Steps with only their first kind rule, and the types left out. */
  oneLookup: (steps: Step[]) => invoke<[Step[], RuleType[]]>("one_lookup", { steps }),
  /** The details typed in a details window over each file's (the command line's --edit does the same). */
  editDetails: <T>(kind: "song" | "book" | "comic", items: T[], changed: Record<string, string>) => invoke<T[]>("edit_details", { kind, items, changed }),
  /** Photo dates typed over the ones they had; null when the fields make no date. */
  editDates: (old: string[], changed: Record<string, string>) => invoke<string[] | null>("edit_dates", { old, changed }),
  /** What books say about themselves, as a start for typing. */
  booksFromGuesses: (guesses: BookGuess[]) => invoke<BookMatch[]>("books_from_guesses", { guesses }),
  /** What the lookups say is missing from the list as shown (the shared flow decides). */
  missingItems: (rules: Rule[], known: unknown, descending: boolean) => invoke<Omit<Ghost, "ghost">[]>("missing_items", { rules, known, descending }).then((l) => l.map((g): Ghost => ({ ...g, ghost: true }))),
  /** What renaming would write besides the renames (the shared flow decides). */
  afterCounts: (rules: Rule[], chosen: unknown) => invoke<AfterCounts>("after_counts", { rules, chosen }),
  /** Everything written once the list is renamed: `done`, each renamed file's old and new path. */
  afterRename: (rules: Rule[], chosen: unknown, done: [string, string][], historyId: string | null, tmdbKey: string, tmdbLang: string, subs: SubsAuth) =>
    invoke<AfterReport>("after_rename", { rules, chosen, done, historyId, tmdbKey, tmdbLang, subs }),
  pdfPrintAvailable: () => invoke<boolean>("pdf_print_available"),
  checkpoint: (overrides: Overrides) => invoke<void>("checkpoint", { overrides }),
  dropCheckpoint: () => invoke<void>("drop_checkpoint"),
  undoList: () => invoke<Overrides | null>("undo_list"),
  addPaths: (paths: string[], options: ScanOptions) =>
    invoke<{ added: string[]; skipped: { path: string; reason: string }[] }>("add_paths", { paths, options }),
  removeFiles: (paths: string[]) => invoke<void>("remove_files", { paths }),
  clearFiles: () => invoke<void>("clear_files"),
  moveFiles: (paths: string[], delta: -1 | 1) => invoke<void>("move_files", { paths, delta }),
  sortFiles: (key: SortKey, descending: boolean) => invoke<void>("sort_files", { key, descending }),
  fileDetails: (paths: string[]) => invoke<Record<string, Details>>("file_details", { paths }),
  /** Stops the searches under way; they keep what they found. */
  stopSearch: () => invoke<void>("stop_search"),
  /** Identical files, and songs that are the same music; null when a
   *  newer search replaced this one. */
  findIdentical: () => invoke<{ identical: string[][]; songs: string[][]; works: WorkGroup[] } | null>("find_identical"),
  findSimilar: () => invoke<string[][] | null>("find_similar"),
  ownCover: (path: string) => invoke<OwnCover | null>("own_cover", { path }),
  coverDistances: (hash: string, urls: string[]) => invoke<(number | null)[]>("cover_distances", { hash, urls }),
  /** A catalogue's picture the page could not show by its address. */
  remoteImage: (url: string) => invoke<string | null>("remote_image", { url }),
  /** The covers of the comic's issue (or volume) in a series; none from
   *  catalogues that give no covers to programs. */
  seriesCovers: (source: ComicSource, key: string, id: string, comic: ComicMatch) => invoke<string[]>("series_covers", { source, key, id, comic }),
  /** Which pictures of a group of similar ones stand out (the shared choice). */
  picturePicks: (cards: { path: string; width: number | null; height: number | null; size: number; taken: string | null; modified: number | null }[]) =>
    invoke<{ best: string | null; largest: string | null; oldest: string | null }>("picture_picks", { cards }),
  pictureCards: (paths: string[], max: number) => invoke<PictureCard[]>("picture_cards", { paths, max }),
  trashFiles: (paths: string[]) => invoke<{ trashed: string[]; failed: [string, string][] }>("trash_files", { paths }),
  preview: (steps: Step[], overrides: Overrides, view: View) =>
    invoke<Preview>("preview", { steps, overrides, view }),
  apply: (steps: Step[], overrides: Overrides, view: View) =>
    invoke<ApplyReport>("apply", { steps, overrides, view }),
  history: () => invoke<HistoryEntry[]>("history"),
  tmdbCheck: (key: string) => invoke<void>("tmdb_check", { key }),
  /** The movie rule's search (shared with the command line): sure picks
   *  and doubts. An error when nothing could be searched. */
  movieSearch: (key: string, lang: string, items: PreviewItem[]) =>
    invoke<MovieFound[]>("movie_search", { key, lang, items }),
  /** Each movie's template fields and collection (null: none), and,
   *  with `sets`, the collections. */
  tmdbCollections: (key: string, lang: string, ids: number[], sets: boolean) =>
    invoke<{ of: [number, number | null][]; sets: MovieSet[]; extras: [number, Record<string, string>][] }>("tmdb_collections", { key, lang, ids, sets }),
  tmdbSearch: (key: string, lang: string, title: string, year: number | null) =>
    invoke<Candidate[]>("tmdb_search", { key, lang, title, year }),
  tvSearch: (key: string, lang: string, query: string, items: TvItem[]) =>
    invoke<{ query: string; candidates: Candidate[]; auto: boolean }>("tv_search", { key, lang, query, items }),
  animeSearch: (query: string, items: TvItem[]) =>
    invoke<{ query: string; candidates: Candidate[]; auto: boolean }>("anime_search", { query, items }),
  /** The TV show rule's search (shared with the command line); its
   *  questions come as `flow-ask` events. */
  seriesSearch: (
    key: string,
    lang: string,
    source: string,
    typed: string,
    items: PreviewItem[],
    settled: [string, number][],
    picked: Record<string, Candidate>,
  ) => invoke<SeriesOutcome>("series_search", { key, lang, source, typed, items, settled, picked }),
  /** Each file's episodes as the rule names them (`show`: its show's
   *  place in `shows`). */
  seriesNamed: (files: { path: string; show: number; choice: EpKey[] }[], shows: Show[]) =>
    invoke<Record<string, EpisodeMatch>>("series_named", { files, shows }),
  /** The answer to a flow's question (null: none fits). */
  flowAnswer: (id: number, answer: unknown) => invoke<void>("flow_answer", { id, answer }),
  /** The music rule's search (shared with the command line): albums,
   *  sure ones with their release. An error when every album failed. */
  musicSearch: (items: PreviewItem[], settled: string[], fromName: boolean, listen: boolean) =>
    invoke<AlbumLookup[]>("music_search", { items, settled, fromName, listen }),
  mbSearch: (query: AlbumQuery, discogs = false) => invoke<MbCandidate[]>("mb_search", { query, discogs }),
  /** The Discogs token from the options (empty: Discogs is not used). */
  discogsToken: (token: string) => invoke<void>("discogs_token", { token }),
  /** The videos whose subtitles are searched (shared with the command line). */
  subsTargets: (files: { path: string; original: string; isDir: boolean; movie: number | null; episode: EpisodeMatch | null }[]) =>
    invoke<SubsItem[]>("subs_targets", { files }),
  subsSearch: (auth: SubsAuth, langs: string, items: SubsItem[]) =>
    invoke<{
      langs: string[];
      found: { path: string; have: string[]; inside: string[]; candidates: SubCandidate[]; pick: Record<string, number> }[];
      errors: string[];
      remaining: number | null;
    }>("subs_search", { auth, langs, items }),
  osCheck: (auth: SubsAuth) => invoke<number | null>("os_check", { auth }),
  subdlCheck: (key: string) => invoke<void>("subdl_check", { key }),
  updateCheck: () => invoke<UpdateFound | null>("update_check"),
  updateInstall: () => invoke<void>("update_install"),
  updateWay: () => invoke<"itself" | "page" | "system">("update_way"),
  serverCheck: (server: ServerCall) => invoke<void>("server_check", { server }),
  serverLibraries: (server: ServerCall) => invoke<ServerLibrary[]>("server_libraries", { server }),
  /** For each server, how many folders it was told about, or why not. */
  /** Tells the servers in the options about the folders the files left and went to (which servers: the shared choice). */
  serverTell: (servers: unknown, keys: string, done: { from: string; to: string }[]) =>
    invoke<{ name: string; folders: { Ok: number } | { Err: string } }[]>("server_tell", { servers, keys, done }),
  subsourceCheck: (key: string) => invoke<void>("subsource_check", { key }),
  discogsCheck: (token: string) => invoke<void>("discogs_check", { token }),
  googleBooksKey: (key: string) => invoke<void>("google_books_key", { key }),
  googleBooksCheck: (key: string) => invoke<void>("google_books_check", { key }),
  mbChoose: (id: string, paths: string[], fromName: boolean) =>
    invoke<{ matches: [string, SongMatch][]; tracks: ListedTrack[]; agree: number }>("mb_choose", { id, paths, fromName }),
  /** `names`: the name each file is read by (null: its own). */
  /** Saves rows as a CSV file, as the command line writes them. */
  saveCsv: (path: string, rows: string[][], lang: string) => invoke<void>("save_csv", { path, rows, lang }),
  /** Where TMDB images come from (`…/t/p`, sizes added). */
  tmdbImages: () => invoke<string>("tmdb_images"),
  comicGuesses: (paths: string[], names: (string | null)[], byFolder: boolean) =>
    invoke<(ComicMatch | null)[]>("comic_guesses", { paths, names, byFolder }),
  songTags: (paths: string[], fromName: boolean) => invoke<(SongMatch | null)[]>("song_own_tags", { paths, fromName }),
  comicCheck: (key: string) => invoke<void>("comic_check", { key }),
  /** Series found, those that could hold issue `highest` first. */
  comicSearch: (source: ComicSource, key: string, query: string, lang: string, year: number | null = null, publisher = "", skip = "", highest = 0) =>
    invoke<ComicVolume[]>("comic_search", { source, key, query, year, publisher, lang, skip, highest }),
  /** The barcodes printed in the comics' first and last pages. */
  /** The issue a barcode belongs to, on Metron. */
  /** The comic rule's search (shared with the command line); a series
   *  it is not sure of comes as a `flow-ask` question. */
  comicsSearch: (
    params: { source: string; key: string; series: string; skip: string; lang: string; uiLang: string; byFolder: boolean; barcode: boolean },
    items: PreviewItem[],
    picked: Record<string, ComicVolume>,
  ) =>
    invoke<{
      empty: boolean;
      volumes: Record<string, { volume: ComicVolume; issues: ComicIssue[] }>;
      files: { path: string; volumeKey: string; own: ComicMatch; choice: string; sure: boolean }[];
      picked: Record<string, ComicVolume>;
    }>("comics_search", { params, items, picked }),
  /** Each comic as the rule names it, with its page in the catalogue. */
  comicNamed: (
    files: { path: string; volumeKey: string; own: ComicMatch; choice: string | null }[],
    volumes: Record<string, { volume: ComicVolume; issues: ComicIssue[] }>,
  ) => invoke<Record<string, { comic: ComicMatch; web: string; manga: boolean }>>("comic_named", { files, volumes }),
  metronCheck: (token: string) => invoke<void>("metron_check", { token }),
  gcdAccount: (email: string, password: string) => invoke<void>("gcd_account", { email, password }),
  gcdCheck: (email: string, password: string) => invoke<void>("gcd_check", { email, password }),
  /** The book rule's search (shared with the command line). An error
   *  when nothing could be searched. */
  booksSearch: (lang: string, items: PreviewItem[], settled: string[], audiobooks: boolean) =>
    invoke<{ empty: boolean; parts: Record<string, BookPart>; found: BookFound[] }>("books_search", { lang, items, settled, audiobooks }),
  /** Each file's book as the rule names it, from the books chosen. */
  bookMatches: (chosen: { path: string; guess: BookGuess; choice: BookCandidate }[], parts: Record<string, BookPart>, audiobooks: boolean) =>
    invoke<Record<string, BookMatch>>("book_matches", { chosen, parts, audiobooks }),
  bookGuesses: (paths: string[], names: string[]) => invoke<BookGuess[]>("book_guesses", { paths, names }),
  bookSearch: (lang: string, title: string, author: string) =>
    invoke<BookCandidate[]>("book_search", { lang, title, author }),
  undo: (id: string) => invoke<ApplyReport>("undo", { id }),
  /** Puts back renames a stopped program left halfway. */
  recover: () => invoke<{ restored: number; renamed: number; undone: number; tagged: number; stuck: { from: string; to: string }[] }>("recover"),
  deleteHistory: (id: string) => invoke<void>("delete_history", { id }),
  clearHistory: () => invoke<number>("clear_history"),
  /** Bytes the kept answers of the catalogues take. */
  cacheSize: () => invoke<number>("cache_size"),
  /** Forgets the kept answers; the bytes freed. */
  clearCache: () => invoke<number>("clear_cache"),
  savePreset: (path: string, steps: Step[]) => invoke<void>("save_preset", { path, steps }),
  loadPreset: (path: string) => invoke<Step[]>("load_preset", { path }),
};
