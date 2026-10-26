import { invoke } from "@tauri-apps/api/core";
import type { AlbumLookup, AlbumQuery, ApplyReport, BookCandidate, BookLookup, BookMatch, Candidate, ComicIssue, ComicMatch, ComicSource, ComicVolume, EpisodeLookup, MbCandidate, SongMatch, HistoryEntry, Lookup, Preview, ScanOptions, Show, SortKey, Step, View } from "./types";

type Overrides = Record<string, string>;
type TvItem = { path: string; name: string; isDir: boolean };

export const api = {
  checkpoint: (overrides: Overrides) => invoke<void>("checkpoint", { overrides }),
  dropCheckpoint: () => invoke<void>("drop_checkpoint"),
  undoList: () => invoke<Overrides | null>("undo_list"),
  addPaths: (paths: string[], options: ScanOptions) =>
    invoke<{ added: string[]; skipped: { path: string; reason: string }[] }>("add_paths", { paths, options }),
  removeFiles: (paths: string[]) => invoke<void>("remove_files", { paths }),
  clearFiles: () => invoke<void>("clear_files"),
  moveFiles: (paths: string[], delta: -1 | 1) => invoke<void>("move_files", { paths, delta }),
  sortFiles: (key: SortKey, descending: boolean) => invoke<void>("sort_files", { key, descending }),
  findIdentical: () => invoke<string[][] | null>("find_identical"),
  preview: (steps: Step[], overrides: Overrides, view: View) =>
    invoke<Preview>("preview", { steps, overrides, view }),
  apply: (steps: Step[], overrides: Overrides, view: View) =>
    invoke<ApplyReport>("apply", { steps, overrides, view }),
  history: () => invoke<HistoryEntry[]>("history"),
  tmdbCheck: (key: string) => invoke<void>("tmdb_check", { key }),
  tmdbLookup: (key: string, lang: string, items: { path: string; name: string }[]) =>
    invoke<Lookup[]>("tmdb_lookup", { key, lang, items }),
  tmdbSearch: (key: string, lang: string, title: string, year: number | null) =>
    invoke<Candidate[]>("tmdb_search", { key, lang, title, year }),
  tvSearch: (key: string, lang: string, query: string, items: TvItem[]) =>
    invoke<{ query: string; candidates: Candidate[]; auto: boolean }>("tv_search", { key, lang, query, items }),
  tvLookup: (key: string, lang: string, id: number, items: TvItem[]) =>
    invoke<{ show: Show; files: EpisodeLookup[] }>("tv_lookup", { key, lang, id, items }),
  mbLookup: (paths: string[], fromName: boolean, listen: boolean) =>
    invoke<AlbumLookup[]>("mb_lookup", { paths, fromName, listen }),
  mbSearch: (query: AlbumQuery) => invoke<MbCandidate[]>("mb_search", { query }),
  mbChoose: (id: string, paths: string[], fromName: boolean) =>
    invoke<{ matches: [string, SongMatch][]; agree: number }>("mb_choose", { id, paths, fromName }),
  /** `names`: the name each file is read by (null: its own). */
  saveText: (path: string, text: string) => invoke<void>("save_text", { path, text }),
  comicGuesses: (paths: string[], names: (string | null)[], byFolder: boolean) =>
    invoke<(ComicMatch | null)[]>("comic_guesses", { paths, names, byFolder }),
  comicCheck: (key: string) => invoke<void>("comic_check", { key }),
  comicSearch: (source: ComicSource, key: string, query: string, lang: string) =>
    invoke<ComicVolume[]>("comic_search", { source, key, query, lang }),
  comicIssues: (source: ComicSource, key: string, id: string, comics: ComicMatch[]) =>
    invoke<{ volume: ComicVolume; issues: ComicIssue[]; found: (string | null)[] }>("comic_issues", { source, key, id, comics }),
  comicVolumeOf: (key: string, issue: string) => invoke<string>("comic_volume_of", { key, issue }),
  gcdSignIn: (email: string, password: string) => invoke<void>("gcd_sign_in", { email, password }),
  gcdSignOut: () => invoke<void>("gcd_sign_out"),
  bookLookup: (lang: string, items: { path: string; name: string }[]) =>
    invoke<BookLookup[]>("book_lookup", { lang, items }),
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
  writeTags: (historyId: string | null, items: { path: string; song: SongMatch }[], cover: boolean, clean: boolean) =>
    invoke<{ written: number; errors: [string, string][] }>("write_tags", { historyId, items, cover, clean }),
  writePlaylists: (historyId: string | null, dirs: string[]) =>
    invoke<{ created: string[]; errors: [string, string][] }>("write_playlists", { historyId, dirs }),
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
  recover: () => invoke<{ restored: number; renamed: number; undone: number; stuck: { from: string; to: string }[] }>("recover"),
  deleteHistory: (id: string) => invoke<void>("delete_history", { id }),
  clearHistory: () => invoke<number>("clear_history"),
  savePreset: (path: string, steps: Step[]) => invoke<void>("save_preset", { path, steps }),
  loadPreset: (path: string) => invoke<Step[]>("load_preset", { path }),
};
