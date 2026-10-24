<script lang="ts">
  import { api } from "./api";
  import { t, tError } from "./i18n";
  import type { AlbumState, MbCandidate } from "./types";

  let {
    queue,
    albums,
    onchoose,
    onclose,
  }: {
    /** Albums to review (indexes into `albums`), in order. */
    queue: number[];
    albums: AlbumState[];
    /** `null`: none of these (keep the tags). Resolves once matched. */
    onchoose: (album: number, choice: MbCandidate | null, candidates?: MbCandidate[]) => Promise<void>;
    onclose: () => void;
  } = $props();

  let index = $state(0);
  const a = $derived(albums[queue[index]]);
  let artist = $state("");
  let album = $state("");
  let busy = $state(false);
  let searchError = $state("");
  /** Results of a search typed here (replace the automatic ones). */
  let typed = $state<MbCandidate[] | null>(null);
  const candidates = $derived(typed ?? a?.candidates ?? []);
  /** Release covers that failed to load. */
  let noCover = $state(new Set<string>());

  // A new album: start from what its tags say.
  $effect(() => {
    if (!a) return;
    artist = albums[queue[index]].query.artist;
    album = albums[queue[index]].query.album;
    typed = null;
    searchError = "";
  });

  function go(delta: number) {
    const next = index + delta;
    if (next < 0) return;
    if (next >= queue.length) onclose();
    else index = next;
  }

  async function choose(c: MbCandidate | null) {
    busy = true;
    try {
      await onchoose(queue[index], c, typed ?? undefined);
      go(1);
    } catch (err) {
      searchError = tError(err);
    } finally {
      busy = false;
    }
  }

  async function search(e: SubmitEvent) {
    e.preventDefault();
    if (!album.trim() || !a) return;
    busy = true;
    searchError = "";
    try {
      typed = await api.mbSearch({ artist: artist.trim(), album: album.trim(), year: a.query.year, tracks: a.files.length });
    } catch (err) {
      searchError = tError(err);
    } finally {
      busy = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input") || busy) return;
    if (e.key === "ArrowRight") go(1);
    if (e.key === "ArrowLeft") go(-1);
    const n = Number.parseInt(e.key, 10);
    if (n >= 1 && n <= Math.min(9, candidates.length)) choose(candidates[n - 1]);
  }

  const folder = (dir: string) => dir.split(/[\\/]/).pop() ?? dir;
  const cover = (id: string) => `https://coverartarchive.org/release/${id}/front-250`;
  const matched = $derived(a?.choice ? Object.keys(a.matches).length : 0);
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="overlay" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="modal review" onclick={(e) => e.stopPropagation()}>
    <header>
      <div>
        <h2>{t("albumReviewTitle")}</h2>
        <p class="file" title={a?.dir}>{a ? `${folder(a.dir)} · ${t("albumFiles", { n: a.files.length })}` : ""}</p>
      </div>
      <span class="muted">{t("reviewOf", { i: index + 1, n: queue.length })}</span>
      <button class="icon" title={t("close")} aria-label={t("close")} onclick={onclose}>✕</button>
    </header>

    {#if a}
      <p class="read">
        <span class="muted">{t("albumRead")}</span>
        {a.query.artist || "—"} · {a.query.album}{a.query.year ? ` (${a.query.year})` : ""}
        {#if a.choice}<span class="muted"> · {t("albumMatched", { n: matched, total: a.files.length })}</span>{/if}
      </p>
      <form class="search" onsubmit={search}>
        <input class="artist" bind:value={artist} placeholder={t("albumArtistField")} spellcheck="false" />
        <input class="album" bind:value={album} placeholder={t("albumField")} spellcheck="false" />
        <button type="submit" disabled={busy}>{t("reviewSearch")}</button>
      </form>
      {#if searchError || a.error}<p class="error">{searchError || tError(a.error)}</p>{/if}

      {#if candidates.length}
        <p class="muted hint">{t("albumPick")}</p>
        <div class="grid">
          {#each candidates.slice(0, 12) as c, i (c.id)}
            <button class="cand" class:current={a.choice?.id === c.id} disabled={busy} onclick={() => choose(c)}>
              {#if noCover.has(c.id)}
                <div class="nocover">♪</div>
              {:else}
                <img src={cover(c.id)} alt="" loading="lazy" onerror={() => (noCover = new Set(noCover).add(c.id))} />
              {/if}
              <span class="info">
                <b>{c.title}</b>
                <span>{c.artist}</span>
                <span class="muted">{[c.date, c.country, c.format, c.label, c.kind].filter(Boolean).join(" · ")}</span>
                <span class:same={c.tracks === a.files.length} class="tracks">{t("albumTracks", { n: c.tracks })}</span>
              </span>
              {#if i < 9}<kbd>{i + 1}</kbd>{/if}
            </button>
          {/each}
        </div>
      {:else if !busy}
        <p class="muted empty">{t("reviewNoResults")}</p>
      {/if}
    {/if}

    <div class="modal-foot">
      <button disabled={index === 0 || busy} onclick={() => go(-1)}>← {t("reviewPrev")}</button>
      <span class="spacer"></span>
      <button disabled={busy} onclick={() => choose(null)}>{t("albumKeepTags")}</button>
      <button disabled={busy} onclick={() => go(1)}>{t("reviewNext")} →</button>
    </div>
  </div>
</div>

<style>
  .review { width: min(980px, 94vw); height: 86vh; max-height: 86vh; gap: 10px; }
  header { display: flex; align-items: flex-start; gap: 12px; }
  header > div { flex: 1; min-width: 0; }
  h2 { margin: 0; font-size: 16px; }
  .file { margin: 2px 0 0; font-size: 13px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .read { margin: 0; font-size: 13px; }
  .search { display: flex; gap: 8px; }
  .search .artist { flex: 2; }
  .search .album { flex: 3; }
  .hint { margin: 0; font-size: 12px; }
  .error { margin: 0; color: var(--danger); font-size: 13px; }
  .empty { margin: 20px 0; text-align: center; }
  .grid {
    flex: 1;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(290px, 1fr));
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
  .cand img, .nocover { flex: none; width: 84px; height: 84px; border-radius: 4px; object-fit: cover; background: var(--surface-2); }
  .nocover { display: grid; place-items: center; font-size: 28px; color: var(--muted); }
  .info { display: flex; flex-direction: column; gap: 2px; min-width: 0; font-size: 13px; }
  .info .muted { font-size: 12px; }
  .tracks { font-size: 12px; color: var(--muted); }
  .tracks.same { color: #138a4e; font-weight: 600; }
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
