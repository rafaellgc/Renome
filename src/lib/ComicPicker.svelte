<script lang="ts">
  import { api } from "./api";
  import { t, tError } from "./i18n";
  import { settings } from "./settings.svelte";
  import type { ComicSource, ComicVolume } from "./types";

  let {
    source,
    query: initial,
    files,
    candidates: found,
    onpick,
  }: {
    source: ComicSource;
    /** What was searched for. */
    query: string;
    /** How many files are of this series. */
    files: number;
    candidates: ComicVolume[];
    /** `null`: skip this series. */
    onpick: (v: ComicVolume | null) => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let query = $state(initial);
  // svelte-ignore state_referenced_locally
  let candidates = $state(found);
  let searching = $state(false);
  let searchError = $state("");

  async function search(e: SubmitEvent) {
    e.preventDefault();
    if (!query.trim()) return;
    searching = true;
    searchError = "";
    try {
      candidates = await api.comicSearch(source, settings.comicVineKey.trim(), query.trim(), settings.lang === "pt" ? "pt-br" : settings.lang);
    } catch (err) {
      searchError = tError(err);
    } finally {
      searching = false;
    }
  }

  const SOURCE_NAME: Record<ComicSource, string> = { gcd: "Grand Comics Database", comicvine: "Comic Vine", mangadex: "MangaDex" };

  function onkeydown(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input")) return;
    const n = Number.parseInt(e.key, 10);
    if (n >= 1 && n <= Math.min(9, candidates.length)) onpick(candidates[n - 1]);
  }
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="overlay" onclick={() => onpick(null)}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="modal review" onclick={(e) => e.stopPropagation()}>
    <header>
      <div>
        <h2>{t("comicPickTitle")}</h2>
        <p class="muted sub">{t("comicPickFor", { series: initial, n: files })}</p>
      </div>
      <button class="icon" title={t("close")} aria-label={t("close")} onclick={() => onpick(null)}>✕</button>
    </header>

    <form class="search" onsubmit={search}>
      <input bind:value={query} placeholder={t("comicSeriesPlaceholder")} spellcheck="false" />
      <button type="submit" disabled={searching}>{t("reviewSearch")}</button>
    </form>
    {#if searchError}<p class="error">{searchError}</p>{/if}

    {#if candidates.length}
      <p class="muted hint">{t("comicPickHint")}</p>
      <div class="grid">
        {#each candidates as c, i (c.id)}
          <button class="cand" onclick={() => onpick(c)}>
            {#if c.cover}
              <img src={c.cover} alt="" loading="lazy" />
            {:else}
              <div class="noposter">{SOURCE_NAME[source]}</div>
            {/if}
            <span class="info">
              <b>{c.name}{c.startYear ? ` (${c.startYear})` : ""}</b>
              {#if c.publisher}<span>{c.publisher}</span>{/if}
              {#if c.writer}<span>{c.writer}</span>{/if}
              {#if c.country || c.language}<span class="muted small">{[c.country, c.language].filter(Boolean).join(" · ").toUpperCase()}</span>{/if}
              {#if c.format}<span class="muted small">{c.format}</span>{/if}
              {#if c.count}<span class="muted small">{t("comicIssuesN", { n: c.count })}</span>{/if}
            </span>
            {#if i < 9}<kbd>{i + 1}</kbd>{/if}
          </button>
        {/each}
      </div>
    {:else if !searching}
      <p class="muted empty">{t("reviewNoResults")}</p>
    {/if}

    <div class="modal-foot">
      <span class="muted small credit">{t("comicDataFrom", { source: SOURCE_NAME[source] })}</span>
      <span class="spacer"></span>
      <button onclick={() => onpick(null)}>{t("comicSkip")}</button>
    </div>
  </div>
</div>

<style>
  .review { width: min(980px, 94vw); height: 86vh; max-height: 86vh; gap: 12px; }
  header { display: flex; align-items: flex-start; gap: 12px; }
  header > div { flex: 1; min-width: 0; }
  h2 { margin: 0; font-size: 16px; }
  .sub { margin: 2px 0 0; font-size: 13px; }
  .search { display: flex; gap: 8px; }
  .search input { flex: 1; }
  .hint { margin: 0; font-size: 12px; }
  .error { margin: 0; color: var(--danger); font-size: 13px; }
  .empty { margin: 20px 0; text-align: center; }
  .grid {
    flex: 1;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 10px;
    align-content: start;
  }
  .cand {
    position: relative;
    display: flex;
    gap: 10px;
    height: auto;
    padding: 8px;
    text-align: left;
    align-items: flex-start;
    white-space: normal;
  }
  .cand:hover { border-color: var(--accent); }
  .cand img, .noposter { flex: none; width: 70px; height: 105px; border-radius: 4px; object-fit: cover; background: var(--surface-2); }
  .noposter { display: grid; place-items: center; text-align: center; font-size: 11px; color: var(--muted); }
  .info { display: flex; flex-direction: column; gap: 2px; min-width: 0; font-size: 13px; }
  .small { font-size: 12px; }
  kbd {
    position: absolute;
    right: 6px;
    bottom: 6px;
    font: 11px ui-monospace, monospace;
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--muted);
  }
  .spacer { flex: 1; }
  .modal-foot { margin-top: auto; align-items: center; }
  .muted { color: var(--muted); }
</style>
