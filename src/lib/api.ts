import { invoke } from "@tauri-apps/api/core";
import type { AlbumLookup, BookTarget, ComicFormat, AlbumQuery, ApplyReport, BookCandidate, BookGuess, BookLookup, BookMatch, Candidate, Details, ComicIssue, ComicMatch, ComicSource, ComicVolume, EpisodeLookup, MbCandidate, SongMatch, HistoryEntry, ListedTrack, Lookup, MovieSet, Preview, ScanOptions, Show, SortKey, Step, SubCandidate, SubsAuth, View } from "./types";

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

/** The same movie, episode, comic or book in several files. */
export interface WorkGroup {
  kind: "movie" | "episode" | "comic" | "book";
  items: { path: string; size: number; picture?: string; codec?: string }[];
  /** The one worth keeping, when one stands out. */
  best?: string;
}

export const api = {
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
  pictureCards: (paths: string[], max: number) => invoke<PictureCard[]>("picture_cards", { paths, max }),
  trashFiles: (paths: string[]) => invoke<{ trashed: string[]; failed: [string, string][] }>("trash_files", { paths }),
  preview: (steps: Step[], overrides: Overrides, view: View) =>
    invoke<Preview>("preview", { steps, overrides, view }),
  apply: (steps: Step[], overrides: Overrides, view: View) =>
    invoke<ApplyReport>("apply", { steps, overrides, view }),
  history: () => invoke<HistoryEntry[]>("history"),
  tmdbCheck: (key: string) => invoke<void>("tmdb_check", { key }),
  tmdbLookup: (key: string, lang: string, items: { path: string; name: string }[]) =>
    invoke<Lookup[]>("tmdb_lookup", { key, lang, items }),
  /** Each movie's template fields and collection (null: none), and,
   *  with `sets`, the collections. */
  tmdbCollections: (key: string, lang: string, ids: number[], sets: boolean) =>
    invoke<{ of: [number, number | null][]; sets: MovieSet[]; extras: [number, Record<string, string>][] }>("tmdb_collections", { key, lang, ids, sets }),
  tmdbSearch: (key: string, lang: string, title: string, year: number | null) =>
    invoke<Candidate[]>("tmdb_search", { key, lang, title, year }),
  tvSearch: (key: string, lang: string, query: string, items: TvItem[]) =>
    invoke<{ query: string; candidates: Candidate[]; auto: boolean }>("tv_search", { key, lang, query, items }),
  tvLookup: (key: string, lang: string, id: number, items: TvItem[], typed: string) =>
    invoke<{ show: Show; files: EpisodeLookup[] }>("tv_lookup", { key, lang, id, items, typed }),
  animeSearch: (query: string, items: TvItem[]) =>
    invoke<{ query: string; candidates: Candidate[]; auto: boolean }>("anime_search", { query, items }),
  animeLookup: (id: number, items: TvItem[], typed: string) =>
    invoke<{ show: Show; files: EpisodeLookup[] }>("anime_lookup", { id, items, typed }),
  tvGroups: (items: TvItem[]) => invoke<{ groups: string[][]; loose: string[] }>("tv_groups", { items }),
  mbLookup: (paths: string[], fromName: boolean, listen: boolean) =>
    invoke<AlbumLookup[]>("mb_lookup", { paths, fromName, listen }),
  mbSearch: (query: AlbumQuery, discogs = false) => invoke<MbCandidate[]>("mb_search", { query, discogs }),
  /** The Discogs token from the options (empty: Discogs is not used). */
  discogsToken: (token: string) => invoke<void>("discogs_token", { token }),
  subsSearch: (auth: SubsAuth, langs: string, items: { path: string; tmdb: number; season?: number; episode?: number }[]) =>
    invoke<{ langs: string[]; found: { path: string; have: string[]; candidates: SubCandidate[] }[]; errors: string[]; remaining: number | null }>("subs_search", { auth, langs, items }),
  writeSubtitles: (auth: SubsAuth, historyId: string | null, items: { path: string; season?: number; episode?: number; langs: SubCandidate[][] }[]) =>
    invoke<{ created: string[]; errors: [string, string][]; remaining: number | null }>("write_subtitles", { auth, historyId, items }),
  osCheck: (auth: SubsAuth) => invoke<number | null>("os_check", { auth }),
  subdlCheck: (key: string) => invoke<void>("subdl_check", { key }),
  subsourceCheck: (key: string) => invoke<void>("subsource_check", { key }),
  discogsCheck: (token: string) => invoke<void>("discogs_check", { token }),
  googleBooksKey: (key: string) => invoke<void>("google_books_key", { key }),
  googleBooksCheck: (key: string) => invoke<void>("google_books_check", { key }),
  mbChoose: (id: string, paths: string[], fromName: boolean) =>
    invoke<{ matches: [string, SongMatch][]; tracks: ListedTrack[]; agree: number }>("mb_choose", { id, paths, fromName }),
  /** `names`: the name each file is read by (null: its own). */
  saveText: (path: string, text: string) => invoke<void>("save_text", { path, text }),
  /** Where TMDB images come from (`…/t/p`, sizes added). */
  tmdbImages: () => invoke<string>("tmdb_images"),
  comicGuesses: (paths: string[], names: (string | null)[], byFolder: boolean) =>
    invoke<(ComicMatch | null)[]>("comic_guesses", { paths, names, byFolder }),
  songTags: (paths: string[], fromName: boolean) => invoke<(SongMatch | null)[]>("song_own_tags", { paths, fromName }),
  comicCheck: (key: string) => invoke<void>("comic_check", { key }),
  comicSearch: (source: ComicSource, key: string, query: string, lang: string, year: number | null = null, publisher = "", skip = "") =>
    invoke<ComicVolume[]>("comic_search", { source, key, query, year, publisher, lang, skip }),
  comicIssues: (source: ComicSource, key: string, id: string, comics: ComicMatch[]) =>
    invoke<{ volume: ComicVolume; issues: ComicIssue[]; found: (string | null)[] }>("comic_issues", { source, key, id, comics }),
  comicVolumeOf: (key: string, issue: string) => invoke<string>("comic_volume_of", { key, issue }),
  /** The barcodes printed in the comics' first and last pages. */
  comicBarcodes: (paths: string[]) => invoke<(string | null)[]>("comic_barcodes", { paths }),
  /** The issue a barcode belongs to, on Metron. */
  comicByCode: (key: string, code: string) =>
    invoke<{ volume: ComicVolume; issues: ComicIssue[]; issue: string } | null>("comic_by_code", { key, code }),
  metronCheck: (token: string) => invoke<void>("metron_check", { token }),
  gcdAccount: (email: string, password: string) => invoke<void>("gcd_account", { email, password }),
  gcdCheck: (email: string, password: string) => invoke<void>("gcd_check", { email, password }),
  bookLookup: (lang: string, items: { path: string; name: string }[]) =>
    invoke<BookLookup[]>("book_lookup", { lang, items }),
  bookGuesses: (paths: string[], names: string[]) => invoke<BookGuess[]>("book_guesses", { paths, names }),
  bookSearch: (lang: string, title: string, author: string) =>
    invoke<BookCandidate[]>("book_search", { lang, title, author }),
  writeBookSidecars: (
    historyId: string | null,
    items: { path: string; book: BookMatch; cover: number | null; language: string }[],
    cover: boolean,
    opf: boolean,
    inFolder: boolean,
  ) => invoke<{ created: string[]; errors: [string, string][] }>("write_book_sidecars", { historyId, items, cover, opf, inFolder }),
  playlistDirs: (dirs: string[]) => invoke<string[]>("playlist_dirs", { dirs }),
  coverDirs: (dirs: string[]) => invoke<string[]>("cover_dirs", { dirs }),
  writeTags: (historyId: string | null, items: { path: string; song: SongMatch }[], cover: boolean, clean: boolean, coverSize: number) =>
    invoke<{ written: number; errors: [string, string][] }>("write_tags", { historyId, items, cover, clean, coverSize }),
  writeInside: (
    historyId: string | null,
    items: (
      | {
          path: string;
          comic?: ComicMatch;
          web?: string;
          manga?: boolean;
          convert?: "keep" | "delete";
          pdf?: { draw: boolean; size: number; info: boolean };
          comicTo?: ComicFormat;
          ebook?: { device: string; crop: boolean; split: boolean };
        }
      | { path: string; book?: BookMatch; language?: string; polish?: { epub3: boolean; shrink: boolean } }
    )[],
  ) => invoke<{ written: number; drawn: number; errors: [string, string][] }>("write_inside", { historyId, items }),
  writeVideoTags: (
    key: string,
    lang: string,
    historyId: string | null,
    items: { path: string; movie?: number; show?: number; season?: number; episodes?: number[] }[],
    options: { cover: boolean; clean: boolean; tracks: boolean },
  ) => invoke<{ written: number; drawn: number; errors: [string, string][] }>("write_video_tags", { key, lang, historyId, items, options }),
  subtitleTools: (
    historyId: string | null,
    items: { path: string; fix: boolean; extract: boolean }[],
    tools: { toSrt: boolean; utf8: boolean; ads: boolean; hearing: boolean; shiftMs: number; fpsFrom: number; fpsTo: number },
    original: "keep" | "delete",
    extractLangs: string,
  ) =>
    invoke<{ written: number; created: number; errors: [string, string][] }>("subtitle_tools", { historyId, items, tools, original, extractLangs }),
  writePlaylists: (historyId: string | null, dirs: string[]) =>
    invoke<{ created: string[]; errors: [string, string][]; kept: number }>("write_playlists", { historyId, dirs }),
  writeCovers: (historyId: string | null, items: { dir: string; release: string }[]) =>
    invoke<{ created: string[]; errors: [string, string][] }>("write_covers", { historyId, items }),
  writeSidecars: (
    key: string,
    lang: string,
    historyId: string | null,
    items: { path: string; id: number }[],
    fanart: boolean,
    inFolder: boolean,
  ) => invoke<{ created: string[]; errors: [string, string][] }>("write_sidecars", { key, lang, historyId, items, fanart, inFolder }),
  writeTvSidecars: (
    key: string,
    lang: string,
    historyId: string | null,
    items: { path: string; id: number; season: number; episodes: number[]; titles: string[] }[],
    fanart: boolean,
    folders: boolean,
  ) => invoke<{ created: string[]; errors: [string, string][] }>("write_tv_sidecars", { key, lang, historyId, items, fanart, folders }),
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
