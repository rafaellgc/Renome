// Mirrors the types serialized by the Rust core (renome-core).

export type Target = "name" | "ext" | "full";
export type CaseMode = "lower" | "upper" | "sentence" | "title" | "titleSmart";
export type Anchor = "start" | "end" | "index" | "fromEnd";
export type Bound = "start" | "end" | "position" | "text";

export type Rule =
  | { type: "replace"; find: string; replace: string; caseSensitive: boolean; firstOnly: boolean; target: Target }
  | { type: "regex"; pattern: string; replace: string; caseSensitive: boolean; firstOnly: boolean; target: Target }
  | { type: "case"; mode: CaseMode; target: Target }
  | {
      type: "insert";
      source: "text" | "now" | "modified" | "taken" | "folder";
      text: string; dateFormat: string; fallback: boolean;
      at: Anchor; position: number;
    }
  | { type: "number"; template: string; start: number; step: number; digits: number; perFolder: boolean }
  | { type: "remove"; allSpaces: boolean; extraSpaces: boolean; accents: boolean; digits: boolean; chars: string; leadingNumber: boolean; maxLength: number; target: Target }
  | { type: "convert"; mojibake: boolean; latin: boolean; web: "" | "-" | "_"; target: Target }
  | {
      type: "removeRange";
      from: Bound; fromPosition: number; fromText: string;
      to: Bound; toPosition: number; toText: string;
      inclusive: boolean;
    }
  | { type: "charAt"; position: number; fromEnd: boolean; onlyIf: string; caseSensitive: boolean; replace: string }
  | { type: "extension"; mode: "set" | "lower" | "upper" | "remove" | "content"; value: string }
  | { type: "rearrange"; separator: string; order: string; target: Target }
  | {
      type: "movie"; template: string; colon: string; sidecar: boolean; fanart: boolean; folder: boolean; missing: boolean;
      /** Download subtitles in `subLangs` (`pt-BR, en`) with the rename. */
      subtitles?: boolean; subLangs?: string;
    }
  | {
      type: "series"; template: string; colon: string; show: string;
      folders: boolean; folderTemplate: string; sidecar: boolean; fanart: boolean; missing: boolean;
      subtitles?: boolean; subLangs?: string;
      /** Where shows are looked up (TMDB when not set). */
      source?: "tmdb" | "anilist";
    }
  | {
      type: "music"; template: string; colon: string; discPrefix: boolean; fromName: boolean;
      folders: boolean; folderTemplate: string; cover: boolean; playlist: boolean; listen: boolean; writeTags: boolean; embedCover: boolean; coverSize: number; cleanTags: boolean; missing: boolean;
    }
  | { type: "book"; template: string; colon: string; folders: boolean; folderTemplate: string; cover: boolean; opf: boolean; writeEpub?: boolean; missing: boolean }
  | { type: "comic"; template: string; colon: string; series: string; source: ComicSource; folders: boolean; folderTemplate: string; byFolder: boolean; writeInfo?: boolean; cleanInfo?: boolean; convert?: boolean; original?: "keep" | "delete"; drawPages?: boolean; pageSize?: number; missing: boolean; barcode?: boolean; skipPublishers?: string }
  | {
      type: "photo"; template: string; fromName: boolean; modified: boolean; shiftHours: number;
      folders: boolean; folderTemplate: string; setModified: boolean;
    };

export type RuleType = Rule["type"];

export interface Step {
  id: string;
  enabled: boolean;
  rule: Rule;
}

export type Status = "unchanged" | "ok" | "conflict";
export type Problem = "empty" | "slash" | "tooLong" | "windowsChars" | "diskChars" | "duplicate" | "copy" | "exists";

export interface PreviewItem {
  path: string;
  dir: string;
  size: number;
  modified: number | null;
  isDir: boolean;
  original: string;
  newName: string;
  /** Folder (inside dir) the file moves into. */
  folder: string | null;
  /** The folder it ends up in, when it moves. */
  dest: string | null;
  status: Status;
  problem: Problem | null;
  windowsUnsafe: boolean;
  manual: boolean;
  /** A song the music rule cannot name (no title in its tags). */
  untagged: boolean;
  /** Matched to a release whose details its tags do not have yet. */
  retag: boolean;
  /** Matched, with tags that cleaning would take away. */
  junk: boolean;
  /** A comic the comic rule converts to CBZ (a RAR, 7z or tar inside, or a PDF). */
  convert: boolean;
  /** A PDF converted: how many of its pages would be drawn. */
  drawn?: number;
  /** With a photo rule, where a photo's date came from ("none": it has none). */
  photoDate?: "inside" | "name" | "modified" | "none";
  /** The name the lookups read, when the rules above the first lookup rule
   *  change it (else the original). */
  lookupName?: string;
}

export interface Preview {
  items: PreviewItem[];
  ruleErrors: { index: number; code: "invalidRegex" | "invalidDateFormat"; detail: string }[];
  filterError: string | null;
  total: number;
  changed: number;
  conflicts: number;
  /** A show's old folders renamed for the files that stay in them. */
  leftFolders: { from: string; to: string; files: number }[];
}

export interface ScanOptions {
  recursive: boolean;
  includeHidden: boolean;
  /** Adds folders themselves (to rename them) instead of their contents. */
  foldersAsItems: boolean;
}

export interface View {
  filter: string;
  sortByNewName: boolean;
  descending: boolean;
  identicalOnly: boolean;
  /** Only songs that are the same music as another, grouped. */
  alikeOnly: boolean;
  /** Only pictures that look like another, grouped. */
  similarOnly: boolean;
  /** The movie chosen for each file, for the movie rule. */
  movies: Record<string, MovieMatch>;
  /** The episode chosen for each file, for the series rule. */
  episodes: Record<string, EpisodeMatch>;
  /** Each song's details from its MusicBrainz release, for the music rule. */
  songs: Record<string, SongMatch>;
  /** The book chosen for each file, for the book rule. */
  books: Record<string, BookMatch>;
  /** The issue chosen for each file (Comic Vine), for the comic rule. */
  comics: Record<string, ComicMatch>;
  /** What a show's old folder is called after when files stay in it. */
  leftoverSuffix: string;
  /** Another folder the files go to, moved or copied. */
  sendTo: { dir: string; copy: boolean } | null;
}

/** A MusicBrainz release offered for an album, best first. */
export interface MbCandidate {
  id: string;
  title: string;
  artist: string;
  date: string;
  country: string;
  format: string;
  label: string;
  tracks: number;
  kind: string;
  score: number;
  /** A small cover from the search (Discogs); else the Cover Art Archive's. */
  image?: string;
}

/** A field of the details window: its key, its label (already
 *  translated), whether only digits go in it and whether it takes the
 *  whole row. */
export interface EditField {
  key: string;
  label: string;
  numeric?: boolean;
  wide?: boolean;
}

export interface SongMatch {
  release: string;
  title: string;
  artist: string;
  albumArtist: string;
  album: string;
  year: number | null;
  track: number;
  trackTotal: number;
  disc: number;
  discTotal: number;
  date?: string;
  recording?: string;
  trackId?: string;
  releaseGroup?: string;
  artistIds?: string[];
  albumArtistIds?: string[];
  /** Typed in by hand. */
  genre?: string;
  /** Typed in by hand, not from a release (no catalogue ids). */
  typed?: boolean;
}

export interface AlbumQuery {
  artist: string;
  album: string;
  year: number | null;
  tracks: number;
}

export interface AlbumLookup {
  files: string[];
  dir: string;
  query: AlbumQuery;
  candidates: MbCandidate[];
  choice: MbCandidate | null;
  matches: [string, SongMatch][];
  /** The chosen release's tracks. */
  tracks: ListedTrack[];
  error: string | null;
  /** Identified by the songs' sound (AcoustID), not their tags. */
  heard: boolean;
}

/** A release's track, for those missing from an album. */
export interface ListedTrack {
  id: string;
  disc: number;
  position: number;
  title: string;
  /** Seconds. */
  length: number | null;
}

/** A TMDB collection of movies (a trilogy, say). */
export interface MovieSet {
  id: number;
  name: string;
  parts: { id: number; title: string; releaseDate: string }[];
}

export interface AlbumState {
  files: string[];
  dir: string;
  query: AlbumQuery;
  candidates: MbCandidate[];
  choice: MbCandidate | null;
  matches: Record<string, SongMatch>;
  /** The chosen release's tracks (empty without one). */
  tracks: ListedTrack[];
  status: MovieStatus;
  error: string | null;
  heard: boolean;
}

/** A TMDB movie offered for a file, best first. */
export interface Candidate {
  id: number;
  title: string;
  originalTitle: string;
  year: number | null;
  overview: string;
  posterPath: string | null;
  score: number;
}

export type MovieMatch = Pick<Candidate, "id" | "title" | "originalTitle" | "year"> & {
  /** Template fields from TMDB (genre, director…), by field. */
  extra?: Record<string, string>;
};

export interface Guess {
  title: string;
  year: number | null;
}

/** auto: picked with confidence; chosen: by the user; choose: waiting for
 *  the user; none: no movie (no results, or the user said none fits). */
export type MovieStatus = "auto" | "chosen" | "choose" | "none";

export interface MovieState {
  guess: Guess;
  candidates: Candidate[];
  choice: Candidate | null;
  status: MovieStatus;
  error: string | null;
}

export interface Lookup {
  path: string;
  guess: Guess;
  candidates: Candidate[];
  auto: boolean;
  error: string | null;
}

/** What a book is thought to be, from its details or its name. */
export interface BookGuess {
  title: string;
  author: string;
  year: number | null;
  isbns: string[];
  fromFile: boolean;
  series: string;
  seriesIndex: string;
  publisher: string;
  language: string;
}

/** An Open Library book offered for a file, best first. */
export interface BookCandidate {
  work: string;
  edition: string;
  title: string;
  originalTitle: string;
  authors: string[];
  year: number | null;
  editionYear: number | null;
  publisher: string;
  isbn: string;
  language: string;
  pages: number | null;
  cover: number | null;
  /** Where it was found besides Open Library: `google`, `brasilapi`. */
  source?: string;
  /** A cover elsewhere (Google Books). */
  coverUrl?: string;
  /** A description elsewhere (Google Books). */
  description?: string;
  editions: number;
  /** Found by the file's ISBN: edition, publisher and ISBN are its own. */
  exact: boolean;
  score: number;
}

export interface BookMatch {
  work: string;
  title: string;
  originalTitle: string;
  authors: string[];
  year: number | null;
  editionYear: number | null;
  publisher: string;
  isbn: string;
  series: string;
  seriesIndex: string;
  /** Template fields (language, pages), by field. */
  extra?: Record<string, string>;
}

export interface BookLookup {
  path: string;
  guess: BookGuess;
  candidates: BookCandidate[];
  auto: boolean;
  error: string | null;
}

export interface BookState {
  guess: BookGuess;
  candidates: BookCandidate[];
  choice: BookCandidate | null;
  status: MovieStatus;
  error: string | null;
}

/** Season and episode. */
export type EpKey = [number, number];

export interface Episode {
  season: number;
  number: number;
  name: string;
  /** The title in other languages (for matching). */
  names: string[];
  airDate: string | null;
  overview: string;
  stillPath: string | null;
}

export interface Show {
  /** Empty for TMDB; `anilist`. */
  source?: string;
  id: number;
  name: string;
  originalName: string;
  year: number | null;
  overview: string;
  posterPath: string | null;
  episodes: Episode[];
  /** Its other titles. */
  aliases: string[];
  /** Template fields (network, genre…), by field. */
  extra?: Record<string, string>;
}

export interface EpisodeGuess {
  show: string;
  season: number | null;
  episodes: number[];
  number: number | null;
  date: string | null;
  title: string;
}

export interface EpisodeLookup {
  path: string;
  guess: EpisodeGuess;
  /** Picked with confidence; empty when the user has to choose. */
  choice: EpKey[];
  /** The numbers in the name point to another episode than its title. */
  conflict: boolean;
  /** Likely episodes, best first. */
  alternatives: EpKey[][];
}

export interface EpisodeState {
  showId: number;
  guess: EpisodeGuess;
  alternatives: EpKey[][];
  conflict: boolean;
  choice: EpKey[] | null;
  status: MovieStatus;
}

/** A subtitle found for a video (see the core's `subtitles::Candidate`). */
export interface SubCandidate {
  source: "openSubtitles" | "subdl";
  id: string;
  lang: string;
  release: string;
  downloads: number;
  /** Made for this very file. */
  exact: boolean;
  hearingImpaired: boolean;
  trusted: boolean;
  url: string;
}

/** The keys and account for the subtitle services. */
export interface SubsAuth { osKey: string; osUser: string; osPassword: string; subdlKey: string }

export interface EpisodeMatch {
  /** Empty for TMDB; `anilist`. */
  source?: string;
  id: number;
  show: string;
  originalShow: string;
  year: number | null;
  season: number;
  episodes: number[];
  titles: string[];
  /** Template fields: the show's, the first episode's air date and
   *  absolute number. */
  extra?: Record<string, string>;
}

export type ErrorKind = "tempExists" | "targetExists" | "notFound" | "permissionDenied" | "otherDisk" | "io" | "restoreFailed" | "history" | "tags";
export interface OpError { from: string; to: string; kind: ErrorKind; detail: string }

export interface ApplyReport {
  /** `copy`: a copy made (or, on undo, deleted: from the copy to its original). */
  done: { from: string; to: string; copy?: boolean }[];
  errors: OpError[];
  historyId: string | null;
  /** Cue sheets and playlists updated to the new names (or rebuilt). */
  lists: number;
  /** Useless cue sheets and playlists deleted. */
  listsRemoved: number;
  /** Copies undo left because they changed after they were made. */
  kept?: string[];
  /** Copies undo deleted. */
  copiesRemoved?: number;
  /** Stopped before the end. */
  stopped?: boolean;
}

export interface HistoryEntry {
  id: string;
  created: string;
  count: number;
  undone: boolean;
  /** Undone so far, when an undo was partial. */
  reverted: number;
  /** Failed on the last undo attempt. */
  failed: number;
  /** Files created with it (.nfo, images). */
  extra: number;
  /** Songs whose tags were written. */
  tagged: number;
}

/** A file's identical copies in the list: its group number (from 0) and
 *  the other files in the group. */
export interface Copies {
  group: number;
  others: string[];
}

/** The optional columns of the list: what the files hold. */
export const DETAIL_COLUMNS = ["artist", "album", "title", "track", "year", "genre", "length", "quality", "resolution", "codec", "taken"] as const;
export type DetailColumn = (typeof DETAIL_COLUMNS)[number];

/** What the optional columns show about a file (empty: it has none). */
export interface Details {
  artist: string;
  album: string;
  title: string;
  track: string;
  year: number | null;
  genre: string;
  /** Seconds. */
  length: number | null;
  quality: string;
  resolution: string;
  codec: string;
  /** Unix seconds of the photo's local time. */
  taken: number | null;
}

/** File list sort orders (done in Rust). */
export type SortKey = "name" | "ext" | "dir" | "modified" | "size" | DetailColumn;
/** Displayed order: a column, the new name or the manual order. */
export type ViewSort = SortKey | "newName" | "manual";

/** A comic as the comic rule names it: what the file says, or its issue on Comic Vine. */
export interface ComicMatch {
  series: string;
  seriesYear: number | null;
  number: string;
  volume: number | null;
  chapter: string;
  title: string;
  year: number | null;
  count: number | null;
  publisher: string;
  writer: string;
  artist: string;
  /** The issue on Comic Vine. */
  id: string;
  /** Template fields (cover date, story arc, colorist…), by field. */
  extra?: Record<string, string>;
}

/** Where comics are looked up. */
export type ComicSource = "gcd" | "comicvine" | "metron" | "mangadex" | "anilist" | "mangaupdates";

/** A series in a catalogue (a "volume" on Comic Vine). */
export interface ComicVolume {
  id: string;
  name: string;
  startYear: number | null;
  publisher: string;
  count: number;
  cover: string;
  country: string;
  language: string;
  format: string;
  writer: string;
}

export interface ComicIssue {
  id: string;
  number: string;
  name: string;
  coverDate: string;
  cover: string;
}

/** A comic file's series and issue in a catalogue. */
export interface ComicState {
  /** `source:id` of its series in `comicVolumes`. */
  volumeKey: string;
  /** What the file says about itself. */
  own: ComicMatch;
  /** The issue chosen; "" for the series alone (the manga catalogues); null for none. */
  choice: string | null;
  status: MovieStatus;
}
