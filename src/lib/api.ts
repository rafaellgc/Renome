import { invoke } from "@tauri-apps/api/core";
import type { AlbumLookup, AlbumQuery, ApplyReport, Candidate, EpisodeLookup, MbCandidate, SongMatch, HistoryEntry, Lookup, Preview, ScanOptions, Show, SortKey, Step, View } from "./types";

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
  mbLookup: (paths: string[], fromName: boolean) => invoke<AlbumLookup[]>("mb_lookup", { paths, fromName }),
  mbSearch: (query: AlbumQuery) => invoke<MbCandidate[]>("mb_search", { query }),
  mbChoose: (id: string, paths: string[], fromName: boolean) =>
    invoke<{ matches: [string, SongMatch][]; agree: number }>("mb_choose", { id, paths, fromName }),
  playlistDirs: (dirs: string[]) => invoke<string[]>("playlist_dirs", { dirs }),
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
  deleteHistory: (id: string) => invoke<void>("delete_history", { id }),
  savePreset: (path: string, steps: Step[]) => invoke<void>("save_preset", { path, steps }),
  loadPreset: (path: string) => invoke<Step[]>("load_preset", { path }),
};
