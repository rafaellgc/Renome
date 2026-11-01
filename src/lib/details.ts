import { LOCALES } from "./i18n";
import { settings } from "./settings.svelte";
import type { DetailColumn, Details } from "./types";

/** The title of each optional column. */
export const COL_LABEL = {
  artist: "colArtist", album: "colAlbum", title: "colTitle", track: "colTrack", year: "colYear", genre: "colGenre",
  length: "colLength", quality: "colQuality", resolution: "colResolution", codec: "colCodec", taken: "colTaken",
} as const;

/** `3:45`, `1:02:03`. */
function clock(s: number) {
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}

let takenFmt: Intl.DateTimeFormat | null = null;
let takenLang = "";

/** A file's value in an optional column, as the list shows it. */
export function detailText(c: DetailColumn, d: Details | undefined): string {
  if (!d) return "";
  switch (c) {
    case "year":
      return d.year ? String(d.year) : "";
    case "length":
      return d.length ? clock(d.length) : "";
    case "taken": {
      if (d.taken === null) return "";
      if (!takenFmt || takenLang !== settings.lang) {
        takenLang = settings.lang;
        // The photo's own clock, as it was taken: no time zone moves it.
        takenFmt = new Intl.DateTimeFormat(LOCALES[settings.lang], { dateStyle: "short", timeStyle: "short", timeZone: "UTC" });
      }
      return takenFmt.format(new Date(d.taken * 1000));
    }
    default:
      return d[c];
  }
}
