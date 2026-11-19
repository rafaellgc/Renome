// The book rule's side of the main page: what Open Library (and the
// others) found per file, the choices, the details typed in and what
// writing them needs.

import { api } from "$lib/api";
import { notify } from "$lib/dialogs.svelte";
import { t, tError } from "$lib/i18n";
import { tmdbLang } from "$lib/settings.svelte";
import type { BookCandidate, BookMatch, BookPart, BookState, EditField } from "$lib/types";
import { counts, editFields, moved, ruleOf, takers, without, type PageCtx } from "./common";



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
  /** The rows chosen in the list the details window edits (the core says). */
  selected = $derived.by(() => this.#ctx.items.filter((i) => this.#ctx.selected.has(i.path) && i.editable));

  /** The details window's fields. */
  fields = $derived<EditField[]>(editFields("book"));

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
          status: f.status,
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
   *  book in other formats) get it too (the shared rule). */
  async choose(path: string, choice: BookCandidate | null, candidates?: BookCandidate[]) {
    if (!this.state[path]) return;
    for (const p of await takers(path, this.state)) {
      this.state[p] = { ...this.state[p], choice, status: choice ? "chosen" : "none", candidates: candidates ?? this.state[p].candidates };
    }
    await this.rebuild();
  }

  /** Opens the details window on the books chosen in the list (an
   *  audiobook's files together); what each starts from, the shared flow
   *  says (the command line's --edit too). */
  async openEdit() {
    if (!this.#ctx.selected.size) return;
    const known = Object.fromEntries(
      this.#ctx.items.map((i) => [i.path, { current: this.matches[i.path], guess: this.state[i.path]?.guess }]),
    );
    const [paths, books] = await api.editStart<BookMatch>({
      kind: "book",
      chosen: [...this.#ctx.selected],
      known: $state.snapshot(known),
      parts: $state.snapshot(this.parts),
      dates: {},
      fromName: false,
      byFolder: false,
      audiobooks: this.audio,
    });
    if (paths.length) this.edit = { paths, books };
  }

  /** The fields changed in the details window, for each of its books (applied as the command line's --edit does). */
  async saveEdit(changed: Record<string, string>) {
    const edit = this.edit;
    if (!edit) return;
    const books = await api.editDetails("book", $state.snapshot(edit.books), changed);
    const next = { ...this.edits };
    edit.paths.forEach((p, i) => (next[p] = books[i]));
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
