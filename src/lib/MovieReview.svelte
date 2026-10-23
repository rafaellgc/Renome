<script lang="ts">
  import { api } from "./api";
  import { t, tError } from "./i18n";
  import { settings, tmdbLang } from "./settings.svelte";
  import type { Candidate, MovieState } from "./types";

  let {
    queue,
    movies,
    names,
    onchoose,
    onclose,
  }: {
    /** Files to review, in order. */
    queue: string[];
    movies: Record<string, MovieState>;
    /** Current name of each file. */
    names: Record<string, string>;
    /** `null`: none of the candidates. */
    onchoose: (path: string, choice: Candidate | null, candidates?: Candidate[]) => void;
    onclose: () => void;
  } = $props();

  let index = $state(0);
  const path = $derived(queue[index]);
  const m = $derived(movies[path]);
  let title = $state("");
  let year = $state("");
  let searching = $state(false);
  let searchError = $state("");
  /** Results of a search typed here (replace the automatic ones). */
  let typed = $state<Candidate[] | null>(null);
  const candidates = $derived(typed ?? m?.candidates ?? []);

  // A new file: start from its guess.
  $effect(() => {
    if (!path) return;
    title = movies[path]?.guess.title ?? "";
    year = movies[path]?.guess.year?.toString() ?? "";
    typed = null;
    searchError = "";
  });

  function go(delta: number) {
    const next = index + delta;
    if (next < 0) return;
    if (next >= queue.length) onclose();
    else index = next;
  }

  function choose(c: Candidate | null) {
    onchoose(path, c, typed ?? undefined);
    // Skip files the choice already settled (subtitles of the same movie).
    const next = queue.findIndex((p, j) => j > index && (movies[p]?.status === "choose" || movies[p]?.status === "none"));
    if (next < 0) onclose();
    else index = next;
  }

  async function search(e: SubmitEvent) {
    e.preventDefault();
    if (!title.trim()) return;
    searching = true;
    searchError = "";
    try {
      const y = Number.parseInt(year, 10);
      typed = await api.tmdbSearch(settings.tmdbKey, tmdbLang(), title.trim(), Number.isFinite(y) ? y : null);
    } catch (err) {
      searchError = tError(err);
    } finally {
      searching = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input")) return;
    if (e.key === "ArrowRight") go(1);
    if (e.key === "ArrowLeft") go(-1);
    const n = Number.parseInt(e.key, 10);
    if (n >= 1 && n <= candidates.length) choose(candidates[n - 1]);
  }

  const poster = (p: string | null) => (p ? `https://image.tmdb.org/t/p/w185${p}` : null);
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="overlay" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="modal review" onclick={(e) => e.stopPropagation()}>
    <header>
      <div>
        <h2>{t("reviewTitle")}</h2>
        <p class="file" title={path}>{names[path] ?? path}</p>
      </div>
      <span class="muted">{t("reviewOf", { i: index + 1, n: queue.length })}</span>
      <button class="icon" title={t("close")} aria-label={t("close")} onclick={onclose}>✕</button>
    </header>

    <form class="search" onsubmit={search}>
      <span class="muted">{t("reviewGuess")}</span>
      <input class="title" bind:value={title} placeholder={t("reviewTitleField")} spellcheck="false" />
      <input class="year" bind:value={year} placeholder={t("reviewYear")} inputmode="numeric" maxlength="4" />
      <button type="submit" disabled={searching}>{t("reviewSearch")}</button>
    </form>

    {#if searchError || m?.error}
      <p class="error">{searchError || tError(m?.error)}</p>
    {/if}

    {#if candidates.length}
      <p class="muted hint">{t("reviewPick")}</p>
      <div class="grid">
        {#each candidates.slice(0, 12) as c, i (c.id)}
          <button class="cand" class:current={m?.choice?.id === c.id} onclick={() => choose(c)}>
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

    <div class="modal-foot">
      <button disabled={index === 0} onclick={() => go(-1)}>← {t("reviewPrev")}</button>
      <span class="spacer"></span>
      <button onclick={() => choose(null)}>{t("reviewNone")}</button>
      <button onclick={() => go(1)}>{t("reviewNext")} →</button>
    </div>
  </div>
</div>

<style>
  .review { width: min(980px, 94vw); height: 86vh; max-height: 86vh; gap: 12px; }
  header { display: flex; align-items: flex-start; gap: 12px; }
  header > div { flex: 1; min-width: 0; }
  h2 { margin: 0; font-size: 16px; }
  .file { margin: 2px 0 0; font-size: 13px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .search { display: flex; align-items: center; gap: 8px; }
  .search .title { flex: 1; }
  .search .year { width: 70px; }
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
  .cand:hover:not(:disabled) { border-color: var(--accent); }
  .cand.current { border-color: var(--accent); background: var(--accent-soft); }
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
  .spacer { flex: 1; }
  .modal-foot { margin-top: auto; }
  .muted { color: var(--muted); }
</style>
