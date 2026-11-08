<script lang="ts">
  import { tick } from "svelte";
  import { api } from "./api";
  import { COL_LABEL, detailText } from "./details";
  import { diff, type Seg } from "./diff";
  import { LOCALES, t } from "./i18n";
  import { interleave, type Ghost, type Row } from "./missing";
  import { saveSettings, settings } from "./settings.svelte";
  import { DETAIL_COLUMNS, type Copies, type DetailColumn, type Details, type MovieStatus, type PreviewItem, type ViewSort } from "./types";

  let {
    items,
    selected = $bindable(),
    sortKey,
    descending,
    onsort,
    onoverride,
    copies = {},
    alike = {},
    similar = {},
    oncompare,
    matches = {},
    onmatch,
    ghosts = [],
    outside = {},
    available = [],
  }: {
    items: PreviewItem[];
    selected: Set<string>;
    sortKey: ViewSort;
    descending: boolean;
    onsort: (key: Exclude<ViewSort, "manual">) => void;
    onoverride: (path: string, name: string | null) => void;
    /** Identical copies of each file in the list. */
    copies?: Record<string, Copies>;
    /** Songs that are the same music as others in the list. */
    alike?: Record<string, Copies>;
    /** Pictures that look like others in the list. */
    similar?: Record<string, Copies>;
    /** Opens the comparison window at a file's group. */
    oncompare?: (kind: "similar" | "identical", path: string) => void;
    /** TMDB state of each file (and its tooltip), while a movie or
     *  series rule is on. */
    matches?: Record<string, { status: MovieStatus; tip: string }>;
    onmatch?: (path: string) => void;
    /** The optional columns that make sense with the rules in use. */
    available?: DetailColumn[];
    /** Missing episodes, shown faded between the files. */
    ghosts?: Ghost[];
    /** Files the lookup rules leave alone, with why. */
    outside?: Record<string, string>;
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

  /** Files and, between them, the missing episodes (which the selection
   *  and the arrow keys skip). */
  const rows = $derived(interleave(items, ghosts, descending));
  const isGhost = (r: Row): r is Ghost => "ghost" in r;
  const TIPS = {
    episode: "missingTip",
    issue: "missingIssueTip",
    track: "missingTrackTip",
    movie: "missingMovieTip",
    book: "missingBookTip",
  } as const;
  const keyOf = (r: Row) => (isGhost(r) ? r.key : r.path);

  const start = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  const end = $derived(Math.min(rows.length, Math.ceil((scrollTop + height) / ROW) + OVERSCAN));
  const visible = $derived(rows.slice(start, end));
  /** What changes in each visible row: removed (current name) and added (new name). */
  const diffs = $derived(
    new Map(
      visible
        .filter((i): i is PreviewItem => !isGhost(i) && i.status !== "unchanged")
        .map((i) => [i.path, diff(i.original, i.newName)]),
    ),
  );

  /** Scrolls to the first of the given paths (if not visible). */
  export function reveal(paths: Iterable<string>) {
    const want = new Set(paths);
    const index = rows.findIndex((i) => !isGhost(i) && want.has(i.path));
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

  /** The file row nearest to `index` going `dir` (1 down, -1 up), past
   *  missing episodes; the other way when there is none. */
  function fileRow(index: number, dir: 1 | -1): number {
    for (const d of [dir, -dir]) {
      for (let i = index; i >= 0 && i < rows.length; i += d) if (!isGhost(rows[i])) return i;
    }
    return -1;
  }

  /** The files between two rows, both included. */
  function range(from: number, to: number): string[] {
    const out: string[] = [];
    for (let i = Math.min(from, to); i <= Math.max(from, to); i++) {
      const r = rows[i];
      if (!isGhost(r)) out.push(r.path);
    }
    return out;
  }

  /** Arrows move the selection (Shift extends it), Home and End go to the
   *  first and last row: the list works without a mouse. */
  function onkeys(e: KeyboardEvent) {
    if (editing || !items.length) return;
    const last = rows.length - 1;
    const from = cursor ?? anchor ?? -1;
    const page = Math.max(1, Math.floor(height / ROW) - 1);
    const move: Record<string, [number, 1 | -1]> = {
      ArrowDown: [Math.min(last, from + 1), 1],
      ArrowUp: [Math.max(0, from - 1), -1],
      PageDown: [Math.min(last, from + page), 1],
      PageUp: [Math.max(0, from - page), -1],
      Home: [0, 1],
      End: [last, -1],
    };
    if (!(e.key in move)) return;
    e.preventDefault();
    const to = fileRow(...move[e.key]);
    if (to < 0) return;
    const path = (rows[to] as PreviewItem).path;
    cursor = to;
    if (e.shiftKey && anchor !== null) {
      selected = new Set(range(anchor, to));
    } else {
      anchor = to;
      selected = new Set([path]);
    }
    reveal([path]);
  }

  function select(e: MouseEvent, index: number) {
    const row = rows[index];
    if (isGhost(row)) return;
    cursor = index;
    const path = row.path;
    const next = new Set(e.ctrlKey || e.metaKey ? selected : []);
    if (e.shiftKey && anchor !== null) {
      for (const p of range(anchor, index)) next.add(p);
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
  const dayFmt = $derived(new Intl.DateTimeFormat(LOCALES[settings.lang], { dateStyle: "short", timeZone: "UTC" }));
  /** An air or cover date (YYYY-MM-DD) in the user's format. */
  const day = (s: string) => {
    const d = new Date(`${s}T00:00:00Z`);
    return Number.isNaN(d.getTime()) ? s : dayFmt.format(d);
  };
  const home = /^\/(var\/)?home\/[^/]+/;
  // The column is right-to-left so long paths are cut at the start; the
  // left-to-right marks keep "~" and the slashes where they belong.
  const dir = (d: string) => `\u200e${d.replace(home, "~")}\u200e`;

  const byPath = $derived(new Map(items.map((i) => [i.path, i])));
  /** Copies of the selected files, highlighted so you can find them. */
  const twins = $derived(
    new Set([...selected].flatMap((p) => [...(copies[p]?.others ?? []), ...(alike[p]?.others ?? []), ...(similar[p]?.others ?? [])]).filter((p) => !selected.has(p))),
  );
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

  /** Columns of numbers, narrow and aligned to the right. */
  const NARROW: DetailColumn[] = ["track", "year", "length"];
  const WIDTH: Record<DetailColumn, string> = {
    artist: "minmax(90px, 0.8fr)", album: "minmax(90px, 0.8fr)", title: "minmax(90px, 0.8fr)", genre: "minmax(70px, 0.5fr)",
    track: "48px", year: "48px", length: "56px", quality: "112px", resolution: "64px", codec: "64px", taken: "120px",
  };
  const extra = $derived(DETAIL_COLUMNS.filter((c) => available.includes(c) && settings.columns.includes(c)));
  $effect(() => {
    if (!available.length) columnsMenu = false;
  });
  const extraWidths = $derived(extra.map((c) => WIDTH[c]).join(" "));

  type Col = { key: Exclude<ViewSort, "manual">; label: Parameters<typeof t>[0]; cls: string };
  const columns = $derived<Col[]>([
    { key: "name", label: "colOriginal", cls: "c-orig" },
    { key: "newName", label: "colNew", cls: "c-new" },
    { key: "dir", label: "colDir", cls: "c-dir" },
    ...extra.map((c) => ({ key: c, label: COL_LABEL[c], cls: `c-x${NARROW.includes(c) ? " num" : ""}` })),
    { key: "size", label: "colSize", cls: "c-size" },
    { key: "modified", label: "colModified", cls: "c-date" },
  ]);

  let columnsMenu = $state(false);
  let headWrap = $state<HTMLDivElement>();
  let winWidth = $state(1200);
  const MIN: Record<DetailColumn, number> = {
    artist: 90, album: 90, title: 90, genre: 70, track: 48, year: 48, length: 56, quality: 112, resolution: 64, codec: 64, taken: 120,
  };
  /** The narrowest the columns go: past it, the list scrolls sideways. */
  const minWidth = $derived.by(() => {
    if (!extra.length) return 0;
    const wide = winWidth > 1000;
    const base = wide ? 28 + 140 + 160 + 90 + 76 + 120 : 28 + 120 + 140 + 80;
    const count = (wide ? 6 : 4) + extra.length;
    return base + extra.reduce((n, c) => n + MIN[c], 0) + 8 * (count - 1) + 20;
  });
  function toggleColumn(c: DetailColumn) {
    settings.columns = settings.columns.includes(c) ? settings.columns.filter((x) => x !== c) : [...settings.columns, c];
    saveSettings();
  }

  /** What the optional columns show, read for the rows in view; read
   *  again when the file changed (its size or date). */
  let details = $state<Record<string, Details>>({});
  const readAs: Record<string, string> = {};
  const stampOf = (i: PreviewItem) => `${i.size}:${i.modified}`;
  let detailTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    if (!extra.length) return;
    const want = visible.filter((r): r is PreviewItem => !isGhost(r) && !r.isDir && readAs[r.path] !== stampOf(r));
    if (!want.length) return;
    clearTimeout(detailTimer);
    detailTimer = setTimeout(async () => {
      for (const i of want) readAs[i.path] = stampOf(i);
      try {
        const got = await api.fileDetails(want.map((i) => i.path));
        details = { ...details, ...got };
      } catch {
        for (const i of want) delete readAs[i.path];
      }
    }, 60);
  });
</script>

<svelte:window bind:innerWidth={winWidth} onkeydown={(e) => { if (columnsMenu && e.key === "Escape") { e.stopPropagation(); columnsMenu = false; } }} />

<div class="table" style:--extra={extraWidths || undefined}>
  {#if columnsMenu}
    <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
    <div class="cols-back" onclick={() => (columnsMenu = false)}></div>
    <div class="cols-menu" role="group" aria-label={t("columnsMenu")}>
      <p class="cols-title">{t("columnsMenu")}</p>
      {#each DETAIL_COLUMNS.filter((c) => available.includes(c)) as c (c)}
        <label class="cols-item">
          <input type="checkbox" checked={settings.columns.includes(c)} onchange={() => toggleColumn(c)} />
          {t(COL_LABEL[c])}
        </label>
      {/each}
      <p class="cols-hint">{t("columnsHint")}</p>
    </div>
  {/if}
  <!-- Scrolled sideways with the list when the columns do not fit. -->
  <div class="head-wrap" bind:this={headWrap}>
  <div class="head" style:min-width={minWidth ? `${minWidth}px` : undefined}>
    <span class="c-status">
      {#if available.length}
      <button class="icon cols" class:on={extra.length > 0} title={t("columnsMenu")} aria-label={t("columnsMenu")} aria-expanded={columnsMenu} onclick={() => (columnsMenu = !columnsMenu)}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M2 3h12v1.5H2zm0 4.25h12v1.5H2zM2 11.5h12V13H2z" /></svg>
      </button>
      {/if}
    </span>
    {#each columns as col (col.key)}
      <button class="th {col.cls}" class:active={sortKey === col.key} onclick={() => onsort(col.key)}>
        {t(col.label)}
        {#if sortKey === col.key}<span class="arrow">{descending ? "▼" : "▲"}</span>{/if}
      </button>
    {/each}
  </div>
  </div>
  <div
    class="viewport"
    bind:this={viewport}
    bind:clientHeight={height}
    onscroll={() => {
      scrollTop = viewport.scrollTop;
      if (headWrap) headWrap.scrollLeft = viewport.scrollLeft;
    }}
    role="grid"
    tabindex="0"
    aria-label={t("fileList")}
    aria-multiselectable="true"
    onkeydown={onkeys}
  >
    <div class="spacer" style:height="{rows.length * ROW}px" style:min-width={minWidth ? `${minWidth}px` : undefined}>
      {#each visible as item, k (keyOf(item))}
        {@const index = start + k}
        {#if isGhost(item)}
          {@const tip = `${t(TIPS[item.kind], { show: item.group })}${item.date ? ` · ${day(item.date)}` : ""}`}
          <div class="row ghost" style:top="{index * ROW}px" role="row" aria-disabled="true" title={tip}>
            <span class="c-status"><span class="dot hole" aria-hidden="true">○</span></span>
            <span class="c-orig"><span class="missing">{t("missingRow")}</span>{item.group}</span>
            <span class="c-new"><span class="name">{item.code}{item.title ? ` · ${item.title}` : ""}</span></span>
            <span class="c-dir"></span>
            {#each extra as c (c)}<span class="c-x"></span>{/each}
            <span class="c-size"></span>
            <span class="c-date">{item.date ? day(item.date) : ""}</span>
          </div>
        {:else}
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
            {#if copies[item.path]}{@const c = copies[item.path]}{@const same = t("identicalTo", { names: c.others.map(base).join(", ") })}<!-- svelte-ignore a11y_no_static_element_interactions --><span class="copy g{c.group % 6}" title={`${same}\n${t("compareDbl")}`} role="img" aria-label={same} ondblclick={() => oncompare?.("identical", item.path)}><svg viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M5 1.5h7A1.5 1.5 0 0 1 13.5 3v8.5H12V3H5zM2.5 4.5A1.5 1.5 0 0 1 4 3h6a1.5 1.5 0 0 1 1.5 1.5v9A1.5 1.5 0 0 1 10 15H4a1.5 1.5 0 0 1-1.5-1.5zM4 4.5v9h6v-9z" /></svg>{c.group + 1}</span>{/if}{#if alike[item.path]}{@const s = alike[item.path]}{@const same = t("sameSongAs", { names: s.others.map(base).join(", ") })}<span class="copy song g{s.group % 6}" title={same} role="img" aria-label={same}>♪{s.group + 1}</span>{/if}{#if similar[item.path]}{@const s = similar[item.path]}{@const same = t("looksLike", { names: s.others.map(base).join(", ") })}<!-- svelte-ignore a11y_no_static_element_interactions --><span class="copy song g{s.group % 6}" title={`${same}\n${t("compareDbl")}`} role="img" aria-label={same} ondblclick={() => oncompare?.("similar", item.path)}><svg viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" fill-rule="evenodd" d="M2.5 2h11A1.5 1.5 0 0 1 15 3.5v9a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9A1.5 1.5 0 0 1 2.5 2zm0 1.5v7.3l3.2-3.2 2.6 2.6 1.9-1.9 3.3 3.3V3.5zM10.8 4.6a1.4 1.4 0 1 1 0 2.8 1.4 1.4 0 0 1 0-2.8z" /></svg>{s.group + 1}</span>{/if}{#if outside[item.path]}<span class="outside" title={outside[item.path]}>{t("outsideRow")}</span>{/if}{#if item.drawn}<span class="outside" title={t("drawnRowTip", { n: item.drawn })}>{t("drawnRow", { n: item.drawn })}</span>{/if}{#if item.drm}<span class="outside" title={t("drmRowTip")}>{t("drmRow")}</span>{/if}{#if item.subFix}{@const f = item.subFix}{#if f.noFps}<span class="outside" title={t("subNoFpsTip")}>{t("subNoFpsRow")}</span>{:else}<span class="outside" title={[f.convert ? t("subTipConvert") : "", f.encoding ? t("subTipEncoding", { enc: f.encoding }) : "", f.ads ? t("subTipAds", { n: f.ads }) : "", f.hearing ? t("subTipHearing", { n: f.hearing }) : "", f.retime ? t("subTipRetime") : ""].filter(Boolean).join("\n")}>{[f.convert ? t("subRowSrt") : "", f.encoding ? "UTF-8" : "", f.ads ? t("subRowAds", { n: f.ads }) : "", f.hearing ? t("subRowHearing", { n: f.hearing }) : "", f.retime ? t("subRowRetime") : ""].filter(Boolean).join(" · ")}</span>{/if}{/if}{#if item.subExtract}<span class="outside" title={t("subExtractTip", { n: item.subExtract })}>{t("subExtractRow", { n: item.subExtract })}</span>{/if}{#if item.subPictures}<span class="outside" title={t("subPicturesTip", { n: item.subPictures })}>{t("subPicturesRow", { n: item.subPictures })}</span>{/if}{#if item.isDir}<svg class="folder" viewBox="0 0 16 16" aria-label={t("isFolder")}><path fill="currentColor" d="M1.5 3.5A1 1 0 0 1 2.5 2.5h3.6l1.5 1.5h5.9a1 1 0 0 1 1 1v7.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z" /></svg>{/if}{#if d}{@render segs(d[0], "del")}{:else}{item.original}{/if}
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
          <!-- Moving: where to, without hovering over the folder icon. -->
          {#if item.folder}
            <span class="c-dir into-dir" title={item.dest ?? item.folder}><span class="arrow">→</span><span class="path">{item.dest ? dir(item.dest) : item.folder}</span></span>
          {:else}
            <span class="c-dir" title={item.dir}>{dir(item.dir)}</span>
          {/if}
          {#each extra as c (c)}
            {@const v = detailText(c, details[item.path])}
            <span class="c-x" class:num={NARROW.includes(c)} title={v}>{v}</span>
          {/each}
          <span class="c-size">{item.isDir ? "" : size(item.size)}</span>
          <span class="c-date">{date(item.modified)}</span>
        </div>
        {/if}
      {/each}
    </div>
  </div>
</div>

{#snippet segs(list: Seg[], cls: string)}{#each list as seg, i (i)}{#if seg.changed}<mark class={cls} class:mov={seg.moved}>{seg.text}</mark>{:else}{seg.text}{/if}{/each}{/snippet}

<style>
  .table {
    position: relative;
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
    grid-template-columns: 28px minmax(140px, 1fr) minmax(160px, 1.2fr) minmax(90px, 0.8fr) var(--extra, ) 76px 120px;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
  }
  .head { height: 32px; }
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
    .head, .row { grid-template-columns: 28px minmax(120px, 1fr) minmax(140px, 1.2fr) minmax(80px, 0.6fr) var(--extra, ); }
    .c-size, .c-date { display: none; }
  }
  .th:hover:not(:disabled) { background: var(--hover); color: var(--text); }
  .th.active { color: var(--accent); }
  .th.c-size, .th.c-date, .th.num { text-align: right; }
  .row .c-x { color: var(--muted); }
  .row .c-x.num { text-align: right; font-variant-numeric: tabular-nums; }
  .head-wrap { flex: none; overflow: hidden; border-bottom: 1px solid var(--border); background: var(--surface-2); }
  .cols { width: 22px; height: 22px; padding: 3px; color: var(--muted); }
  .cols.on { color: var(--accent); }
  .cols svg { width: 14px; height: 14px; display: block; }
  .cols-back { position: fixed; inset: 0; z-index: 20; }
  .cols-menu {
    position: absolute;
    top: 30px;
    left: 4px;
    z-index: 21;
    min-width: 200px;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    box-shadow: 0 6px 24px rgb(0 0 0 / 0.18);
  }
  .cols-title { margin: 2px 6px 6px; font-size: 12px; font-weight: 600; color: var(--muted); }
  .cols-item { display: flex; align-items: center; gap: 8px; padding: 4px 6px; border-radius: 5px; font-size: 13px; }
  .cols-item:hover { background: var(--hover); }
  .cols-hint { max-width: 220px; margin: 6px 6px 2px; font-size: 11px; color: var(--muted); }
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
  /* A missing episode: faded, only there to be seen. */
  .row.ghost { color: var(--muted); opacity: 0.6; font-style: italic; }
  .row.ghost:hover { background: none; }
  .missing {
    margin-right: 6px;
    padding: 0 6px;
    border: 1px dashed currentColor;
    border-radius: 9px;
    font-size: 11px;
    font-style: normal;
  }
  /* Left alone by the lookup rules: the row stays as it is. */
  .outside {
    margin-right: 6px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: 9px;
    color: var(--muted);
    font-size: 11px;
  }
  .dot.hole { color: var(--muted); }
  .row span { overflow: hidden; text-overflow: ellipsis; white-space: pre; }
  .c-size, .c-date { text-align: right; color: var(--muted); font-variant-numeric: tabular-nums; }
  .row .c-dir { color: var(--muted); direction: rtl; text-align: left; }
  /* Where it goes: the arrow stays, the path is cut at its start, like
     the others' (its end, the new folders, is what tells). */
  .row .c-dir.into-dir { color: var(--accent); direction: ltr; display: flex; gap: 4px; }
  .into-dir .arrow { flex: none; }
  .into-dir .path { flex: 1; min-width: 0; direction: rtl; text-align: left; }
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
  /* The same song in another file: outlined, to tell it from a copy. */
  .copy.song { background: none; box-shadow: inset 0 0 0 1px var(--g); }
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
