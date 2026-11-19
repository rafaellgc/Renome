import type { DetailColumn } from "./types";

/** The title of each optional column (its key in the texts, as the Rust
 *  CSV's `export::column_label` names it). */
export const COL_LABEL = Object.fromEntries(
  (["artist", "album", "title", "track", "year", "genre", "length", "quality", "resolution", "codec", "taken"] as const).map((c) => [c, `col${c[0].toUpperCase()}${c.slice(1)}`]),
) as Record<DetailColumn, `col${Capitalize<DetailColumn>}`>;
