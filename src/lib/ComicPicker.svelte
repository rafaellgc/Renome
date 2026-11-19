<script lang="ts">
  import { dialog } from "./modal";
  import Icon from "./Icon.svelte";
  import { untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, type OwnCover } from "./api";
  import CoverZoom from "./CoverZoom.svelte";
  import { t, tError, type Key } from "./i18n";
  import { TABLE } from "./rules";
  import { comicKey, langs } from "./settings.svelte";
  import type { ComicMatch, ComicSource, ComicVolume } from "./types";

  let {
    source,
    skip = "",
    query: initial,
    files,
    highest,
    candidates: found,
    path,
    comic,
    onpick,
  }: {
    source: ComicSource;
    /** Publishers the rule leaves out. */
    skip?: string;
    /** What was searched for. */
    query: string;
    /** How many files are of this series. */
    files: number;
    /** The highest issue number among them. */
    highest: number;
    candidates: ComicVolume[];
    /** A file of the series, whose cover is compared with the series'. */
    path?: string;
    /** What that file says of itself (its number tells which cover). */
    comic?: ComicMatch;
    /** `null`: skip this series. */
    onpick: (v: ComicVolume | null) => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let query = $state(initial);
  // svelte-ignore state_referenced_locally
  let candidates = $state(found);
  let year = $state("");
  let publisher = $state("");
  let searching = $state(false);
  let searchError = $state("");

  async function search(e: SubmitEvent) {
    e.preventDefault();
    if (!query.trim()) return;
    searching = true;
    searchError = "";
    const y = Number.parseInt(year, 10);
    try {
      candidates = await api.comicSearch(source, comicKey(source), query.trim(), langs.comic, y >= 1800 && y <= 2200 ? y : null, publisher.trim(), skip, highest);
      if (own) compareCovers(candidates);
    } catch (err) {
      searchError = tError(err);
    } finally {
      searching = false;
    }
  }

  /** The file's cover, and the series with it (by id, how far): the
   *  shared comparison (`covers::series_by_cover`, the command line's
   *  chooser too) orders them, those with it first. */
  let own = $state<OwnCover | null>(null);
  let same = $state<Record<string, number>>({});
  let comparing = $state<{ i: number; n: number } | null>(null);
  let compared = $state(false);
  /** Why the covers say little here (a key of the texts), if they do. */
  let note = $state<string | null>(null);
  /** The catalogue cover each series was compared with, by id. */
  let compared_ = $state<Record<string, string>>({});
  /** The covers shown large: the file's beside a series' (or alone). */
  let zoom = $state<{ url: string; name: string } | { own: true } | null>(null);
  const cardCover = (v: ComicVolume) => v.cover || compared_[v.id] || "";
  // svelte-ignore state_referenced_locally
  if (path) api.ownCover(path).then((c) => (own = c));

  let run = 0;
  async function compareCovers(list: ComicVolume[]) {
    const hash = own?.hash;
    const me = ++run;
    same = {};
    compared_ = {};
    compared = false;
    if (!hash || !comic) return;
    const stop = await listen<{ i: number; n: number }>("cover-progress", (e) => {
      if (me === run) comparing = e.payload;
    });
    try {
      const r = await api.seriesByCover(source, comicKey(source), hash, $state.snapshot(comic), $state.snapshot(list));
      if (me !== run) return;
      note = r.note;
      same = r.same;
      compared_ = r.covers;
      compared = r.compared;
      candidates = r.candidates;
    } catch {
      // Not compared: the list stays as it came.
    } finally {
      stop();
      if (me === run) comparing = null;
    }
  }
  // Once the file's cover is read; the list's own changes do not ask again.
  $effect(() => {
    if (own) untrack(() => compareCovers(candidates));
  });
  const sameCover = (v: ComicVolume) => v.id in same;
  const sameCount = $derived(Object.keys(same).length);

  /** Covers the page could not show by their address, read by the
   *  program instead (MangaDex refuses them to other pages). */
  let shown = $state<Record<string, string>>({});
  async function fetchCover(url: string) {
    if (url in shown) return;
    shown[url] = url;
    const data = await api.remoteImage(url).catch(() => null);
    if (data) shown[url] = data;
  }

  const SOURCE_NAME: Record<ComicSource, string> = { gcd: "Grand Comics Database", comicvine: "Comic Vine", metron: "Metron", mangadex: "MangaDex", anilist: "AniList", mangaupdates: "MangaUpdates" };

  function onkeydown(e: KeyboardEvent) {
    // The enlarged covers take the keys.
    if (zoom) return;
    if ((e.target as HTMLElement).closest("input")) return;
    const n = Number.parseInt(e.key, 10);
    if (n >= 1 && n <= Math.min(9, candidates.length)) onpick(candidates[n - 1]);
  }
</script>

<svelte:window {onkeydown} />

<div class="overlay">
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="modal review" use:dialog onclick={(e) => e.stopPropagation()}>
    <header>
      <div>
        <h2>{t("comicPickTitle")}</h2>
        <p class="muted sub">{t("comicPickFor", { series: initial, n: files })}</p>
      </div>
      {#if own}
        <button class="own" title={t("coverZoomOwn")} aria-label={t("ownCover")} onclick={() => (zoom = { own: true })}><img src={own.image} alt={t("ownCover")} /></button>
      {/if}
      <button class="icon close" title={t("close")} aria-label={t("close")} onclick={() => onpick(null)}><Icon name="x" size={18} /></button>
    </header>

    <form class="search" onsubmit={search}>
      <input bind:value={query} placeholder={t("comicSeriesPlaceholder")} title={t("linkHintComic")} spellcheck="false" />
      <input class="year" bind:value={year} oninput={() => (year = year.replace(/\D/g, ""))} inputmode="numeric" maxlength="4" placeholder={t("comicPickYear")} title={t("comicPickYearHint")} />
      {#if !TABLE.seriesOnly.includes(source)}
        <input class="publisher" bind:value={publisher} placeholder={t("comicPickPublisher")} title={t(source === "gcd" ? "comicPickPublisherGcd" : "comicPickPublisherHint")} spellcheck="false" />
      {/if}
      <button type="submit" disabled={searching}>{t("reviewSearch")}</button>
    </form>
    {#if searchError}<p class="error">{searchError}</p>{/if}

    {#if searching}
      <div class="searching" role="status">
        <span class="muted">{t("comicSearching", { series: query.trim() })}</span>
        <progress></progress>
      </div>
    {:else if candidates.length}
      <p class="muted hint">{t("comicPickHint")}</p>
      {#if own}
        <p class="muted hint covers" role="status">
          {#if note}{t(note as Key)}
          {:else if comparing}{t("coverPickComparing", comparing)}
          {:else if compared}{sameCount ? t("coverPickDone", { n: sameCount }) : t("coverPickNone")}{/if}
        </p>
      {/if}
      <div class="grid">
        {#each candidates as c, i (c.id)}
          {@const pic = cardCover(c)}
          <div class="cand">
            {#if pic}
              <button class="pic" title={t("coverZoom")} onclick={() => (zoom = { url: compared_[c.id] || pic, name: `${c.name}${c.startYear ? ` (${c.startYear})` : ""}` })}>
                <img src={shown[pic] ?? pic} alt="" loading="lazy" onerror={() => fetchCover(pic)} />
              </button>
            {:else}
              <div class="noposter">{SOURCE_NAME[source]}</div>
            {/if}
            <button class="info" onclick={() => onpick(c)}>
              <b>{c.name}{c.startYear ? ` (${c.startYear})` : ""}</b>
              {#if c.publisher}<span>{c.publisher}</span>{/if}
              {#if c.writer}<span>{c.writer}</span>{/if}
              {#if c.country || c.language}<span class="muted small">{[c.country, c.language].filter(Boolean).join(" · ").toUpperCase()}</span>{/if}
              {#if c.format}<span class="muted small">{c.format}</span>{/if}
              {#if c.count}<span class="muted small">{t("comicIssuesN", { n: c.count })}</span>{/if}
              {#if sameCover(c)}<span class="same" title={t("sameCoverHint")}>{t("sameCover")}</span>{/if}
              {#if i < 9}<kbd>{i + 1}</kbd>{/if}
            </button>
          </div>
        {/each}
      </div>
    {:else}
      <p class="muted empty">{t("reviewNoResults")}</p>
    {/if}

    {#if zoom}
      <CoverZoom own={own?.image ?? null} url={"url" in zoom ? zoom.url : ""} name={"url" in zoom ? zoom.name : ""} onclose={() => (zoom = null)} />
    {/if}

    <div class="modal-foot">
      <span class="muted small credit">{t("comicDataFrom", { source: SOURCE_NAME[source] })}</span>
      <span class="spacer"></span>
      <button onclick={() => onpick(null)}>{t("comicSkip")}</button>
    </div>
  </div>
</div>

<style>
  .review { position: relative; width: min(980px, 94vw); height: 86vh; max-height: 86vh; gap: 12px; }
  header { display: flex; align-items: flex-start; gap: 12px; }
  header > div { flex: 1; min-width: 0; }
  h2 { margin: 0; font-size: var(--fs-lg); }
  .sub { margin: 2px 0 0; font-size: var(--fs-md); }
  .search { display: flex; gap: 8px; }
  .search input { flex: 1; min-width: 0; }
  .search .year { flex: none; width: 70px; }
  .search .publisher { flex: none; width: 170px; }
  .hint { margin: 0; font-size: var(--fs-sm); }
  .covers:empty { display: none; }
  .own { all: unset; flex: none; cursor: zoom-in; }
  .own img { display: block; width: 48px; height: 72px; object-fit: cover; border-radius: 3px; background: var(--surface-2); }
  .same { align-self: flex-start; font-size: var(--fs-xs); padding: 1px 6px; border-radius: 8px; background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .error { margin: 0; color: var(--danger); font-size: var(--fs-md); }
  .empty { margin: 20px 0; text-align: center; }
  .searching { display: flex; flex-direction: column; gap: 8px; align-items: center; margin: 40px auto; width: min(360px, 80%); font-size: var(--fs-md); }
  .searching progress { width: 100%; accent-color: var(--accent); }
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
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
  }
  .cand:hover { border-color: var(--accent); }
  .cand .pic { all: unset; flex: none; cursor: zoom-in; border-radius: 4px; }
  .cand .pic:focus-visible { outline: 2px solid var(--accent); }
  .cand .info { all: unset; flex: 1; align-self: stretch; cursor: pointer; display: flex; flex-direction: column; gap: 2px; min-width: 0; font-size: var(--fs-md); }
  .cand .info:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .cand img, .noposter { flex: none; width: 70px; height: 105px; border-radius: 4px; object-fit: cover; background: var(--surface-2); display: block; }
  .noposter { display: grid; place-items: center; text-align: center; font-size: var(--fs-xs); color: var(--muted); }
  .small { font-size: var(--fs-sm); }
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
