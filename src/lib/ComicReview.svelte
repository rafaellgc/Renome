<script lang="ts">
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

  const norm = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
  const shown = $derived.by(() => {
    const all = vol?.issues ?? [];
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

  function onkeydown(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input")) return;
    if (e.key === "ArrowRight") go(1);
    if (e.key === "ArrowLeft") go(-1);
  }
</script>

<svelte:window {onkeydown} />

<div class="overlay">
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="modal review" onclick={(e) => e.stopPropagation()}>
    <header>
      <div>
        <h2>{t("comicReviewTitle")}</h2>
        <p class="file" title={path}>{names[path] ?? path}</p>
        <p class="muted small">{t("comicRead", { read })} · {vol ? `${vol.volume.name}${vol.volume.startYear ? ` (${vol.volume.startYear})` : ""}` : ""}</p>
      </div>
      <span class="muted">{t("reviewOf", { i: index + 1, n: queue.length })}</span>
      <button class="icon" title={t("close")} aria-label={t("close")} onclick={onclose}>✕</button>
    </header>

    <input class="filter" bind:value={query} placeholder={t("comicIssueFilter")} spellcheck="false" />

    {#if shown.length}
      <div class="list">
        {#each shown as i (i.id)}
          <button class="issue" class:current={st?.choice === i.id} onclick={() => choose(i.id)}>
            {#if i.cover}<img src={i.cover} alt="" loading="lazy" />{:else}<span class="nocover"></span>{/if}
            <b>{i.number ? `#${i.number}` : "—"}</b>
            <span class="name">{i.name}</span>
            <span class="muted small">{i.coverDate}</span>
          </button>
        {/each}
      </div>
    {:else}
      <p class="muted empty">{t("reviewNoResults")}</p>
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
  .review { width: min(820px, 94vw); height: 86vh; max-height: 86vh; gap: 10px; }
  header { display: flex; align-items: flex-start; gap: 12px; }
  header > div { flex: 1; min-width: 0; }
  h2 { margin: 0; font-size: 16px; }
  .file { margin: 2px 0 0; font-size: 13px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  header p { margin: 2px 0 0; }
  .filter { width: 100%; }
  .empty { margin: 20px 0; text-align: center; }
  .list { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 4px; }
  .issue {
    display: grid;
    grid-template-columns: 36px 70px 1fr auto;
    align-items: center;
    gap: 10px;
    height: auto;
    padding: 4px 8px;
    text-align: left;
    white-space: normal;
  }
  .issue:hover { border-color: var(--accent); }
  .issue.current { border-color: var(--accent); background: var(--accent-soft); }
  .issue img, .nocover { width: 36px; height: 54px; border-radius: 3px; object-fit: cover; background: var(--surface-2); }
  .name { min-width: 0; }
  .small { font-size: 12px; }
  .spacer { flex: 1; }
  .modal-foot { margin-top: auto; }
  .muted { color: var(--muted); }
</style>
