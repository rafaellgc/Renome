<script lang="ts">
  import { api } from "./api";
  import { t, tError } from "./i18n";
  import { settings, tmdbLang } from "./settings.svelte";
  import type { Candidate } from "./types";

  let {
    query: initial,
    candidates: found,
    onpick,
  }: {
    /** What was searched for. */
    query: string;
    candidates: Candidate[];
    /** `null`: closed without choosing. */
    onpick: (show: Candidate | null) => void;
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
      candidates = (await api.tvSearch(settings.tmdbKey, tmdbLang(), query.trim(), [])).candidates;
    } catch (err) {
      searchError = tError(err);
    } finally {
      searching = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input")) return;
    const n = Number.parseInt(e.key, 10);
    if (n >= 1 && n <= Math.min(9, candidates.length)) onpick(candidates[n - 1]);
  }

  const poster = (p: string | null) => (p ? `https://image.tmdb.org/t/p/w185${p}` : null);
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="overlay" onclick={() => onpick(null)}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="modal review" onclick={(e) => e.stopPropagation()}>
    <header>
      <h2>{t("showPickTitle")}</h2>
      <button class="icon" title={t("close")} aria-label={t("close")} onclick={() => onpick(null)}>✕</button>
    </header>

    <form class="search" onsubmit={search}>
      <input bind:value={query} placeholder={t("seriesShowPlaceholder")} spellcheck="false" />
      <button type="submit" disabled={searching}>{t("reviewSearch")}</button>
    </form>
    {#if searchError}<p class="error">{searchError}</p>{/if}

    {#if candidates.length}
      <p class="muted hint">{t("showPickHint")}</p>
      <div class="grid">
        {#each candidates.slice(0, 12) as c, i (c.id)}
          <button class="cand" onclick={() => onpick(c)}>
            {#if poster(c.posterPath)}
              <img src={poster(c.posterPath)} alt="" loading="lazy" />
            {:else}
              <div class="noposter">TMDB</div>
            {/if}
            <span class="info">
              <b>{c.title}{c.year ? ` (${c.year})` : ""}</b>
              {#if c.originalTitle && c.originalTitle !== c.title}<i>{c.originalTitle}</i>{/if}
              <span class="overview">{c.overview}</span>
            </span>
            {#if i < 9}<kbd>{i + 1}</kbd>{/if}
          </button>
        {/each}
      </div>
    {:else if !searching}
      <p class="muted empty">{t("reviewNoResults")}</p>
    {/if}
  </div>
</div>

<style>
  .review { width: min(980px, 94vw); height: 86vh; max-height: 86vh; gap: 12px; }
  header { display: flex; align-items: center; gap: 12px; }
  h2 { flex: 1; margin: 0; font-size: 16px; }
  .search { display: flex; gap: 8px; }
  .search input { flex: 1; }
  .hint { margin: 0; font-size: 12px; }
  .error { margin: 0; color: var(--danger); font-size: 13px; }
  .empty { margin: 20px 0; text-align: center; }
  .grid {
    flex: 1;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
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
  .noposter { display: grid; place-items: center; font-size: 11px; color: var(--muted); }
  .info { display: flex; flex-direction: column; gap: 2px; min-width: 0; font-size: 13px; }
  .info i { color: var(--muted); font-size: 12px; }
  .overview {
    color: var(--muted);
    font-size: 12px;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
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
  .muted { color: var(--muted); }
</style>
