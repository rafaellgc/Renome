<script lang="ts">
  import { tick } from "svelte";
  import { diff, type Seg } from "./diff";
  import { LOCALES, t } from "./i18n";
  import { settings } from "./settings.svelte";
  import type { Copies, MovieStatus, PreviewItem, ViewSort } from "./types";

  let {
    items,
    selected = $bindable(),
    sortKey,
    descending,
    onsort,
    onoverride,
    copies = {},
    matches = {},
    onmatch,
  }: {
    items: PreviewItem[];
    selected: Set<string>;
    sortKey: ViewSort;
    descending: boolean;
    onsort: (key: Exclude<ViewSort, "manual">) => void;
    onoverride: (path: string, name: string | null) => void;
    /** Identical copies of each file in the list. */
    copies?: Record<string, Copies>;
    /** TMDB state of each file (and its tooltip), while a movie or
     *  series rule is on. */
    matches?: Record<string, { status: MovieStatus; tip: string }>;
    onmatch?: (path: string) => void;
  } = $props();

  const MOVIE_MARK: Record<MovieStatus, string> = { auto: "✓", chosen: "✓", choose: "?", none: "✕" };

  const ROW = 30;
  const OVERSCAN = 10;

  let viewport: HTMLDivElement;
  let scrollTop = $state(0);
  let height = $state(400);
  let anchor = $state<number | null>(null);
  let editing = $state<string | null>(null);
  let draft = $state("");

  const start = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  const end = $derived(Math.min(items.length, Math.ceil((scrollTop + height) / ROW) + OVERSCAN));
  const visible = $derived(items.slice(start, end));
  /** What changes in each visible row: removed (current name) and added (new name). */
  const diffs = $derived(
    new Map(visible.filter((i) => i.status !== "unchanged").map((i) => [i.path, diff(i.original, i.newName)])),
  );

  /** Scrolls to the first of the given paths (if not visible). */
  export function reveal(paths: Iterable<string>) {
    const want = new Set(paths);
    const index = items.findIndex((i) => want.has(i.path));
    if (index < 0 || !viewport) return;
    const top = index * ROW;
    if (top < viewport.scrollTop || top + ROW > viewport.scrollTop + viewport.clientHeight) {
      viewport.scrollTop = Math.max(0, top - ROW * 2);
      scrollTop = viewport.scrollTop;
    }
  }

  /** Starts editing the new name of a row (F2). */
  export async function edit(path: string) {
    const item = items.find((i) => i.path === path);
    if (!item) return;
    reveal([path]);
    await tick();
    await startEdit(item);
  }

  /** The row the arrow keys move from (the last one clicked or reached). */
  let cursor = $state<number | null>(null);

  /** Arrows move the selection (Shift extends it), Home and End go to the
   *  first and last row: the list works without a mouse. */
  function onkeys(e: KeyboardEvent) {
    if (editing || !items.length) return;
    const last = items.length - 1;
    const from = cursor ?? anchor ?? -1;
    const page = Math.max(1, Math.floor(height / ROW) - 1);
    const to = {
      ArrowDown: Math.min(last, from + 1),
      ArrowUp: Math.max(0, from - 1),
      PageDown: Math.min(last, from + page),
      PageUp: Math.max(0, from - page),
      Home: 0,
      End: last,
    }[e.key];
    if (to === undefined) return;
    e.preventDefault();
    cursor = to;
    if (e.shiftKey && anchor !== null) {
      const next = new Set<string>();
      const [a, b] = [Math.min(anchor, to), Math.max(anchor, to)];
      for (let i = a; i <= b; i++) next.add(items[i].path);
      selected = next;
    } else {
      anchor = to;
      selected = new Set([items[to].path]);
    }
    reveal([items[to].path]);
  }

  function select(e: MouseEvent, index: number) {
    cursor = index;
    const path = items[index].path;
    const next = new Set(e.ctrlKey || e.metaKey ? selected : []);
    if (e.shiftKey && anchor !== null) {
      const [a, b] = [Math.min(anchor, index), Math.max(anchor, index)];
      for (let i = a; i <= b; i++) next.add(items[i].path);
    } else if ((e.ctrlKey || e.metaKey) && next.has(path)) {
      next.delete(path);
      anchor = index;
    } else {
      next.add(path);
      anchor = index;
    }
    selected = next;
  }

  async function startEdit(item: PreviewItem) {
    editing = item.path;
    draft = item.newName;
    await tick();
    const input = viewport.querySelector<HTMLInputElement>("input.edit");
    if (input) {
      input.focus();
      // Select only the stem, without the extension.
      const dot = draft.lastIndexOf(".");
      input.setSelectionRange(0, dot > 0 ? dot : draft.length);
    }
  }

  function commit(item: PreviewItem) {
    if (editing !== item.path) return;
    editing = null;
    if (draft !== item.newName) onoverride(item.path, draft);
  }

  function onkey(e: KeyboardEvent, item: PreviewItem) {
    if (e.key === "Enter") commit(item);
    if (e.key === "Escape") editing = null;
  }

  function size(n: number) {
    if (n < 1024) return `${n} B`;
    const units = ["KB", "MB", "GB", "TB"];
    let v = n / 1024;
    let u = 0;
    while (v >= 1024 && u < units.length - 1) { v /= 1024; u++; }
    return `${v.toFixed(v < 10 ? 1 : 0)} ${units[u]}`;
  }

  const fmt = $derived(new Intl.DateTimeFormat(LOCALES[settings.lang], { dateStyle: "short", timeStyle: "short" }));
  const date = (s: number | null) => (s === null ? "" : fmt.format(new Date(s * 1000)));
  const home = /^\/(var\/)?home\/[^/]+/;
  // The column is right-to-left so long paths are cut at the start; the
  // left-to-right marks keep "~" and the slashes where they belong.
  const dir = (d: string) => `\u200e${d.replace(home, "~")}\u200e`;

  const byPath = $derived(new Map(items.map((i) => [i.path, i])));
  /** Copies of the selected files, highlighted so you can find them. */
  const twins = $derived(new Set([...selected].flatMap((p) => copies[p]?.others ?? []).filter((p) => !selected.has(p))));
  const base = (p: string) => p.split(/[\\/]/).pop() ?? p;

  function note(item: PreviewItem) {
    // Two copies of the same file heading for the same name: say so.
    if (item.problem === "duplicate" && copies[item.path]?.others.some((p) => byPath.get(p)?.newName === item.newName)) {
      return t("problem.duplicateIdentical");
    }
    if (item.problem) return t(`problem.${item.problem}`);
    if (item.windowsUnsafe && settings.warnWindows) return t("windowsUnsafe");
    return "";
  }

  const columns: { key: Exclude<ViewSort, "manual">; label: "colOriginal" | "colNew" | "colDir" | "colSize" | "colModified"; cls: string }[] = [
    { key: "name", label: "colOriginal", cls: "c-orig" },
    { key: "newName", label: "colNew", cls: "c-new" },
    { key: "dir", label: "colDir", cls: "c-dir" },
    { key: "size", label: "colSize", cls: "c-size" },
    { key: "modified", label: "colModified", cls: "c-date" },
  ];
</script>

<div class="table">
  <div class="head">
    <span class="c-status"></span>
    {#each columns as col (col.key)}
      <button class="th {col.cls}" class:active={sortKey === col.key} onclick={() => onsort(col.key)}>
        {t(col.label)}
        {#if sortKey === col.key}<span class="arrow">{descending ? "▼" : "▲"}</span>{/if}
      </button>
    {/each}
  </div>
  <div
    class="viewport"
    bind:this={viewport}
    bind:clientHeight={height}
    onscroll={() => (scrollTop = viewport.scrollTop)}
    role="grid"
    tabindex="0"
    aria-label={t("fileList")}
    aria-multiselectable="true"
    onkeydown={onkeys}
  >
    <div class="spacer" style:height="{items.length * ROW}px">
      {#each visible as item, k (item.path)}
        {@const index = start + k}
        {@const warn = item.windowsUnsafe && settings.warnWindows}
        {@const d = diffs.get(item.path)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="row {item.status}"
          class:sel={selected.has(item.path)}
          class:twin={twins.has(item.path)}
          style:top="{index * ROW}px"
          role="row"
          tabindex="-1"
          onclick={(e) => select(e, index)}
        >
          <span class="c-status" title={note(item)}>
            {#if item.status === "conflict"}<span class="dot bad">!</span>
            {:else if warn}<span class="dot warn">!</span>
            {:else if item.status === "ok"}<span class="dot good">●</span>{/if}
          </span>
          <span class="c-orig" title={item.path}>
            {#if copies[item.path]}{@const c = copies[item.path]}{@const same = t("identicalTo", { names: c.others.map(base).join(", ") })}<span class="copy g{c.group % 6}" title={same} role="img" aria-label={same}><svg viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M5 1.5h7A1.5 1.5 0 0 1 13.5 3v8.5H12V3H5zM2.5 4.5A1.5 1.5 0 0 1 4 3h6a1.5 1.5 0 0 1 1.5 1.5v9A1.5 1.5 0 0 1 10 15H4a1.5 1.5 0 0 1-1.5-1.5zM4 4.5v9h6v-9z" /></svg>{c.group + 1}</span>{/if}{#if item.isDir}<svg class="folder" viewBox="0 0 16 16" aria-label={t("isFolder")}><path fill="currentColor" d="M1.5 3.5A1 1 0 0 1 2.5 2.5h3.6l1.5 1.5h5.9a1 1 0 0 1 1 1v7.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z" /></svg>{/if}{#if d}{@render segs(d[0], "del")}{:else}{item.original}{/if}
          </span>
          <span class="c-new" role="gridcell" tabindex="-1" ondblclick={() => startEdit(item)} title={note(item) || t("editHint")}>
            {#if editing === item.path}
              <input
                class="edit"
                bind:value={draft}
                spellcheck="false"
                onkeydown={(e) => onkey(e, item)}
                onblur={() => commit(item)}
              />
            {:else}
              {#if matches[item.path]}
                {@const m = matches[item.path]}
                <button
                  class="movie {m.status}"
                  title={m.tip}
                  aria-label={m.tip}
                  onclick={(e) => { e.stopPropagation(); onmatch?.(item.path); }}
                >{MOVIE_MARK[m.status]}</button>
              {/if}
              {#if item.folder}<span class="into" title={t("intoFolder", { folder: item.folder })}><svg class="folder" viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M1.5 3.5A1 1 0 0 1 2.5 2.5h3.6l1.5 1.5h5.9a1 1 0 0 1 1 1v7.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z" /></svg>/</span>{/if}
              <span class="name" class:same={item.status === "unchanged"}>{#if d}{@render segs(d[1], "ins")}{:else}{item.newName}{/if}</span>
              {#if item.manual}
                <button class="icon reset" title={t("resetManual")} onclick={(e) => { e.stopPropagation(); onoverride(item.path, null); }}>↺</button>
              {/if}
            {/if}
          </span>
          <span class="c-dir" title={item.dir}>{dir(item.dir)}</span>
          <span class="c-size">{item.isDir ? "" : size(item.size)}</span>
          <span class="c-date">{date(item.modified)}</span>
        </div>
      {/each}
    </div>
  </div>
</div>

{#snippet segs(list: Seg[], cls: string)}{#each list as seg, i (i)}{#if seg.changed}<mark class={cls} class:mov={seg.moved}>{seg.text}</mark>{:else}{seg.text}{/if}{/each}{/snippet}

<style>
  .table {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    overflow: hidden;
  }
  .head, .row {
    display: grid;
    grid-template-columns: 28px minmax(140px, 1fr) minmax(160px, 1.2fr) minmax(90px, 0.6fr) 76px 120px;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
  }
  .head {
    height: 32px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
  }
  .th {
    height: 26px;
    padding: 0 4px;
    margin: 0 -4px;
    border: none;
    background: none;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Narrow windows: size and date give way to the names. */
  @media (max-width: 1000px) {
    .head, .row { grid-template-columns: 28px minmax(120px, 1fr) minmax(140px, 1.2fr) minmax(80px, 0.6fr); }
    .c-size, .c-date { display: none; }
  }
  .th:hover:not(:disabled) { background: var(--hover); color: var(--text); }
  .th.active { color: var(--accent); }
  .th.c-size, .th.c-date { text-align: right; }
  .arrow { font-size: 9px; margin-left: 2px; }
  .viewport { flex: 1; overflow: auto; position: relative; }
  .viewport:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .spacer { position: relative; }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 30px;
    font-size: 13px;
    cursor: default;
    border-bottom: 1px solid var(--row-line);
  }
  .row:hover { background: var(--hover); }
  .row.sel { background: var(--selected); }
  .row span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .c-size, .c-date { text-align: right; color: var(--muted); font-variant-numeric: tabular-nums; }
  .row .c-dir { color: var(--muted); direction: rtl; text-align: left; }
  .c-new { display: flex; align-items: center; gap: 4px; }
  .name { overflow: hidden; text-overflow: ellipsis; }
  .row.conflict .name { color: var(--danger); }
  mark { color: inherit; background: none; border-radius: 3px; }
  mark.ins { color: var(--accent); background: var(--accent-soft); font-weight: 600; }
  .row.conflict mark.ins { color: var(--danger); background: color-mix(in srgb, var(--danger) 16%, transparent); }
  mark.del { color: var(--danger); text-decoration: line-through; text-decoration-thickness: 1px; }
  /* Only moved: the same words are on the other side. */
  mark.mov, .row.conflict mark.ins.mov { color: inherit; background: none; font-weight: inherit; text-decoration: underline dotted; text-decoration-thickness: 1px; text-underline-offset: 3px; }
  /* Copy badge: icon + group number; the color repeats every 6 groups. */
  .copy {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    margin-right: 6px;
    padding: 0 5px 0 4px;
    border-radius: 9px;
    font-size: 11px;
    font-weight: 700;
    line-height: 17px;
    vertical-align: 1px;
    color: var(--g);
    background: color-mix(in srgb, var(--g) 15%, transparent);
  }
  .copy svg { width: 11px; height: 11px; }
  .g0 { --g: #2f6fde; }
  .g1 { --g: #138a5e; }
  .g2 { --g: #c26100; }
  .g3 { --g: #8a4fd6; }
  .g4 { --g: #c0397a; }
  .g5 { --g: #0e8aa3; }
  :global(:root[data-theme="dark"]) .copy { filter: brightness(1.45); }
  .row.twin { background: color-mix(in srgb, var(--accent) 9%, transparent); box-shadow: inset 3px 0 var(--accent); }
  .folder { width: 14px; height: 14px; margin-right: 5px; vertical-align: -2px; color: var(--muted); }
  .name.same { color: var(--muted); }
  .edit { width: 100%; height: 24px; padding: 0 6px; }
  .reset { height: 20px; width: 20px; font-size: 13px; }
  .into { flex: none; display: inline-flex; align-items: center; color: var(--accent); font-weight: 600; }
  .into .folder { margin-right: 1px; color: var(--accent); }
  .movie {
    flex: none;
    width: 18px;
    height: 18px;
    padding: 0;
    border: none;
    border-radius: 50%;
    font-size: 11px;
    font-weight: 700;
    line-height: 18px;
  }
  .movie.auto, .movie.chosen { background: color-mix(in srgb, #1a9a5c 18%, transparent); color: #138a4e; }
  .movie.choose { background: var(--warn); color: black; }
  .movie.none { background: var(--hover); color: var(--muted); }
  .dot { display: inline-grid; place-items: center; width: 16px; height: 16px; border-radius: 50%; font-size: 10px; font-weight: 700; }
  .dot.good { color: var(--accent); }
  .dot.bad { background: var(--danger); color: white; }
  .dot.warn { background: var(--warn); color: black; }
</style>
