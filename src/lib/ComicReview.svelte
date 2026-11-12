<script lang="ts">
  import { dialog } from "./modal";
  import Icon from "./Icon.svelte";
  import { api, SAME_COVER, type OwnCover } from "./api";
  import CoverZoom from "./CoverZoom.svelte";
  import { t } from "./i18n";
  import type { ComicIssue, ComicState, ComicVolume } from "./types";

  let {
    queue,
    comics,
    volumes,
    names,
    onchoose,
    onclose,
  }: {
    /** Files to review, in order. */
    queue: string[];
    comics: Record<string, ComicState>;
    volumes: Record<string, { volume: ComicVolume; issues: ComicIssue[] }>;
    /** Current name of each file. */
    names: Record<string, string>;
    /** `null`: none of the issues. */
    onchoose: (path: string, issue: string | null) => void;
    onclose: () => void;
  } = $props();

  let index = $state(0);
  const path = $derived(queue[index]);
  const st = $derived(comics[path]);
  const vol = $derived(st ? volumes[st.volumeKey] : undefined);
  let query = $state("");

  // A new file: start over.
  $effect(() => {
    if (!path) return;
    query = "";
  });

  /** What was read from the file, for the header. */
  const read = $derived.by(() => {
    const o = st?.own;
    if (!o) return "";
    const parts = [o.series];
    if (o.volume != null) parts.push(`v${String(o.volume).padStart(2, "0")}`);
    if (o.number) parts.push(`#${o.number}`);
    if (o.chapter) parts.push(`c${o.chapter}`);
    if (o.year) parts.push(`(${o.year})`);
    return parts.join(" ");
  });

  /** The file's own cover, and how far each issue's cover is from it. */
  let own = $state<OwnCover | null>(null);
  let far = $state<Record<string, number | null>>({});
  let comparing = $state(false);
  /** Issues with a cover, the most compared at once. */
  const withCover = $derived((vol?.issues ?? []).filter((i) => i.cover).slice(0, 300));
  $effect(() => {
    const p = path;
    own = null;
    far = {};
    if (p) api.ownCover(p).then((c) => { if (path === p) own = c; });
  });
  async function compareCovers() {
    const hash = own?.hash;
    const urls = withCover.map((i) => i.cover).filter((u) => !(u in far));
    if (!hash || !urls.length) return;
    comparing = true;
    try {
      const d = await api.coverDistances(hash, urls);
      if (own?.hash !== hash) return;
      const next = { ...far };
      urls.forEach((u, i) => (next[u] = d[i]));
      far = next;
    } finally {
      comparing = false;
    }
  }
  // Few covers: compared at once; many: when asked (each is a download).
  $effect(() => {
    if (own && withCover.length && withCover.length <= 60 && !withCover.some((i) => i.cover in far)) compareCovers();
  });
  const distance = (i: ComicIssue) => {
    const d = far[i.cover];
    return d != null && d <= SAME_COVER ? d : null;
  };
  const compared = $derived(withCover.some((i) => i.cover in far));

  const norm = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
  const shown = $derived.by(() => {
    // The issues whose cover is the file's first, nearest first.
    const all = [...(vol?.issues ?? [])].sort((a, b) => (distance(a) ?? 99) - (distance(b) ?? 99));
    const q = norm(query.trim()).replace(/^#/, "");
    if (!q) return all;
    return all.filter((i) => norm(i.number) === q.replace(/^0+(?=\d)/, "") || norm(i.number).startsWith(q) || norm(i.name).includes(q));
  });

  function go(delta: number) {
    const next = index + delta;
    if (next < 0) return;
    if (next >= queue.length) onclose();
    else index = next;
  }

  function choose(issue: string | null) {
    onchoose(path, issue);
    go(1);
  }

  /** The covers shown large: the file's beside an issue's (or alone). */
  let zoom = $state<{ url: string; name: string } | null>(null);
  $effect(() => {
    path;
    zoom = null;
  });

  function onkeydown(e: KeyboardEvent) {
    if (zoom || (e.target as HTMLElement).closest("input")) return;
    if (e.key === "ArrowRight") go(1);
    if (e.key === "ArrowLeft") go(-1);
  }
</script>

<svelte:window {onkeydown} />

<div class="overlay">
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="modal review" use:dialog onclick={(e) => e.stopPropagation()}>
    <header>
      {#if own}
        <button class="own" title={t("coverZoomOwn")} aria-label={t("ownCover")} onclick={() => (zoom = { url: "", name: "" })}><img src={own.image} alt={t("ownCover")} /></button>
      {/if}
      <div>
        <h2>{t("comicReviewTitle")}</h2>
        <p class="file" title={path}>{names[path] ?? path}</p>
        <p class="muted small">{t("comicRead", { read })} · {vol ? `${vol.volume.name}${vol.volume.startYear ? ` (${vol.volume.startYear})` : ""}` : ""}</p>
      </div>
      <span class="muted">{t("reviewOf", { i: index + 1, n: queue.length })}</span>
      <button class="icon close" title={t("close")} aria-label={t("close")} onclick={onclose}><Icon name="x" size={18} /></button>
    </header>

    <div class="tools">
      <input class="filter" bind:value={query} placeholder={t("comicIssueFilter")} spellcheck="false" />
      {#if own && withCover.length}
        <button disabled={comparing || withCover.every((i) => i.cover in far)} onclick={compareCovers}>
          {comparing ? t("coverComparing") : compared ? t("coverCompared") : t("coverCompare", { n: withCover.length })}
        </button>
      {:else if own && st?.volumeKey.startsWith("gcd:")}
        <span class="muted small gcdnote">{t("coverNoneGcd")}</span>
      {/if}
    </div>

    {#if shown.length}
      <div class="list">
        {#each shown as i (i.id)}
          <div class="issue" class:current={st?.choice === i.id}>
            {#if i.cover}
              <button class="pic" title={t("coverZoom")} onclick={() => (zoom = { url: i.cover, name: [vol?.volume.name, i.number ? `#${i.number}` : "", i.name].filter(Boolean).join(" ") })}><img src={i.cover} alt="" loading="lazy" /></button>
            {:else}<span class="nocover"></span>{/if}
            <button class="pick" onclick={() => choose(i.id)}>
              <b>{i.number ? `#${i.number}` : "—"}</b>
              <span class="name">{i.name}</span>
              <span class="muted small">{#if distance(i) !== null}<span class="same" title={t("sameCoverHint")}>{t("sameCover")}</span> {/if}{i.coverDate}</span>
            </button>
          </div>
        {/each}
      </div>
    {:else}
      <p class="muted empty">{t("reviewNoResults")}</p>
    {/if}

    {#if zoom}
      <CoverZoom own={own?.image ?? null} url={zoom.url} name={zoom.name} onclose={() => (zoom = null)} />
    {/if}

    <div class="modal-foot">
      <button disabled={index === 0} onclick={() => go(-1)}>← {t("reviewPrev")}</button>
      <span class="spacer"></span>
      <button onclick={() => choose(null)}>{t("comicNone")}</button>
      <button onclick={() => go(1)}>{t("reviewNext")} →</button>
    </div>
  </div>
</div>

<style>
  .review { position: relative; width: min(820px, 94vw); height: 86vh; max-height: 86vh; gap: 10px; }
  header { display: flex; align-items: flex-start; gap: 12px; }
  header > div { flex: 1; min-width: 0; }
  h2 { margin: 0; font-size: var(--fs-lg); }
  .file { margin: 2px 0 0; font-size: var(--fs-md); color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  header p { margin: 2px 0 0; }
  .tools { display: flex; gap: 8px; }
  .filter { flex: 1; }
  .own { all: unset; flex: none; cursor: zoom-in; }
  .own img { display: block; width: 48px; height: 72px; object-fit: cover; border-radius: 3px; background: var(--surface-2); }
  .gcdnote { align-self: center; }
  .same { font-size: var(--fs-xs); padding: 1px 6px; border-radius: 8px; background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .empty { margin: 20px 0; text-align: center; }
  .list { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 4px; }
  .issue {
    display: grid;
    grid-template-columns: 36px 1fr;
    align-items: center;
    gap: 10px;
    padding: 4px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
  }
  .issue .pic { all: unset; cursor: zoom-in; display: block; }
  .issue .pic:focus-visible, .issue .pick:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .issue .pick {
    all: unset;
    cursor: pointer;
    display: grid;
    grid-template-columns: 70px 1fr auto;
    align-items: center;
    gap: 10px;
    min-height: 54px;
  }
  .issue:hover { border-color: var(--accent); }
  .issue.current { border-color: var(--accent); background: var(--accent-soft); }
  .issue img, .nocover { display: block; width: 36px; height: 54px; border-radius: 3px; object-fit: cover; background: var(--surface-2); }
  .name { min-width: 0; }
  .small { font-size: var(--fs-sm); }
  .spacer { flex: 1; }
  .modal-foot { margin-top: auto; }
  .muted { color: var(--muted); }
</style>
