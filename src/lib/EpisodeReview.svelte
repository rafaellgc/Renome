<script lang="ts">
  import { t } from "./i18n";
  import type { EpKey, Episode, EpisodeState, Show } from "./types";

  let {
    queue,
    series,
    shows,
    names,
    onchoose,
    onclose,
  }: {
    /** Files to review, in order. */
    queue: string[];
    series: Record<string, EpisodeState>;
    shows: Record<number, Show>;
    /** Current name of each file. */
    names: Record<string, string>;
    /** `null`: none of the episodes. */
    onchoose: (path: string, choice: EpKey[] | null) => void;
    onclose: () => void;
  } = $props();

  let index = $state(0);
  const path = $derived(queue[index]);
  const st = $derived(series[path]);
  const show = $derived(st ? shows[st.showId] : undefined);
  let query = $state("");
  /** Episodes marked with Ctrl+click, for a file with several. */
  let marked = $state<EpKey[]>([]);

  // A new file: start over.
  $effect(() => {
    if (!path) return;
    query = "";
    marked = [];
  });

  const pad = (n: number) => String(n).padStart(2, "0");
  const code = (keys: EpKey[]) => (keys.length ? `S${pad(keys[0][0])}E${keys.map((k) => pad(k[1])).join("-E")}` : "");
  const episode = (k: EpKey) => show?.episodes.find((e) => e.season === k[0] && e.number === k[1]);
  const same = (a: EpKey[] | null | undefined, b: EpKey[]) =>
    !!a && a.length === b.length && a.every((k, i) => k[0] === b[i][0] && k[1] === b[i][1]);
  const isCurrent = (e: Episode) => !!st?.choice?.some((k) => k[0] === e.season && k[1] === e.number);
  const isMarked = (e: Episode) => marked.some((k) => k[0] === e.season && k[1] === e.number);

  /** What was read from the name, for the header. */
  const read = $derived.by(() => {
    const g = st?.guess;
    if (!g) return "";
    const parts: string[] = [];
    if (g.episodes.length) {
      parts.push(g.season !== null ? code(g.episodes.map((e): EpKey => [g.season!, e])) : g.episodes.map((e) => `E${pad(e)}`).join("-"));
    } else if (g.number !== null) parts.push(String(g.number));
    else if (g.date) parts.push(g.date);
    if (g.title) parts.push(`“${g.title}”`);
    return parts.join(" · ") || t("epReadNothing");
  });

  const norm = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
  /** Episodes by season (specials last), filtered by the text typed. */
  const seasons = $derived.by(() => {
    if (!show) return [];
    const q = norm(query.trim());
    const hit = (e: Episode) =>
      !q ||
      [e.name, ...e.names].some((n) => norm(n).includes(q)) ||
      `s${pad(e.season)}e${pad(e.number)}`.includes(q.replace(/\s/g, "")) ||
      `${e.season}x${pad(e.number)}`.includes(q);
    const groups = new Map<number, Episode[]>();
    for (const e of show.episodes.filter(hit)) {
      if (!groups.has(e.season)) groups.set(e.season, []);
      groups.get(e.season)!.push(e);
    }
    return [...groups.entries()];
  });

  function go(delta: number) {
    const next = index + delta;
    if (next < 0) return;
    if (next >= queue.length) onclose();
    else index = next;
  }

  function choose(keys: EpKey[] | null) {
    onchoose(path, keys);
    // Skip files the choice already settled (subtitles of the same episode).
    const next = queue.findIndex((p, j) => j > index && (series[p]?.status === "choose" || series[p]?.status === "none"));
    if (next < 0) onclose();
    else index = next;
  }

  function pick(e: Episode, ev: MouseEvent) {
    const k: EpKey = [e.season, e.number];
    if (ev.ctrlKey || ev.metaKey) {
      // Episodes run together are from one season.
      const kept = marked.filter((m) => m[0] === k[0]);
      marked = isMarked(e) ? kept.filter((m) => m[1] !== k[1]) : [...kept, k].sort((a, b) => a[1] - b[1]);
    } else {
      choose([k]);
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input")) return;
    const alts = st?.alternatives ?? [];
    if (e.key === "ArrowRight") go(1);
    if (e.key === "ArrowLeft") go(-1);
    if (e.key === "Enter" && alts.length) choose(alts[0]);
    const n = Number.parseInt(e.key, 10);
    if (n >= 1 && n <= Math.min(9, alts.length)) choose(alts[n - 1]);
  }

  const still = (p: string | null) => (p ? `https://image.tmdb.org/t/p/w300${p}` : null);
  const date = (d: string | null) => (d ? new Date(`${d}T12:00`).toLocaleDateString() : "");
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="overlay" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="modal review" onclick={(e) => e.stopPropagation()}>
    <header>
      <div>
        <h2>{t("epReviewTitle")}{show ? ` — ${show.name}${show.year ? ` (${show.year})` : ""}` : ""}</h2>
        <p class="file" title={path}>{names[path] ?? path}</p>
      </div>
      <span class="muted">{t("reviewOf", { i: index + 1, n: queue.length })}</span>
      <button class="icon" title={t("close")} aria-label={t("close")} onclick={onclose}>✕</button>
    </header>

    <p class="read"><span class="muted">{t("epRead")}</span> {read}</p>
    {#if st?.conflict && st.alternatives.length > 1}
      <p class="warn">{t("epConflict", { num: code(st.alternatives[1]), title: code(st.alternatives[0]) })}</p>
    {/if}

    <div class="body">
      {#if st?.alternatives.length}
        <p class="muted hint">{t("epSuggestions")}</p>
        <div class="grid">
          {#each st.alternatives.slice(0, 9) as keys, i (code(keys))}
            {@const e = episode(keys[0])}
            <button class="cand" class:current={same(st.choice, keys)} onclick={() => choose(keys)}>
              {#if still(e?.stillPath ?? null)}
                <img src={still(e!.stillPath)} alt="" loading="lazy" />
              {:else}
                <div class="nostill">TMDB</div>
              {/if}
              <span class="info">
                <b><span class="code">{code(keys)}</span> {keys.map((k) => episode(k)?.name).join(" & ")}</b>
                {#if e?.airDate}<i>{date(e.airDate)}</i>{/if}
                <span class="overview">{e?.overview}</span>
              </span>
              <kbd>{i + 1}</kbd>
            </button>
          {/each}
        </div>
      {/if}

      <div class="all-head">
        <p class="muted hint">{t("epAll")}</p>
        <input bind:value={query} placeholder={t("epFilter")} spellcheck="false" />
      </div>
      <div class="list">
        {#each seasons as [season, episodes] (season)}
          <h3>{season === 0 ? t("epSpecials") : t("epSeason", { n: season })}</h3>
          {#each episodes as e (`${e.season}x${e.number}`)}
            <button class="ep" class:current={isCurrent(e)} class:marked={isMarked(e)} onclick={(ev) => pick(e, ev)}>
              <span class="code">{code([[e.season, e.number]])}</span>
              <span class="name">{e.name}</span>
              <span class="muted">{date(e.airDate)}</span>
            </button>
          {/each}
        {/each}
      </div>
      <p class="muted hint">{t("epMultiHint")}</p>
    </div>

    <div class="modal-foot">
      <button disabled={index === 0} onclick={() => go(-1)}>← {t("reviewPrev")}</button>
      <span class="spacer"></span>
      {#if marked.length}
        <button class="primary" onclick={() => choose(marked)}>{t("epUseSelected", { codes: code(marked) })}</button>
      {/if}
      <button onclick={() => choose(null)}>{t("reviewNone")}</button>
      <button onclick={() => go(1)}>{t("reviewNext")} →</button>
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
  .warn { margin: 0; padding: 6px 10px; border-radius: 6px; background: var(--warn); color: black; font-size: 13px; }
  .hint { margin: 0; font-size: 12px; }
  .body { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 10px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 10px; }
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
  .cand:hover, .ep:hover { border-color: var(--accent); }
  .cand.current, .ep.current { border-color: var(--accent); background: var(--accent-soft); }
  .cand img, .nostill { flex: none; width: 120px; height: 68px; border-radius: 4px; object-fit: cover; background: var(--surface-2); }
  .nostill { display: grid; place-items: center; font-size: 11px; color: var(--muted); }
  .info { display: flex; flex-direction: column; gap: 2px; min-width: 0; font-size: 13px; }
  .info i { color: var(--muted); font-size: 12px; }
  .overview {
    color: var(--muted);
    font-size: 12px;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .code { font: 12px ui-monospace, monospace; color: var(--accent); font-weight: 600; }
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
  .all-head { display: flex; align-items: center; gap: 10px; margin-top: 4px; }
  .all-head p { flex: 1; }
  .all-head input { width: min(280px, 50%); }
  .list { display: flex; flex-direction: column; gap: 3px; }
  h3 { margin: 8px 0 2px; font-size: 13px; }
  .ep {
    display: flex;
    align-items: center;
    gap: 10px;
    height: auto;
    min-height: 28px;
    padding: 3px 8px;
    text-align: left;
    white-space: normal;
  }
  .ep .name { flex: 1; min-width: 0; }
  .ep .muted { font-size: 12px; }
  .ep.marked { box-shadow: inset 3px 0 var(--accent); background: var(--hover); }
  .spacer { flex: 1; }
  .modal-foot { margin-top: auto; }
  .muted { color: var(--muted); }
</style>
