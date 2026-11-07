// The photo rule's side of the main page: dates typed in by hand for the
// photos and videos whose own date is wrong or missing (a picture sent
// through a messaging app keeps only the day it arrived).

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

/** The parts as the core takes them; "" when all are empty; null when
 *  they make no date. */
function joined(p: Parts): string | null {
  if (Object.values(p).every((v) => !v)) return "";
  const [y, mo, d] = [Number(p.year), Number(p.month), Number(p.day)];
  const date = new Date(y, mo - 1, d);
  if (p.year.length !== 4 || y < 1900 || date.getFullYear() !== y || date.getMonth() !== mo - 1 || date.getDate() !== d) return null;
  const pad = (n: number) => String(n).padStart(2, "0");
  const day = `${y}-${pad(mo)}-${pad(d)}`;
  if (!p.hour && !p.minute) return day;
  const [h, mi] = [Number(p.hour || 0), Number(p.minute || 0)];
  if (h > 23 || mi > 59) return null;
  return `${day} ${pad(h)}:${pad(mi)}`;
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

  /** The fields changed in the date window, for each of its photos; the
   *  window stays open when they make no date. */
  async saveEdit(changed: Record<string, string>) {
    const edit = this.edit;
    if (!edit) return;
    const next = { ...this.dates };
    for (const [i, p] of edit.paths.entries()) {
      const text = joined({ ...edit.rows[i], ...changed });
      if (text === null) {
        await notify({ title: t("rule.photo"), text: t("photoEditInvalid") });
        return;
      }
      if (text) next[p] = text;
      else delete next[p];
    }
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
