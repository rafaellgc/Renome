// The book rule's side of the main page: what Open Library (and the
// others) found per file, the choices, the details typed in and what
// writing them needs.

import { modelOf, screenMm } from "../readers";
import { api } from "$lib/api";
import { notify } from "$lib/dialogs.svelte";
import { t, tError } from "$lib/i18n";
import { isAudio, isBook, splitExt } from "$lib/rules";
import { tmdbLang } from "$lib/settings.svelte";
import type { BookCandidate, BookGuess, BookMatch, BookPart, BookState, BookTarget, EditField } from "$lib/types";
import { counts, isEpub, moved, ruleOf, without, type PageCtx, isWritableBook } from "./common";

/** A book as the file says it is (its details and name), to type over. */
function fromGuess(g: BookGuess): BookMatch {
  return {
    work: "", title: g.title, originalTitle: "", authors: g.author ? g.author.split(/\s*[;&]\s*/).filter(Boolean) : [],
    year: g.year, editionYear: null, publisher: g.publisher, isbn: g.isbns[0] ?? "", series: g.series, seriesIndex: g.seriesIndex,
    extra: g.language ? { language: g.language } : {},
  };
}
const NO_GUESS: BookGuess = { title: "", author: "", year: null, isbns: [], fromFile: false, series: "", seriesIndex: "", publisher: "", language: "" };


export class Books {
  #ctx: PageCtx;
  /** Audiobooks' files, by path. */
  parts = $state<Record<string, BookPart>>({});
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

  /** Each chosen book's details as the rule names it (made by the shared
   *  flow: one spelling per author across the list, an audiobook's files
   *  with their part); rebuilt whenever the choices change. */
  built = $state<Record<string, BookMatch>>({});
  /** Each book's details for naming and writing: chosen or typed in. */
  matches = $derived.by(() => ({ ...this.built, ...this.edits }));

  /** Asks the shared flow for the books' details, from the choices. */
  async rebuild() {
    const chosen = Object.entries(this.state).flatMap(([path, b]) => (b.choice ? [{ path, guess: b.guess, choice: b.choice }] : []));
    this.built = chosen.length ? await api.bookMatches($state.snapshot(chosen), $state.snapshot(this.parts), this.audio) : {};
  }
  counts = $derived.by(() => counts(this.state, this.#ctx.items));
  /** The books among the rows chosen in the list. */
  selected = $derived.by(() => this.#ctx.items.filter((i) => this.#ctx.selected.has(i.path) && !i.isDir && this.takes(i.original)));



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







  /** Looks up the visible books without a choice yet on Open Library;
   *  books picked with confidence are done, the rest go to the review.
   *  Which files, the audiobooks and what is sure, the shared flow says. */
  async search() {
    const open = (p: string) => this.state[p]?.status !== "auto" && this.state[p]?.status !== "chosen";
    const settled = this.#ctx.items.filter((i) => !open(this.parts[i.path]?.leader ?? i.path)).map((i) => i.path);
    this.progress = { done: 0, total: 0, kind: "search" };
    try {
      const res = await api.booksSearch(tmdbLang(), this.#ctx.items, settled, this.audio);
      if (res.empty) {
        this.#ctx.say(t("bookNoFiles"));
        return;
      }
      this.parts = { ...this.parts, ...res.parts };
      for (const f of res.found) {
        this.state[f.path] = {
          guess: f.guess,
          candidates: f.candidates,
          choice: f.choice,
          status: f.choice ? "auto" : f.candidates.length ? "choose" : "none",
          error: f.error,
          group: f.group,
        };
      }
      await this.rebuild();
      if (res.found.some((f) => !f.choice && f.candidates.length)) this.openReview();
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

  /** The user's pick for a book; undecided files of its group (the same
   *  book in other formats) get it too. */
  async choose(path: string, choice: BookCandidate | null, candidates?: BookCandidate[]) {
    const b = this.state[path];
    if (!b) return;
    const set = (p: string) => {
      this.state[p] = { ...this.state[p], choice, status: choice ? "chosen" : "none", candidates: candidates ?? this.state[p].candidates };
    };
    set(path);
    for (const [p, o] of Object.entries(this.state)) {
      if (p !== path && o.group === b.group && (o.status === "choose" || o.status === "none")) set(p);
    }
    await this.rebuild();
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
    this.built = paths ? without(this.built, paths) : {};
  }

  /** The files renamed: what was found and typed follows them. */
  follow(to: (p: string) => string) {
    this.state = moved(this.state, to);
    this.edits = moved(this.edits, to);
    this.parts = Object.fromEntries(Object.entries(moved(this.parts, to)).map(([p, g]) => [p, { ...g, leader: to(g.leader) }]));
    this.built = moved(this.built, to);
  }

  /** What was found, to put back with Ctrl+Z. */
  snapshot() {
    return { state: this.state, edits: this.edits, parts: this.parts, built: this.built };
  }
  restore(s: ReturnType<Books["snapshot"]>) {
    this.state = s.state;
    this.edits = s.edits;
    this.parts = s.parts;
    this.built = s.built;
  }
}
