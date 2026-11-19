// The comic rule's side of the main page: the series and issues found in
// the catalogue chosen (GCD, Metron, Comic Vine or a manga one), the
// choices, the details typed in and what converting and writing need.

import { api } from "$lib/api";
import { confirm, notify } from "$lib/dialogs.svelte";
import { t, tError } from "$lib/i18n";
import { comicKey, langs, settings } from "$lib/settings.svelte";
import type { ComicIssue, ComicMatch, ComicSource, ComicState, ComicVolume, EditField, Rule, ComicFormat } from "$lib/types";
import { counts, editFields, moved, ruleOf, without, type PageCtx } from "./common";

/** What the series picker asks, while open. */
export type ComicPick = { source: ComicSource; skip: string; query: string; files: number; highest: number; candidates: ComicVolume[]; path?: string; comic?: ComicMatch; resolve: (v: ComicVolume | null) => void };

export class Comics {
  #ctx: PageCtx;
  /** The series and issue per file. */
  state = $state<Record<string, ComicState>>({});
  /** Series fetched with their issues, by `source:id`. */
  volumes = $state<Record<string, { volume: ComicVolume; issues: ComicIssue[] }>>({});
  /** The series picked for each search text (typed, or the files' series). */
  picked = $state<Record<string, ComicVolume>>({});
  /** Files in the review window; null when closed. */
  review = $state<string[] | null>(null);
  /** The series picker, while open. */
  pick = $state<ComicPick | null>(null);
  /** Whether this session was told that the GCD works better with an account. */
  #gcdWarned = false;
  progress = $state<string | null>(null);
  /** Comics whose details were typed in (over what the catalogue said). */
  edits = $state<Record<string, ComicMatch>>({});
  /** The details window, while open. */
  edit = $state<{ paths: string[]; comics: ComicMatch[] } | null>(null);

  constructor(ctx: PageCtx) {
    this.#ctx = ctx;
  }

  rule = $derived.by(() => ruleOf(this.#ctx.steps, "comic"));
  /** Each comic's details from its series and issue, with its page in the
   *  catalogue (made by the shared flow); rebuilt whenever they change. */
  built = $state<Record<string, { comic: ComicMatch; web: string; manga: boolean }>>({});
  /** Each comic's details: its issue's (or its series'), or typed in. */
  matches = $derived.by(() => ({
    ...Object.fromEntries(Object.entries(this.built).map(([p, n]): [string, ComicMatch] => [p, n.comic])),
    ...this.edits,
  }));

  /** Asks the shared flow for the comics' details, from the choices. */
  async rebuild() {
    const files = Object.entries(this.state).map(([path, c]) => ({ path, volumeKey: c.volumeKey, own: c.own, choice: c.choice }));
    const keys = new Set(files.map((f) => f.volumeKey));
    const volumes = Object.fromEntries(Object.entries(this.volumes).filter(([k]) => keys.has(k)));
    this.built = files.length ? await api.comicNamed($state.snapshot(files), $state.snapshot(volumes)) : {};
  }
  /** The comics among the rows chosen in the list. */
  selected = $derived.by(() => this.#ctx.items.filter((i) => this.#ctx.selected.has(i.path) && i.editable));
  counts = $derived.by(() => counts(this.state, this.#ctx.items));
  /** The details window's fields. */
  fields = $derived<EditField[]>(editFields("comic"));



  /** The chosen catalogue (GCD, Metron, Comic Vine, a manga one) for the
   *  visible comics: on Metron, first each file's barcode (recorded in it,
   *  or read in its pages when asked); then one series per group of files
   *  (the series typed, else each series the files say), picked by itself
   *  when only one has that name (and year), else asked; then each file's
   *  issue by its number. */
  async search(rule: Extract<Rule, { type: "comic" }>) {
    const source = rule.source ?? "gcd";
    const skip = rule.skipPublishers?.trim() ?? "";
    const key = comicKey(source);
    if ((source === "comicvine" || source === "metron") && !key) {
      const ok = await confirm({ title: t("rule.comic"), text: t(source === "metron" ? "mtNeedsKey" : "cvNeedsKey"), ok: t("options"), cancel: t("cancel") });
      if (ok) this.#ctx.openOptions();
      return;
    }
    // Without an account the GCD takes few searches: said once per session.
    if (source === "gcd" && !this.#gcdWarned && !(settings.gcdEmail.trim() && settings.gcdPassword)) {
      this.#gcdWarned = true;
      const ok = await confirm({ title: t("rule.comic"), text: t("gcdNoAccount"), ok: t("options"), cancel: t("gcdAnonymous") });
      if (ok) {
        this.#ctx.openOptions();
        return;
      }
    }
    // Which files, the barcodes, the series and issues and what is sure,
    // the shared flow says; a series it is not sure of comes as a question
    // (see {@link ask}).
    this.progress = t("comicReading");
    try {
      const params = {
        source,
        key,
        series: rule.series,
        skip,
        lang: langs.comic,
        uiLang: settings.lang,
        byFolder: !!rule.seriesByFolder,
        barcode: !!rule.barcode,
      };
      const res = await api.comicsSearch(params, this.#ctx.items, this.picked);
      if (res.empty) {
        this.#ctx.say(t("comicNoFiles"));
        return;
      }
      this.picked = res.picked;
      for (const [k, v] of Object.entries(res.volumes)) this.volumes[k] = v;
      for (const f of res.files) this.state[f.path] = { volumeKey: f.volumeKey, own: f.own, choice: f.choice, status: f.status, read: f.read };
      await this.rebuild();
      if (Object.values(this.state).some((c) => c.status === "choose")) this.openReview();
    } catch (e) {
      const err = String(e);
      // A refused account is fixed in the options.
      if (err.startsWith("gcdBadLogin") || err.startsWith("mtBadKey")) {
        if (await confirm({ title: t("rule.comic"), text: tError(e), ok: t("options"), cancel: t("close") })) this.#ctx.openOptions();
      } else {
        await notify({ title: t("rule.comic"), text: tError(e) });
      }
    } finally {
      this.progress = null;
      this.pick = null;
    }
  }

  /** Opens the review on the comics still without an issue (or on one). */
  openReview(start?: string) {
    const paths = this.#ctx.items.map((i) => i.path);
    const pending = [
      ...paths.filter((p) => this.state[p]?.status === "choose"),
      ...paths.filter((p) => this.state[p]?.status === "none"),
    ];
    const queue = start ? [start, ...pending.filter((p) => p !== start)] : pending;
    this.review = queue.length ? queue : null;
  }

  async choose(path: string, issue: string | null) {
    const c = this.state[path];
    if (!c) return;
    this.state[path] = { ...c, choice: issue, status: issue == null ? "none" : "chosen" };
    await this.rebuild();
  }

  /** The flow's progress: reading the files, their barcodes, searching or
   *  loading a series. */
  onProgress(p: { stage: string; series?: string; n?: string }) {
    if (!this.progress) return;
    const keys = { reading: "comicReading", barcodeReading: "comicBarcodeReading", barcodeLooking: "comicBarcodeLooking", searching: "comicSearching", loading: "comicLoading" } as const;
    const key = keys[p.stage as keyof typeof keys];
    if (key) this.progress = t(key, { series: p.series ?? "", n: p.n ?? "" });
  }

  /** The flow asks which series a group of comics is: the picker opens,
   *  and its answer goes back. */
  ask(id: number, q: Omit<ComicPick, "resolve" | "path" | "comic"> & { path: string | null; comic: ComicMatch | null }) {
    this.pick = {
      ...q,
      path: q.path ?? undefined,
      comic: q.comic ?? undefined,
      resolve: (v) => {
        this.pick = null;
        api.flowAnswer(id, v);
      },
    };
  }

  /** Opens the details window on the comics chosen in the list; what each
   *  starts from, the shared flow says (the command line's --edit too). */
  async openEdit() {
    if (!this.selected.length) return;
    const r = this.rule?.rule;
    const known = Object.fromEntries(this.selected.map((i) => [i.path, { current: this.matches[i.path], guess: this.state[i.path]?.own }]));
    const [paths, comics] = await api.editStart<ComicMatch>({
      kind: "comic",
      chosen: this.selected.map((i) => i.path),
      known: $state.snapshot(known),
      parts: {},
      dates: {},
      fromName: false,
      byFolder: r?.type === "comic" && !!r.seriesByFolder,
      audiobooks: false,
    });
    if (paths.length) this.edit = { paths, comics };
  }

  /** The fields changed in the details window, for each of its comics (applied as the command line's --edit does). */
  async saveEdit(changed: Record<string, string>) {
    const edit = this.edit;
    if (!edit) return;
    const comics = await api.editDetails("comic", $state.snapshot(edit.comics), changed);
    const next = { ...this.edits };
    edit.paths.forEach((p, i) => (next[p] = comics[i]));
    this.edits = next;
    this.edit = null;
  }

  /** Forgets the files taken out of the list (all, with the series
   *  picked, with null). */
  forget(paths: Set<string> | null) {
    if (!paths) {
      this.state = {};
      this.edits = {};
      this.picked = {};
      this.built = {};
      return;
    }
    this.state = without(this.state, paths);
    this.edits = without(this.edits, paths);
    this.built = without(this.built, paths);
  }

  /** The files renamed: what was found and typed follows them. */
  follow(to: (p: string) => string) {
    this.state = moved(this.state, to);
    this.edits = moved(this.edits, to);
    this.built = moved(this.built, to);
  }

  /** What was found, to put back with Ctrl+Z. */
  snapshot() {
    return { state: this.state, edits: this.edits, picked: this.picked, built: this.built };
  }
  restore(s: ReturnType<Comics["snapshot"]>) {
    this.state = s.state;
    this.edits = s.edits;
    this.picked = s.picked;
    this.built = s.built;
  }
}
