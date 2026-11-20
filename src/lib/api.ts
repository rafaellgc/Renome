import { invoke } from "@tauri-apps/api/core";
import type { Ghost } from "./missing";
import type { Blocked, RuleTable } from "./rules";
import type { RuleType, AfterCounts, AfterReport, Rule, AlbumLookup, BookTarget, ComicFormat, AlbumQuery, ApplyReport, BookCandidate, BookGuess, BookLookup, BookMatch, Candidate, Details, ComicIssue, ComicMatch, ComicSource, ComicVolume, MbCandidate, SongMatch, HistoryEntry, ListedTrack, EpisodeMatch, EpKey, BookFound, BookPart, MovieFound, PreviewItem, SeriesOutcome, AudioBook, ServerCall, ServerLibrary, UpdateFound, MovieSet, Preview, ScanOptions, Show, SortKey, Step, SubCandidate, SubsAuth, View } from "./types";

type Overrides = Record<string, string>;

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
/** A video whose subtitles are searched, with what they were searched by
 *  (`key`: searched again when it changes). */
type SubsItem = { path: string; tmdb: number; season?: number | null; episode?: number | null; key: string };
/** A CSV cell from Rust: text, or a key of the texts. */
export type Cell = string | { key: string };

/** The same movie, episode, comic or book in several files. */
export interface WorkGroup {
  kind: "movie" | "episode" | "comic" | "book";
  items: { path: string; size: number; picture?: string; codec?: string }[];
  /** The one worth keeping, when one stands out. */
  best?: string;
}

export const api = {
  /** Everything rules, their menus and the options are built from (the Rust catalog's). */
  ruleTable: (lang: string) => invoke<RuleTable>("rule_table", { lang }),
  /** The list's optional columns that make sense with these rules. */
  columnsFor: (steps: Step[]) => invoke<import("./types").DetailColumn[]>("columns_for", { steps }),
  /** Why each rule cannot join the rules there, when it cannot. */
  ruleBlocks: (types: string[]) => invoke<Record<string, Blocked | null>>("rule_blocks", { types }),
  /** Saved steps made whole and up to date, and the types left out. */
  completeSteps: (steps: Step[], lang: string, saved: boolean) => invoke<[Step[], RuleType[]]>("complete_steps", { steps, lang, saved }),
  /** The files that take an answer given for `path` (its undecided group). */
  sameAnswer: (path: string, files: [string, string, string][]) => invoke<string[]>("same_answer", { path, files }),
  /** The details typed in a details window over each file's (the command line's --edit does the same). */
  editDetails: <T>(kind: "song" | "book" | "comic", items: T[], changed: Record<string, string>) => invoke<T[]>("edit_details", { kind, items, changed }),
  /** Photo dates typed over the ones they had; null when the fields make no date. */
  editDates: (old: string[], changed: Record<string, string>) => invoke<string[] | null>("edit_dates", { old, changed }),
  /** The rows a details window opens on for those chosen, what each starts
   *  from and how its fields show it. */
  editStart: <T>(args: {
    kind: "music" | "book" | "comic" | "photo";
    chosen: string[];
    known: Record<string, { current?: unknown; guess?: unknown }>;
    parts: Record<string, BookPart>;
    dates: Record<string, string>;
    fromName: boolean;
    byFolder: boolean;
    audiobooks: boolean;
  }) => invoke<[string[], T[], Record<string, string>[]]>("edit_start", args),
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
  /** The optional columns' values as the list shows them. */
  fileDetails: (paths: string[], lang: string) => invoke<Record<string, Record<string, string>>>("file_details", { paths, lang }),
  /** The list as shown, as CSV cells. */
  exportRows: (columns: string[], lang: string) => invoke<Cell[][]>("export_rows", { columns, lang }),
  /** The missing items as CSV: where, under what name, the rows. */
  exportMissing: () => invoke<{ dir: string | null; group: string | null; nameKey: string; rows: Cell[][] }>("export_missing"),
  /** Stops the searches under way; they keep what they found. */
  stopSearch: () => invoke<void>("stop_search"),
  /** Identical files, and songs that are the same music; null when a
   *  newer search replaced this one. */
  findIdentical: () => invoke<{ identical: string[][]; songs: string[][]; works: WorkGroup[] } | null>("find_identical"),
  findSimilar: () => invoke<string[][] | null>("find_similar"),
  ownCover: (path: string) => invoke<OwnCover | null>("own_cover", { path }),
  /** How far each cover is from the file's, for those that are the file's (null: another). */
  sameCovers: (hash: string, urls: string[]) => invoke<(number | null)[]>("cover_distances", { hash, urls }),
  /** The series found, compared with the file's cover (`cover-progress` as it goes). */
  seriesByCover: (source: ComicSource, key: string, hash: string, comic: ComicMatch, candidates: ComicVolume[]) =>
    invoke<{ candidates: ComicVolume[]; same: Record<string, number>; covers: Record<string, string>; compared: boolean; note: string | null }>("series_by_cover", { source, key, hash, comic, candidates }),
  /** A series' issues in the order shown, those `typed` names. */
  issueOrder: (issues: ComicIssue[], own: ComicMatch, far: Record<string, number | null>, typed: string) =>
    invoke<number[]>("issue_order", { issues, own, far, typed }),
  /** The covers a review compares, and whether at once. */
  issueCovers: (issues: ComicIssue[], volumeKey: string) => invoke<[string[], boolean, string | null]>("issue_covers", { issues, volumeKey }),
  /** The episodes what was typed names: a code exactly, and those whose name or code holds it. */
  episodesTyped: (show: Show, text: string) => invoke<{ exact: EpKey[] | null; matches: EpKey[] }>("episodes_typed", { show, text }),
  /** A larger copy of a catalogue's cover, where there is one. */
  biggerCover: (url: string) => invoke<string>("bigger_cover", { url }),
  /** A catalogue's picture the page could not show by its address. */
  remoteImage: (url: string) => invoke<string | null>("remote_image", { url }),
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
  /** Where a new album search looks first. */
  albumOnDiscogs: (firstFound: string | null, token: boolean) => invoke<boolean>("album_on_discogs", { firstFound, token }),
  /** The Discogs token from the options (empty: Discogs is not used). */
  discogsToken: (token: string) => invoke<void>("discogs_token", { token }),
  /** The videos whose subtitles are searched (shared with the command line). */
  subsTargets: (files: { path: string; original: string; isDir: boolean; movie: number | null; episode: EpisodeMatch | null; conflict: boolean }[], langs: string) =>
    invoke<SubsItem[]>("subs_targets", { files, langs }),
  /** The languages whose subtitles the rules on download (null: none). */
  subsWanted: (rules: Rule[]) => invoke<string | null>("subs_wanted", { rules }),
  /** Whether some subtitle service can be asked with these keys. */
  subsReady: (auth: SubsAuth) => invoke<boolean>("subs_ready", { auth }),
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
  /** Reads a server's libraries and pairs their folders: the server with them, libraries and folders. */
  serverRead: (server: unknown, key: string) => invoke<[import("./types").MediaServer, number, number]>("server_read", { server, key }),
  serverNew: () => invoke<import("./types").MediaServer>("server_new"),
  serverKeys: (raw: string) => invoke<Record<string, string>>("server_keys", { raw }),
  serverKeyWith: (raw: string, id: string, key: string) => invoke<string>("server_key_with", { raw, id, key }),
  /** The rule sets with rules saved by name; refused `exists:<place>`. */
  setsSave: (sets: unknown[], name: string, steps: Step[], replace: boolean) => invoke<unknown[]>("sets_save", { sets, name, steps, replace }),
  setsRename: (sets: unknown[], at: number, name: string) => invoke<unknown[]>("sets_rename", { sets, at, name }),
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
  comicCheck: (key: string) => invoke<void>("comic_check", { key }),
  /** Series found, those that could hold issue `highest` first. */
  comicSearch: (source: ComicSource, key: string, query: string, lang: string, year = "", publisher = "", skip = "", highest = 0) =>
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
      files: { path: string; volumeKey: string; own: ComicMatch; choice: string; sure: boolean; status: import("./types").MovieStatus; read: string }[];
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
  loadPreset: (path: string, lang: string) => invoke<[Step[], RuleType[]]>("load_preset", { path, lang }),
};
