import { invoke } from "@tauri-apps/api/core";
import type { ApplyReport, HistoryEntry, Preview, ScanOptions, SortKey, Step, View } from "./types";

type Overrides = Record<string, string>;

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
  preview: (steps: Step[], overrides: Overrides, view: View) =>
    invoke<Preview>("preview", { steps, overrides, view }),
  apply: (steps: Step[], overrides: Overrides, view: View) =>
    invoke<ApplyReport>("apply", { steps, overrides, view }),
  history: () => invoke<HistoryEntry[]>("history"),
  undo: (id: string) => invoke<ApplyReport>("undo", { id }),
  deleteHistory: (id: string) => invoke<void>("delete_history", { id }),
  savePreset: (path: string, steps: Step[]) => invoke<void>("save_preset", { path, steps }),
  loadPreset: (path: string) => invoke<Step[]>("load_preset", { path }),
};
