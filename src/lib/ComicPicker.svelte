<script lang="ts">
  import { untrack } from "svelte";
  import { api, SAME_COVER, type OwnCover } from "./api";
  import CoverZoom from "./CoverZoom.svelte";
  import { t, tError } from "./i18n";
  import { byFit, seriesOnly } from "./rules";
  import { comicKey, settings } from "./settings.svelte";
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
      const lang = settings.lang === "pt" ? "pt-br" : settings.lang;
      const found = await api.comicSearch(source, comicKey(source), query.trim(), lang, y >= 1800 && y <= 2200 ? y : null, publisher.trim(), skip);
      candidates = byFit(found, highest);
      if (own) compareCovers(candidates);
    } catch (err) {
      searchError = tError(err);
    } finally {
      searching = false;
    }
  }

  /** The file's cover, and how far each series' cover is from it (by id;
   *  `null`: nothing to compare). */
  let own = $state<OwnCover | null>(null);
  let far = $state<Record<string, number | null>>({});
  let comparing = $state<{ i: number; n: number } | null>(null);
  let compared = $state(false);
  /** The catalogue cover each series was compared with, by id. */
  let compared_ = $state<Record<string, string>>({});
  /** The covers shown large: the file's beside a series' (or alone). */
  let zoom = $state<{ url: string; name: string } | { own: true } | null>(null);
  const cardCover = (v: ComicVolume) => v.cover || compared_[v.id] || "";
  // svelte-ignore state_referenced_locally
  if (path) api.ownCover(path).then((c) => (own = c));

  /** Catalogues of whole series have one cover: the first volume's. */
  const firstOnly = $derived(source === "anilist" || source === "mangaupdates");
  const isFirst = $derived(!comic || ((comic.volume ?? 1) === 1 && (!comic.number || Number.parseFloat(comic.number) === 1)));
  /** The series compared: those of keys 1–9 (each is one or two requests). */
  let run = 0;
  async function compareCovers(list: ComicVolume[]) {
    const hash = own?.hash;
    const me = ++run;
    far = {};
    compared_ = {};
    compared = false;
    if (!hash || !comic || source === "gcd" || (firstOnly && !isFirst)) return;
    const top = list.slice(0, 9);
    const next: Record<string, number | null> = {};
    const pics: Record<string, string> = {};
    for (const [i, v] of top.entries()) {
      comparing = { i: i + 1, n: top.length };
      let urls: string[] = [];
      if (firstOnly) urls = v.cover ? [v.cover] : [];
      // A long series is many pages of issues: left out.
      else if (v.count <= 200) urls = await api.seriesCovers(source, comicKey(source), v.id, comic).catch(() => []);
      if (me !== run) return;
      const d = urls.length ? await api.coverDistances(hash, urls).catch(() => []) : [];
      if (me !== run) return;
      const near = d.filter((x): x is number => x != null);
      next[v.id] = near.length ? Math.min(...near) : null;
      far = { ...next };
      // The cover compared (the nearest), shown on the card and enlarged.
      const at = d.findIndex((x) => x != null && x === next[v.id]);
      if (urls.length) pics[v.id] = urls[Math.max(0, at)];
      compared_ = { ...pics };
    }
    comparing = null;
    compared = true;
    // Those with the file's cover first, once all are known (keys stay put
    // while comparing).
    const same = (v: ComicVolume) => (far[v.id] ?? 99) <= SAME_COVER;
    candidates = [...list.filter(same).sort((a, b) => (far[a.id] ?? 99) - (far[b.id] ?? 99)), ...list.filter((v) => !same(v))];
  }
  // Once the file's cover is read; the list's own changes do not ask again.
  $effect(() => {
    if (own) untrack(() => compareCovers(candidates));
  });
  const sameCover = (v: ComicVolume) => {
    const d = far[v.id];
    return d != null && d <= SAME_COVER;
  };
  const sameCount = $derived(Object.values(far).filter((d) => d != null && d <= SAME_COVER).length);

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
  <div class="modal review" onclick={(e) => e.stopPropagation()}>
    <header>
      <div>
        <h2>{t("comicPickTitle")}</h2>
        <p class="muted sub">{t("comicPickFor", { series: initial, n: files })}</p>
      </div>
      {#if own}
        <button class="own" title={t("coverZoomOwn")} aria-label={t("ownCover")} onclick={() => (zoom = { own: true })}><img src={own.image} alt={t("ownCover")} /></button>
      {/if}
      <button class="icon" title={t("close")} aria-label={t("close")} onclick={() => onpick(null)}>✕</button>
    </header>

    <form class="search" onsubmit={search}>
      <input bind:value={query} placeholder={t("comicSeriesPlaceholder")} title={t("linkHintComic")} spellcheck="false" />
      <input class="year" bind:value={year} oninput={() => (year = year.replace(/\D/g, ""))} inputmode="numeric" maxlength="4" placeholder={t("comicPickYear")} title={t("comicPickYearHint")} />
      {#if !seriesOnly(source)}
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
          {#if source === "gcd"}{t("coverNoneGcd")}
          {:else if firstOnly}{t("coverNoneSeries")}
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
  h2 { margin: 0; font-size: 16px; }
  .sub { margin: 2px 0 0; font-size: 13px; }
  .search { display: flex; gap: 8px; }
  .search input { flex: 1; min-width: 0; }
  .search .year { flex: none; width: 70px; }
  .search .publisher { flex: none; width: 170px; }
  .hint { margin: 0; font-size: 12px; }
  .covers:empty { display: none; }
  .own { all: unset; flex: none; cursor: zoom-in; }
  .own img { display: block; width: 48px; height: 72px; object-fit: cover; border-radius: 3px; background: var(--surface-2); }
  .same { align-self: flex-start; font-size: 11px; padding: 1px 6px; border-radius: 8px; background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .error { margin: 0; color: var(--danger); font-size: 13px; }
  .empty { margin: 20px 0; text-align: center; }
  .searching { display: flex; flex-direction: column; gap: 8px; align-items: center; margin: 40px auto; width: min(360px, 80%); font-size: 13px; }
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
  .cand .info { all: unset; flex: 1; align-self: stretch; cursor: pointer; display: flex; flex-direction: column; gap: 2px; min-width: 0; font-size: 13px; }
  .cand .info:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .cand img, .noposter { flex: none; width: 70px; height: 105px; border-radius: 4px; object-fit: cover; background: var(--surface-2); display: block; }
  .noposter { display: grid; place-items: center; text-align: center; font-size: 11px; color: var(--muted); }
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
