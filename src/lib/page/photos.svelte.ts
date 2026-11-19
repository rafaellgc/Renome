// The photo rule's side of the main page: dates typed in by hand for the
// photos and videos whose own date is wrong or missing (a picture sent
// through a messaging app keeps only the day it arrived).

import { api } from "$lib/api";
import { notify } from "$lib/dialogs.svelte";
import { t } from "$lib/i18n";
import type { EditField } from "$lib/types";
import { moved, ruleOf, without, type PageCtx } from "./common";

type Parts = { day: string; month: string; year: string; hour: string; minute: string };
const NONE: Parts = { day: "", month: "", year: "", hour: "", minute: "" };

/** `2018-03-27` or `2018-03-27 14:05` in its parts. */
function parts(text: string | undefined): Parts {
  const m = /^(\d{4})-(\d{2})-(\d{2})(?: (\d{2}):(\d{2}))?$/.exec(text ?? "");
  return m ? { year: m[1], month: m[2], day: m[3], hour: m[4] ?? "", minute: m[5] ?? "" } : { ...NONE };
}

export class Photos {
  #ctx: PageCtx;
  /** Dates typed in, by path (`2018-03-27`, `2018-03-27 14:05`). */
  dates = $state<Record<string, string>>({});
  /** The date window, while open. */
  edit = $state<{ paths: string[]; rows: Parts[] } | null>(null);

  constructor(ctx: PageCtx) {
    this.#ctx = ctx;
  }

  rule = $derived.by(() => ruleOf(this.#ctx.steps, "photo"));
  /** The photos and videos among the rows chosen in the list (those the
   *  rule reads). */
  selected = $derived.by(() => this.#ctx.items.filter((i) => this.#ctx.selected.has(i.path) && i.photoDate !== undefined));
  /** The date window's fields. */
  fields = $derived<EditField[]>([
    { key: "day", label: t("photoEditDay"), numeric: true },
    { key: "month", label: t("photoEditMonth"), numeric: true },
    { key: "year", label: t("photoEditYear"), numeric: true },
    { key: "hour", label: t("photoEditHour"), numeric: true },
    { key: "minute", label: t("photoEditMinute"), numeric: true },
  ]);

  /** Opens the date window on the photos chosen in the list. */
  openEdit() {
    const rows = this.selected;
    if (!rows.length) return;
    this.edit = { paths: rows.map((i) => i.path), rows: rows.map((i) => parts(this.dates[i.path])) };
  }

  /** The fields changed in the date window, for each of its photos (checked
   *  as the command line's --edit does); the window stays open when they
   *  make no date. */
  async saveEdit(changed: Record<string, string>) {
    const edit = this.edit;
    if (!edit) return;
    const dates = await api.editDates(edit.paths.map((p) => this.dates[p] ?? ""), changed);
    if (!dates) {
      await notify({ title: t("rule.photo"), text: t("photoEditInvalid") });
      return;
    }
    const next = { ...this.dates };
    edit.paths.forEach((p, i) => {
      if (dates[i]) next[p] = dates[i];
      else delete next[p];
    });
    this.dates = next;
    this.edit = null;
  }

  /** Forgets the files taken out of the list (all, with null). */
  forget(paths: Set<string> | null) {
    this.dates = paths ? without(this.dates, paths) : {};
  }

  /** The files renamed: their dates follow them. */
  follow(to: (p: string) => string) {
    this.dates = moved(this.dates, to);
  }

  /** What was typed, to put back with Ctrl+Z. */
  snapshot() {
    return { dates: this.dates };
  }
  restore(s: ReturnType<Photos["snapshot"]>) {
    this.dates = s.dates;
  }
}
