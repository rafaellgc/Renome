// The book rule's side of the main page: what Open Library (and the
// others) found per file, the choices, the details typed in and what
// writing them needs.

import { modelOf, screenMm } from "../readers";
import { api } from "$lib/api";
import { authorSpellings } from "$lib/authors";
import { notify } from "$lib/dialogs.svelte";
import { t, tError } from "$lib/i18n";
import { isAudio, isBook, splitExt } from "$lib/rules";
import { tmdbLang } from "$lib/settings.svelte";
import type { AudioBook, BookCandidate, BookGuess, BookMatch, BookState, BookTarget, EditField } from "$lib/types";
import { counts, isEpub, moved, ruleOf, without, type PageCtx, isWritableBook } from "./common";

/** What the rule uses of a chosen book: the edition's publisher and ISBN
 *  only when it was found by the file's ISBN (otherwise they would be
 *  another edition's), else the file's own. */
function bookMatch(b: BookState): BookMatch {
  const c = b.choice!;
  return {
    work: c.work, title: c.title, originalTitle: c.originalTitle, authors: c.authors, year: c.year,
    editionYear: c.exact ? c.editionYear : null,
    publisher: c.exact ? c.publisher : b.guess.publisher,
    isbn: c.exact ? c.isbn : (b.guess.isbns[0] ?? ""),
    series: b.guess.series, seriesIndex: b.guess.seriesIndex,
    extra: Object.fromEntries(
      [["language", c.exact ? c.language : b.guess.language || c.language], ["pages", c.pages ? String(c.pages) : ""]].filter(([, v]) => v),
    ),
  };
}

/** A book as the file says it is (its details and name), to type over. */
function fromGuess(g: BookGuess): BookMatch {
  return {
    work: "", title: g.title, originalTitle: "", authors: g.author ? g.author.split(/\s*[;&]\s*/).filter(Boolean) : [],
    year: g.year, editionYear: null, publisher: g.publisher, isbn: g.isbns[0] ?? "", series: g.series, seriesIndex: g.seriesIndex,
    extra: g.language ? { language: g.language } : {},
  };
}
const NO_GUESS: BookGuess = { title: "", author: "", year: null, isbns: [], fromFile: false, series: "", seriesIndex: "", publisher: "", language: "" };

/** An audiobook's file: the file its book is looked up by (its first),
 *  its place among the book's files and what its tags say. */
interface Part { leader: string; part: number; parts: number; narrator: string; series: string; seriesIndex: string }

export class Books {
  #ctx: PageCtx;
  /** Audiobooks' files, by path. */
  parts = $state<Record<string, Part>>({});
  /** Results and choices per file. */
  state = $state<Record<string, BookState>>({});
  /** Books whose details were typed in (over what the catalogue said). */
  edits = $state<Record<string, BookMatch>>({});
  /** The details window, while open. */
  edit = $state<{ paths: string[]; books: BookMatch[] } | null>(null);
  /** Files in the review window; null when closed. */
  review = $state<string[] | null>(null);
  progress = $state<{ done: number; total: number; kind: "search" | "save" } | null>(null);

  constructor(ctx: PageCtx) {
    this.#ctx = ctx;
  }

  rule = $derived.by(() => ruleOf(this.#ctx.steps, "book"));
  /** The rule takes audiobooks too (unless turned off). */
  audio = $derived.by(() => {
    const r = this.rule?.rule;
    return r?.type === "book" && r.audiobooks !== false;
  });

  /** A file the rule names: a book, or an audiobook's audio. */
  takes(name: string) {
    return isBook(name) || (this.audio && isAudio(name));
  }

  /** The file an audiobook is looked up, named on its cover and written
   *  about by (its first); any book is its own. */
  leads(p: string) {
    return (this.parts[p]?.leader ?? p) === p;
  }
  /** Each book's details for naming and writing: chosen (one spelling per
   *  author across the list) or typed in. */
  matches = $derived.by(() => {
    const raw = Object.entries(this.state).filter(([p, b]) => b.choice && (this.audio || !isAudio(p))).map(([p, b]): [string, BookMatch] => [p, bookMatch(b)]);
    const spelling = authorSpellings(raw.flatMap(([, m]) => m.authors));
    const same = (a: string) => spelling.get(a.normalize("NFC")) ?? a.normalize("NFC");
    const out: Record<string, BookMatch> = Object.fromEntries(
      raw.map(([p, m]) => [p, { ...m, title: m.title.normalize("NFC"), authors: [...new Set(m.authors.map(same))] }]),
    );
    // An audiobook's files: its book, their part and what their tags say.
    for (const [p, g] of this.audio ? Object.entries(this.parts) : []) {
      const m = out[g.leader];
      if (m && !this.edits[p])
        out[p] = { ...m, narrator: g.narrator, part: g.part, parts: g.parts, series: m.series || g.series, seriesIndex: m.seriesIndex || g.seriesIndex };
    }
    return { ...out, ...this.edits };
  });
  counts = $derived.by(() => counts(this.state, this.#ctx.items));
  /** The books among the rows chosen in the list. */
  selected = $derived.by(() => this.#ctx.items.filter((i) => this.#ctx.selected.has(i.path) && !i.isDir && this.takes(i.original)));
  /** Books whose names are already right but that still get a cover or a
   *  metadata.opf (files that already exist are skipped when writing). */
  metaOnly = $derived.by(() => {
    const r = this.rule?.rule;
    return r?.type === "book" && (r.cover || r.metadataOpf)
      ? this.#ctx.items.filter((i) => i.status === "unchanged" && this.has(i.path) && (isBook(i.original) || (isAudio(i.original) && this.leads(i.path))))
      : [];
  });
  /** Books whose names are already right but whose details get written
   *  or that get polished (those that already have them are skipped). */
  epubOnly = $derived.by(() =>
    this.#ctx.items.filter((i) => i.status === "unchanged" && !i.isDir && !i.bookTo && this.writes(i.path, i.path)),
  );

  /** The EPUB polish the rule asks for, if any. */
  get polish() {
    const r = this.rule?.rule;
    return r?.type === "book" && (r.epub3 || r.shrinkImages) ? { epub3: !!r.epub3, shrink: !!r.shrinkImages } : null;
  }

  /** Whether a book (`key` before renaming, `path` after) gets its details
   *  written inside or gets polished. */
  writes(key: string, path: string) {
    const r = this.rule?.rule;
    if (r?.type !== "book") return false;
    return (!!r.writeEpub && this.has(key) && isWritableBook(path)) || (!!this.polish && isEpub(path));
  }

  /** What writing a book's details inside it and polishing it need. */
  writeItem(key: string, path: string) {
    const r = this.rule?.rule;
    const details = r?.type === "book" && r.writeEpub && this.has(key) ? this.insideItem(key, path) : { path };
    const polish = isEpub(path) ? this.polish : null;
    return polish ? { ...details, polish } : details;
  }
  /** Books whose names are already right but that get converted (an
   *  `.epub` that is really another format). */
  convertOnly = $derived.by(() => {
    const r = this.rule?.rule;
    return r?.type === "book" && r.convertTo ? this.#ctx.items.filter((i) => i.status === "unchanged" && !!i.bookTo) : [];
  });
  /** The details window's fields. */
  fields = $derived<EditField[]>([
    { key: "title", label: t("bookEditTitle"), wide: true },
    { key: "authors", label: t("bookEditAuthors"), wide: true },
    { key: "year", label: t("bookEditYear"), numeric: true },
    { key: "publisher", label: t("bookEditPublisher") },
    { key: "language", label: t("bookEditLanguage") },
    { key: "series", label: t("bookEditSeries"), wide: true },
    { key: "seriesIndex", label: t("bookEditSeriesIndex") },
    { key: "isbn", label: t("bookEditIsbn") },
  ]);

  /** A book has details to name and write it with: chosen or typed in. */
  has(p: string) {
    const leader = this.parts[p]?.leader ?? p;
    return !!(this.state[leader]?.choice || this.edits[p]);
  }

  /** A book's details as the window shows them. */
  text(m: BookMatch): Record<string, string> {
    return {
      title: m.title,
      authors: m.authors.join("; "),
      year: m.year ? String(m.year) : "",
      publisher: m.publisher,
      language: m.extra?.language ?? "",
      series: m.series,
      seriesIndex: m.seriesIndex,
      isbn: m.isbn,
    };
  }

  /** What writing a book's details inside it needs; `key` is the path before renaming. */
  insideItem(key: string, path: string) {
    const typed = this.edits[key];
    if (typed) return { path, book: typed, language: typed.extra?.language ?? "" };
    const b = this.state[key];
    const c = b.choice!;
    return { path, book: this.matches[key], language: c.exact ? c.language : b.guess.language };
  }

  /** What converting a book needs: the format, the original's fate and,
   *  when it has some, the details to write in it. */
  convertItem(key: string, path: string, to: BookTarget) {
    const r = this.rule?.rule;
    const original = r?.type === "book" ? (r.original ?? "keep") : "keep";
    const base = this.has(key) ? this.insideItem(key, path) : { path };
    // A PDF the size of the chosen reader's screen.
    const device = r?.type === "book" ? modelOf(r.model)?.device : undefined;
    const pdfPage = to === "pdf" && device ? screenMm(device) : null;
    return { ...base, bookTo: to, convert: original, ...(pdfPage ? { pdfPage } : {}) };
  }

  /** Whether the rule writes audiobooks' details inside their files. */
  get audioTags() {
    const r = this.rule?.rule;
    return r?.type === "book" && this.audio && !!r.audioTags;
  }

  /** Audiobook files whose names are already right but that get their
   *  book's details written (files that already have them are skipped). */
  tagsOnly = $derived.by(() =>
    this.audioTags ? this.#ctx.items.filter((i) => i.status === "unchanged" && !i.isDir && isAudio(i.original) && this.has(i.path)) : [],
  );

  /** What writing an audiobook file's details needs: its book, with this
   *  file's part. */
  audioTagItem(key: string, path: string) {
    const g = this.parts[key];
    // The catalogue's cover and synopsis are the book's, chosen for its
    // first file.
    const item = this.sidecarItem(g?.leader ?? key, path);
    const m = this.matches[key] ?? item.book;
    return g ? { ...item, book: { ...m, part: g.part, parts: g.parts, narrator: m.narrator || g.narrator } } : { ...item, book: m };
  }

  /** What the cover and metadata.opf of a book need; `key` is the path before renaming. */
  sidecarItem(key: string, path: string) {
    const typed = this.edits[key];
    const b = this.state[key];
    const c = b?.choice;
    // Typed in: the catalogue's cover and description only while it is still that book.
    if (typed) {
      const same = !!(c && typed.work);
      return { path, book: typed, cover: same ? c!.cover : null, coverUrl: same ? (c!.coverUrl ?? "") : "", description: same ? (c!.description ?? "") : "", language: typed.extra?.language ?? "" };
    }
    if (!c) throw new Error("no book");
    return { path, book: this.matches[key], cover: c.cover, coverUrl: c.coverUrl ?? "", description: c.description ?? "", language: c.exact ? c.language : b.guess.language };
  }

  /** Looks up the visible books without a choice yet on Open Library;
   *  books picked with confidence are done, the rest go to the review. */
  async search() {
    const open = (p: string) => this.state[p]?.status !== "auto" && this.state[p]?.status !== "chosen";
    const items = this.#ctx.items
      .filter((i) => !i.isDir && isBook(i.original) && open(i.path))
      .map((i) => ({ path: i.path, name: splitExt(this.#ctx.readName(i))[0] }));
    // Audiobooks: their files grouped into books, each looked up once by
    // what its tags and folder say.
    const audio = this.audio
      ? this.#ctx.items.filter((i) => !i.isDir && isAudio(i.original) && open(this.parts[i.path]?.leader ?? i.path)).map((i) => i.path)
      : [];
    if (audio.length) {
      let found: AudioBook[] = [];
      try {
        found = await api.audiobookGroups(audio);
      } catch (e) {
        await notify({ title: t("rule.book"), text: tError(e) });
        return;
      }
      const next = { ...this.parts };
      for (const b of found) {
        const leader = b.paths[0];
        b.paths.forEach((p, n) => {
          next[p] = { leader, part: n + 1, parts: b.paths.length > 1 ? b.paths.length : 0, narrator: b.narrator, series: b.series, seriesIndex: b.seriesIndex };
        });
        const name = `${b.author ? `${b.author} - ` : ""}${b.title}${b.year ? ` (${b.year})` : ""}`;
        if (b.title || b.author) items.push({ path: leader, name });
      }
      this.parts = next;
    }
    if (!items.length) {
      this.#ctx.say(t("bookNoFiles"));
      return;
    }
    this.progress = { done: 0, total: items.length, kind: "search" };
    try {
      const found = await api.bookLookup(tmdbLang(), items);
      // Nothing was really searched (no connection): mark nothing.
      const failed = found.find((f) => f.error);
      if (failed && found.every((f) => f.error || (!f.guess.title && !f.guess.isbns.length))) {
        await notify({ title: t("rule.book"), text: tError(failed.error) });
        return;
      }
      for (const f of found) {
        this.state[f.path] = {
          guess: f.guess,
          candidates: f.candidates,
          choice: f.auto ? f.candidates[0] : null,
          status: f.auto ? "auto" : f.candidates.length ? "choose" : "none",
          error: f.error,
        };
      }
      if (found.some((f) => !f.auto && f.candidates.length)) this.openReview();
    } catch (e) {
      await notify({ title: t("rule.book"), text: tError(e) });
    } finally {
      this.progress = null;
    }
  }

  /** Opens the review on the books still without a choice (or on one). */
  openReview(start?: string) {
    const paths = this.#ctx.items.map((i) => i.path);
    const pending = [
      ...paths.filter((p) => this.state[p]?.status === "choose"),
      ...paths.filter((p) => this.state[p]?.status === "none"),
    ];
    const queue = start ? [start, ...pending.filter((p) => p !== start)] : pending;
    this.review = queue.length ? queue : null;
  }

  /** The user's pick for a book; undecided files with the same guess (the
   *  same book in other formats) get it too. */
  choose(path: string, choice: BookCandidate | null, candidates?: BookCandidate[]) {
    const b = this.state[path];
    if (!b) return;
    const set = (p: string) => {
      this.state[p] = { ...this.state[p], choice, status: choice ? "chosen" : "none", candidates: candidates ?? this.state[p].candidates };
    };
    set(path);
    for (const [p, o] of Object.entries(this.state)) {
      const same = o.guess.title === b.guess.title && o.guess.author === b.guess.author;
      if (p !== path && same && (o.status === "choose" || o.status === "none")) set(p);
    }
  }

  /** Opens the details window on the books chosen in the list. */
  async openEdit() {
    // An audiobook's files are edited together.
    const leaders = new Set(this.selected.map((i) => this.parts[i.path]?.leader).filter(Boolean));
    const rows = this.#ctx.items.filter((i) => this.selected.includes(i) || (!!this.parts[i.path] && leaders.has(this.parts[i.path].leader)));
    if (!rows.length) return;
    const missing = rows.filter((i) => !this.matches[i.path] && !this.state[i.path]);
    const guessed: Record<string, BookGuess> = {};
    if (missing.length) {
      const got = await api.bookGuesses(missing.map((i) => i.path), missing.map((i) => splitExt(this.#ctx.readName(i))[0]));
      missing.forEach((i, n) => (guessed[i.path] = got[n]));
    }
    this.edit = {
      paths: rows.map((i) => i.path),
      books: rows.map((i) => $state.snapshot(this.matches[i.path] ?? fromGuess(this.state[i.path]?.guess ?? guessed[i.path] ?? NO_GUESS))),
    };
  }

  /** The fields changed in the details window, for each of its books. */
  saveEdit(changed: Record<string, string>) {
    const edit = this.edit;
    if (!edit) return;
    const next = { ...this.edits };
    edit.paths.forEach((p, i) => {
      const m: BookMatch = { ...edit.books[i], extra: { ...(edit.books[i].extra ?? {}) } };
      for (const [k, v] of Object.entries(changed)) {
        if (k === "authors") m.authors = v.split(";").map((a) => a.trim()).filter(Boolean);
        else if (k === "year") {
          m.year = v ? Number.parseInt(v, 10) : null;
          // One year typed: the edition's goes.
          m.editionYear = null;
        } else if (k === "language") {
          if (v) m.extra!.language = v;
          else delete m.extra!.language;
        } else (m as unknown as Record<string, string>)[k] = v;
      }
      // Another book: the catalogue's work (and its cover) no longer tell it.
      if (changed.title !== undefined || changed.authors !== undefined) m.work = "";
      next[p] = m;
    });
    this.edits = next;
    this.edit = null;
  }

  /** Forgets the files taken out of the list (all, with null). */
  forget(paths: Set<string> | null) {
    this.state = paths ? without(this.state, paths) : {};
    this.edits = paths ? without(this.edits, paths) : {};
    this.parts = paths ? without(this.parts, paths) : {};
  }

  /** The files renamed: what was found and typed follows them. */
  follow(to: (p: string) => string) {
    this.state = moved(this.state, to);
    this.edits = moved(this.edits, to);
    this.parts = Object.fromEntries(Object.entries(moved(this.parts, to)).map(([p, g]) => [p, { ...g, leader: to(g.leader) }]));
  }

  /** What was found, to put back with Ctrl+Z. */
  snapshot() {
    return { state: this.state, edits: this.edits, parts: this.parts };
  }
  restore(s: ReturnType<Books["snapshot"]>) {
    this.state = s.state;
    this.edits = s.edits;
    this.parts = s.parts;
  }
}
