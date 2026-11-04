import { api } from "./api";

/** Where TMDB images are shown from; tests may point it elsewhere. */
let base = $state("https://image.tmdb.org/t/p");
api.tmdbImages().then((b) => (base = b)).catch(() => {});

/** A TMDB image at a size (`w185`, `w300`), or null without a path; a
 *  whole address (AniList's) as it is. */
export function tmdbImage(size: string, path: string | null): string | null {
  if (path?.startsWith("https://")) return path;
  return path ? `${base}/${size}${path}` : null;
}
