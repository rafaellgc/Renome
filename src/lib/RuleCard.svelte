<script lang="ts">
  import { t } from "./i18n";
  import { splitExt, splitParts } from "./rules";
  import type { Step } from "./types";

  let {
    step = $bindable(),
    error = null,
    sample = "",
    first = false,
    last = false,
    onmove,
    onremove,
    lookup,
    summary = null,
  }: {
    step: Step;
    error?: string | null;
    /** A file name from the list, to show examples. */
    sample?: string;
    first?: boolean;
    last?: boolean;
    onmove: (delta: -1 | 1) => void;
    onremove: () => void;
    /** A line about the rule's effect (songs without tags). */
    summary?: string | null;
    /** Movie and series rules: search state and actions. */
    lookup?: {
      /** The show found last (series rule). */
      show?: string | null;
      progress: string | null;
      found: number;
      choose: number;
      none: number;
      onsearch: () => void;
      onreview: () => void;
    };
  } = $props();

  let open = $state(true);
  const r = $derived(step.rule);

  const parts = $derived.by(() => {
    if (r.type !== "rearrange" || !sample) return [];
    const [stem, ext] = splitExt(sample);
    const text = r.target === "name" ? stem : r.target === "ext" ? ext : sample;
    return splitParts(text, r.separator);
  });

  /** Appends a part number to the order (clicking a part). */
  function addPart(n: number) {
    if (r.type !== "rearrange") return;
    const sep = r.separator.trim() ? r.separator : " ";
    r.order = r.order.trim() ? `${r.order}${sep}${n}` : `${n}`;
  }
</script>

<article class="card" class:off={!step.enabled} class:has-error={!!error}>
  <header>
    <input type="checkbox" bind:checked={step.enabled} title={t("toggleRule")} />
    <button class="title" onclick={() => (open = !open)} aria-expanded={open}>
      <span class="chev" class:closed={!open}>▾</span>
      {t(`rule.${r.type}`)}
    </button>
    <div class="actions">
      <button class="icon" disabled={first} onclick={() => onmove(-1)} title={t("up")}>↑</button>
      <button class="icon" disabled={last} onclick={() => onmove(1)} title={t("down")}>↓</button>
      <button class="icon danger" onclick={onremove} title={t("removeRule")}>✕</button>
    </div>
  </header>

  {#if open}
    <div class="body">
      {#if r.type === "replace"}
        <label>{t("find")} <input bind:value={r.find} spellcheck="false" /></label>
        <label>{t("replaceWith")} <input bind:value={r.replace} spellcheck="false" /></label>
        <div class="row">
          <label class="check"><input type="checkbox" bind:checked={r.caseSensitive} /> {t("caseSensitive")}</label>
          <label class="check" title={t("firstOnlyHint")}><input type="checkbox" bind:checked={r.firstOnly} /> {t("firstOnly")}</label>
        </div>
        {@render target()}
      {:else if r.type === "regex"}
        <label>{t("expression")} <input class="mono" bind:value={r.pattern} spellcheck="false" placeholder={"(\\d+) - (.*)"} /></label>
        <label>{t("replaceWith")} <input class="mono" bind:value={r.replace} spellcheck="false" placeholder="$2 ($1)" /></label>
        <div class="row">
          <label class="check"><input type="checkbox" bind:checked={r.caseSensitive} /> {t("caseSensitive")}</label>
          <label class="check" title={t("firstOnlyHint")}><input type="checkbox" bind:checked={r.firstOnly} /> {t("firstOnly")}</label>
        </div>
        {@render target()}
        <p class="hint mono">{t("groupsHint")}</p>
      {:else if r.type === "rearrange"}
        <label>{t("separator")} <input class="mono sep" bind:value={r.separator} spellcheck="false" placeholder=" - " /></label>
        {#if parts.length}
          <div class="parts">
            <span class="hint">{t("partsOfSample")}</span>
            {#each parts as part, i (i)}
              <button class="part" title={t("addPartHint")} onclick={() => addPart(i + 1)}>
                <b>{i + 1}</b><span>{part}</span>
              </button>
            {/each}
          </div>
        {:else if sample}
          <p class="hint">{t("noParts")}</p>
        {/if}
        <label>{t("newOrder")} <input class="mono" bind:value={r.order} spellcheck="false" placeholder="1 - 3 - 2" /></label>
        <p class="hint">{t("newOrderHint")}</p>
        {@render target()}
      {:else if r.type === "case"}
        <label>{t("mode")}
          <select bind:value={r.mode}>
            <option value="lower">{t("caseLower")}</option>
            <option value="upper">{t("caseUpper")}</option>
            <option value="sentence">{t("caseSentence")}</option>
            <option value="title">{t("caseTitle")}</option>
            <option value="titleSmart">{t("caseTitleSmart")}</option>
          </select>
        </label>
        {@render target()}
      {:else if r.type === "insert"}
        <label>{t("what")}
          <select bind:value={r.source}>
            <option value="text">{t("srcText")}</option>
            <option value="now">{t("srcNow")}</option>
            <option value="modified">{t("srcModified")}</option>
            <option value="taken">{t("srcTaken")}</option>
            <option value="folder">{t("srcFolder")}</option>
          </select>
        </label>
        {#if r.source === "text"}
          <label>{t("text")} <input bind:value={r.text} spellcheck="false" /></label>
        {:else if r.source !== "folder"}
          <label>{t("format")} <input class="mono" bind:value={r.dateFormat} spellcheck="false" /></label>
          <p class="hint mono">{t("formatHint")}</p>
          {#if r.source === "taken"}
            <label class="check"><input type="checkbox" bind:checked={r.fallback} /> {t("takenFallback")}</label>
            <p class="hint">{t("takenHint")}</p>
          {/if}
        {/if}
        <div class="row">
          <label>{t("where")}
            <select bind:value={r.at}>
              <option value="start">{t("atStart")}</option>
              <option value="end">{t("atEnd")}</option>
              <option value="index">{t("atIndex")}</option>
              <option value="fromEnd">{t("atFromEnd")}</option>
            </select>
          </label>
          {#if r.at === "index" || r.at === "fromEnd"}
            <label class="narrow">{t("position")} <input type="number" min="0" bind:value={r.position} /></label>
          {/if}
        </div>
      {:else if r.type === "number"}
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("templateHint")}</p>
        <div class="row">
          <label class="narrow">{t("start")} <input type="number" bind:value={r.start} /></label>
          <label class="narrow">{t("step")} <input type="number" bind:value={r.step} /></label>
          <label class="narrow">{t("digits")} <input type="number" min="0" max="12" bind:value={r.digits} /></label>
        </div>
        <label class="check"><input type="checkbox" bind:checked={r.perFolder} /> {t("perFolder")}</label>
      {:else if r.type === "remove"}
        <div class="checks">
          <label class="check"><input type="checkbox" bind:checked={r.extraSpaces} /> {t("rmExtraSpaces")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.allSpaces} /> {t("rmAllSpaces")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.accents} /> {t("rmAccents")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.digits} /> {t("rmDigits")}</label>
        </div>
        <label>{t("chars")} <input class="mono" bind:value={r.chars} spellcheck="false" placeholder="_#[]" /></label>
        {@render target()}
      {:else if r.type === "removeRange"}
        <div class="row">
          <label>{t("from")}
            <select bind:value={r.from}>
              <option value="start">{t("bStart")}</option>
              <option value="position">{t("bPosition")}</option>
              <option value="text">{t("bText")}</option>
            </select>
          </label>
          {#if r.from === "position"}
            <label class="narrow">{t("position")} <input type="number" min="1" bind:value={r.fromPosition} /></label>
          {:else if r.from === "text"}
            <label>{t("text")} <input bind:value={r.fromText} spellcheck="false" /></label>
          {/if}
        </div>
        <div class="row">
          <label>{t("to")}
            <select bind:value={r.to}>
              <option value="end">{t("bEnd")}</option>
              <option value="position">{t("bPosition")}</option>
              <option value="text">{t("bText")}</option>
            </select>
          </label>
          {#if r.to === "position"}
            <label class="narrow">{t("position")} <input type="number" min="1" bind:value={r.toPosition} /></label>
          {:else if r.to === "text"}
            <label>{t("text")} <input bind:value={r.toText} spellcheck="false" /></label>
          {/if}
        </div>
        {#if r.from === "text" || r.to === "text"}
          <label class="check"><input type="checkbox" bind:checked={r.inclusive} /> {t("inclusive")}</label>
        {/if}
      {:else if r.type === "charAt"}
        <div class="row">
          <label class="narrow">{t("position")} <input type="number" min="1" bind:value={r.position} /></label>
          <label class="check"><input type="checkbox" bind:checked={r.fromEnd} /> {t("countFromEnd")}</label>
        </div>
        <div class="row">
          <label>{t("onlyIf")} <input bind:value={r.onlyIf} maxlength="4" placeholder={t("any")} /></label>
          <label>{t("replaceBy")} <input bind:value={r.replace} placeholder={t("deleteIt")} /></label>
        </div>
        {#if r.onlyIf}
          <label class="check"><input type="checkbox" bind:checked={r.caseSensitive} /> {t("caseSensitive")}</label>
        {/if}
      {:else if r.type === "extension"}
        <div class="row">
          <label>{t("action")}
            <select bind:value={r.mode}>
              <option value="lower">{t("extLower")}</option>
              <option value="upper">{t("extUpper")}</option>
              <option value="set">{t("extSet")}</option>
              <option value="remove">{t("extRemove")}</option>
            </select>
          </label>
          {#if r.mode === "set"}
            <label>{t("newExt")} <input bind:value={r.value} spellcheck="false" placeholder="jpg" /></label>
          {/if}
        </div>
      {:else if r.type === "movie"}
        <p class="hint top">{t("movieHint")}</p>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("movieTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colon} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.folder} /> <span>{t("folderLabel")}<small>{t("folderHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.sidecar} /> <span>{t("sidecarLabel")}<small>{t("sidecarHint")}</small></span></label>
        {#if r.sidecar}
          <label class="check sub"><input type="checkbox" bind:checked={r.fanart} /> {t("fanartLabel")}</label>
        {/if}
        {@render actions(t("tmdbSearch"), "movieSummary")}
      {:else if r.type === "series"}
        <p class="hint top">{t("seriesHint")}</p>
        <label>{t("seriesShow")} <input bind:value={r.show} placeholder={t("seriesShowPlaceholder")} spellcheck="false" /></label>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("seriesTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colon} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{t("seriesFoldersLabel")}<small>{t("seriesFoldersHint")}</small></span></label>
        {#if r.folders}
          <label class="sub">{t("seriesFolderName")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.sidecar} /> <span>{t("seriesSidecarLabel")}<small>{t("seriesSidecarHint")}</small></span></label>
        {#if r.sidecar}
          <label class="check sub"><input type="checkbox" bind:checked={r.fanart} /> {t("fanartLabel")}</label>
        {/if}
        {#if lookup?.show}<p class="hint">{t("seriesCurrent", { show: lookup.show })}</p>{/if}
        {@render actions(t("tvSearch"), "seriesSummary")}
      {:else if r.type === "music"}
        <p class="hint top">{t("musicHint")}</p>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("musicTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colon} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.discPrefix} /> <span>{t("musicDiscPrefix")}<small>{t("musicDiscPrefixHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.fromName} /> <span>{t("musicFromName")}<small>{t("musicFromNameHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{t("musicFolders")}<small>{t("musicFoldersHint")}</small></span></label>
        {#if r.folders}
          <label class="sub">{t("musicFolderName")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.playlist} /> <span>{t("musicPlaylist")}<small>{t("musicPlaylistHint")}</small></span></label>
        <p class="hint top">{t("mbHint")}</p>
        <label class="check"><input type="checkbox" bind:checked={r.listen} /> <span>{t("musicListen")}<small>{t("musicListenHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.cover} /> <span>{t("musicCover")}<small>{t("musicCoverHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.writeTags} /> <span>{t("musicWriteTags")}<small>{t("musicWriteTagsHint")}</small></span></label>
        {#if r.writeTags}
          <label class="check sub"><input type="checkbox" bind:checked={r.embedCover} /> <span>{t("musicEmbedCover")}<small>{t("musicEmbedCoverHint")}</small></span></label>
          <label class="check sub"><input type="checkbox" bind:checked={r.cleanTags} /> <span>{t("musicCleanTags")}<small>{t("musicCleanTagsHint")}</small></span></label>
        {/if}
        {@render actions(t("mbSearch"), "albumSummary")}
      {:else if r.type === "book"}
        <p class="hint top">{t("bookHint")}</p>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("bookTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colon} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{t("bookFolders")}<small>{t("bookFoldersHint")}</small></span></label>
        {#if r.folders}
          <label class="sub">{t("musicFolderName")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.cover} /> <span>{t("bookCover")}<small>{t("bookCoverHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.opf} /> <span>{t("bookOpf")}<small>{t("bookOpfHint")}</small></span></label>
        {@render actions(t("olSearch"), "bookSummary")}
      {:else if r.type === "comic"}
        <p class="hint top">{t("comicHint")}</p>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("comicTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colon} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.byFolder} /> <span>{t("comicByFolder")}<small>{t("comicByFolderHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{t("comicFolders")}<small>{t("comicFoldersHint")}</small></span></label>
        {#if r.folders}
          <label class="sub">{t("musicFolderName")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <label>{t("comicSource")}
          <select bind:value={r.source}>
            <option value="gcd">{t("comicSourceGcd")}</option>
            <option value="mangadex">{t("comicSourceMangadex")}</option>
            <option value="comicvine">{t("comicSourceCv")}</option>
          </select>
        </label>
        <p class="hint">{t(r.source === "comicvine" ? "cvHint" : r.source === "mangadex" ? "mdHint" : "gcdHint")}</p>
        <label>{t("comicSeries")} <input bind:value={r.series} placeholder={t("comicSeriesPlaceholder")} spellcheck="false" /></label>
        {#if lookup?.show}<p class="hint">{t("seriesCurrent", { show: lookup.show })}</p>{/if}
        {@render actions(t("comicSearch"), "comicSummary")}
      {/if}

      {#if summary}<p class="hint">{summary}</p>{/if}
      {#if error}<p class="error">{error}</p>{/if}
    </div>
  {/if}
</article>

{#snippet actions(label: string, summary: "movieSummary" | "seriesSummary" | "albumSummary" | "bookSummary" | "comicSummary")}
  {#if lookup}
    <div class="movie-actions">
      <button class="primary-soft" disabled={!!lookup.progress} onclick={lookup.onsearch}>{label}</button>
      {#if lookup.choose || lookup.none}
        <button onclick={lookup.onreview}>{t("movieReview")} ({lookup.choose + lookup.none})</button>
      {/if}
    </div>
    {#if lookup.progress}
      <p class="hint">{lookup.progress}</p>
    {:else if lookup.found || lookup.choose || lookup.none}
      <p class="hint">{t(summary, { found: lookup.found, choose: lookup.choose, none: lookup.none })}</p>
    {/if}
  {/if}
{/snippet}

{#snippet target()}
  {#if "target" in r}
    <label>{t("applyTo")}
      <select bind:value={r.target}>
        <option value="name">{t("targetName")}</option>
        <option value="ext">{t("targetExt")}</option>
        <option value="full">{t("targetFull")}</option>
      </select>
    </label>
  {/if}
{/snippet}

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    transition: opacity 0.15s;
  }
  .card.off { opacity: 0.55; }
  .card.has-error { border-color: var(--danger); }
  header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
  }
  .title {
    flex: 1;
    text-align: left;
    background: none;
    border: none;
    font-weight: 600;
    padding: 2px 4px;
    color: var(--text);
    /* Long titles shrink so the buttons stay visible in a narrow panel. */
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title:hover:not(:disabled) { background: none; }
  .chev { display: inline-block; transition: transform 0.15s; color: var(--muted); }
  .chev.closed { transform: rotate(-90deg); }
  .actions { display: flex; gap: 2px; }
  .body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px 12px 12px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    color: var(--muted);
    flex: 1;
    min-width: 0;
  }
  label.narrow { flex: 0 0 80px; }
  label.check {
    flex-direction: row;
    align-items: center;
    gap: 6px;
    color: var(--text);
    flex: 0 1 auto;
  }
  .row { display: flex; gap: 10px; align-items: flex-end; flex-wrap: wrap; }
  .checks { display: flex; flex-direction: column; gap: 4px; }
  .hint { margin: -4px 0 0; font-size: 11px; color: var(--muted); }
  .error { margin: 0; font-size: 12px; color: var(--danger); }
  .sep { white-space: pre; }
  .hint.top { margin: 0; }
  label.colon { flex: none; align-self: flex-start; }
  label.colon input { width: 90px; }
  label.check span { display: flex; flex-direction: column; }
  label.check small { color: var(--muted); font-size: 11px; }
  label.sub { margin-left: 22px; }
  .movie-actions { display: flex; gap: 8px; flex-wrap: wrap; }
  .parts { display: flex; flex-direction: column; gap: 3px; }
  .parts .hint { margin: 0 0 2px; }
  .part {
    display: flex;
    align-items: center;
    gap: 8px;
    height: auto;
    min-height: 26px;
    padding: 2px 8px 2px 4px;
    text-align: left;
    font-size: 12px;
    white-space: normal;
  }
  .part b {
    flex: none;
    display: inline-grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 4px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 11px;
  }
  .part span { overflow-wrap: anywhere; }
</style>
