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
  | { type: "remove"; allSpaces: boolean; extraSpaces: boolean; accents: boolean; digits: boolean; chars: string; target: Target }
  | {
      type: "removeRange";
      from: Bound; fromPosition: number; fromText: string;
      to: Bound; toPosition: number; toText: string;
      inclusive: boolean;
    }
  | { type: "charAt"; position: number; fromEnd: boolean; onlyIf: string; caseSensitive: boolean; replace: string }
  | { type: "extension"; mode: "set" | "lower" | "upper" | "remove"; value: string }
  | { type: "rearrange"; separator: string; order: string; target: Target }
  | { type: "movie"; template: string; colon: string; sidecar: boolean; fanart: boolean; folder: boolean }
  | {
      type: "series"; template: string; colon: string; show: string;
      folders: boolean; folderTemplate: string; sidecar: boolean; fanart: boolean;
    }
  | {
      type: "music"; template: string; colon: string; discPrefix: boolean; fromName: boolean;
      folders: boolean; folderTemplate: string; cover: boolean; playlist: boolean; listen: boolean; writeTags: boolean; embedCover: boolean; cleanTags: boolean;
    }
  | { type: "book"; template: string; colon: string; folders: boolean; folderTemplate: string; cover: boolean; opf: boolean };

export type RuleType = Rule["type"];

export interface Step {
  id: string;
  enabled: boolean;
  rule: Rule;
}

export type Status = "unchanged" | "ok" | "conflict";
export type Problem = "empty" | "slash" | "tooLong" | "windowsChars" | "duplicate" | "copy" | "exists";

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
}

export interface Preview {
  items: PreviewItem[];
  ruleErrors: { index: number; code: "invalidRegex" | "invalidDateFormat"; detail: string }[];
  filterError: string | null;
  total: number;
  changed: number;
  conflicts: number;
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
  /** The movie chosen for each file, for the movie rule. */
  movies: Record<string, MovieMatch>;
  /** The episode chosen for each file, for the series rule. */
  episodes: Record<string, EpisodeMatch>;
  /** Each song's details from its MusicBrainz release, for the music rule. */
  songs: Record<string, SongMatch>;
  /** The book chosen for each file, for the book rule. */
  books: Record<string, BookMatch>;
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
  error: string | null;
  /** Identified by the songs' sound (AcoustID), not their tags. */
  heard: boolean;
}

export interface AlbumState {
  files: string[];
  dir: string;
  query: AlbumQuery;
  candidates: MbCandidate[];
  choice: MbCandidate | null;
  matches: Record<string, SongMatch>;
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

export type MovieMatch = Pick<Candidate, "id" | "title" | "originalTitle" | "year">;

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
  cover: number | null;
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
  id: number;
  name: string;
  originalName: string;
  year: number | null;
  overview: string;
  posterPath: string | null;
  episodes: Episode[];
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

export interface EpisodeMatch {
  id: number;
  show: string;
  originalShow: string;
  year: number | null;
  season: number;
  episodes: number[];
  titles: string[];
}

export type ErrorKind = "tempExists" | "targetExists" | "notFound" | "permissionDenied" | "io" | "restoreFailed" | "history" | "tags";
export interface OpError { from: string; to: string; kind: ErrorKind; detail: string }

export interface ApplyReport {
  done: { from: string; to: string }[];
  errors: OpError[];
  historyId: string | null;
  /** Cue sheets and playlists updated to the new names (or rebuilt). */
  lists: number;
  /** Useless cue sheets and playlists deleted. */
  listsRemoved: number;
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

/** File list sort orders (done in Rust). */
export type SortKey = "name" | "ext" | "dir" | "modified" | "size";
/** Displayed order: a column, the new name or the manual order. */
export type ViewSort = SortKey | "newName" | "manual";
