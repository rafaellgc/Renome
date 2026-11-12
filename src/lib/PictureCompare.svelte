<script lang="ts">
  import { dialog as dialogBox } from "./modal";
  import Icon from "./Icon.svelte";
  import { api, type PictureCard } from "./api";
  import { confirm, dialog, notify } from "./dialogs.svelte";
  import { LOCALES, t, tError } from "./i18n";
  import { settings } from "./settings.svelte";

  let {
    kind,
    groups,
    start = 0,
    onclose,
  }: {
    /** Pictures that look alike, or identical copies. */
    kind: "similar" | "identical";
    groups: string[][];
    /** The group shown first. */
    start?: number;
    /** `trashed`: the files that went to the trash. */
    onclose: (trashed: string[]) => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let index = $state(Math.min(start, groups.length - 1));
  let cards = $state<Record<string, PictureCard>>({});
  let marked = $state(new Set<string>());
  /** The picture shown big, and its larger copy once read. */
  let zoom = $state<string | null>(null);
  let big = $state<Record<string, string | null>>({});
  let busy = $state(false);

  const group = $derived(groups[index] ?? []);

  // Cards of the group shown, then of the next one, so moving on is quick.
  $effect(() => {
    for (const i of [index, index + 1]) {
      const missing = (groups[i] ?? []).filter((p) => !(p in cards));
      if (missing.length) {
        api.pictureCards(missing, 480).then((found) => {
          const next = { ...cards };
          for (const c of found) next[c.path] = c;
          cards = next;
        });
      }
    }
  });

  $effect(() => {
    const p = zoom;
    if (p && !(p in big)) {
      big = { ...big, [p]: null };
      api.pictureCards([p], 1800).then(([c]) => (big = { ...big, [p]: c?.image ?? null }));
    }
  });

  const pixels = (c?: PictureCard) => (c?.width && c?.height ? c.width * c.height : 0);
  /** The card worth keeping: most pixels, then the largest file. */
  function best(paths: string[]) {
    return [...paths].sort((a, b) => pixels(cards[b]) - pixels(cards[a]) || (cards[b]?.size ?? 0) - (cards[a]?.size ?? 0))[0];
  }
  /** When the photo was taken, else when the file was changed (seconds). */
  const when = (c?: PictureCard) => (c?.taken ? Date.parse(c.taken) / 1000 : (c?.modified ?? Infinity));

  const loaded = $derived(group.every((p) => p in cards));
  const largest = $derived.by(() => {
    if (!loaded) return null;
    const top = best(group);
    const c = cards[top];
    // Only when it stands out.
    return group.some((p) => pixels(cards[p]) !== pixels(c) || cards[p].size !== c.size) ? top : null;
  });
  const oldest = $derived.by(() => {
    if (!loaded) return null;
    const sorted = [...group].sort((a, b) => when(cards[a]) - when(cards[b]));
    return when(cards[sorted[0]]) < when(cards[sorted[1]]) ? sorted[0] : null;
  });

  function toggle(p: string) {
    const next = new Set(marked);
    if (next.has(p)) next.delete(p);
    else next.add(p);
    marked = next;
  }

  function keepBest() {
    const keep = best(group);
    const next = new Set(marked);
    for (const p of group) {
      if (p === keep) next.delete(p);
      else next.add(p);
    }
    marked = next;
  }

  function unmark() {
    const next = new Set(marked);
    for (const p of group) next.delete(p);
    marked = next;
  }

  function go(step: number) {
    zoom = null;
    index = Math.max(0, Math.min(groups.length - 1, index + step));
  }

  async function moveToTrash() {
    const paths = [...marked];
    const whole = groups.filter((g) => g.every((p) => marked.has(p))).length;
    const ok = await confirm({
      title: t("compareTrashTitle", { n: paths.length }),
      text: [t("compareTrashText"), whole ? t("compareTrashWhole", { n: whole }) : ""].filter(Boolean).join("\n\n"),
      ok: t("compareTrash"),
      cancel: t("cancel"),
      danger: true,
    });
    if (!ok) return;
    busy = true;
    try {
      const r = await api.trashFiles(paths);
      if (r.failed.length) {
        await notify({ title: t("compareTrashFailed", { n: r.failed.length }), items: r.failed.map(([p, e]) => `${base(p)}: ${e.replace(/^trashFailed:\s*/, "")}`) });
      }
      onclose(r.trashed);
    } catch (e) {
      await notify({ title: t("error"), text: tError(e) });
    } finally {
      busy = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    // A question over the window takes the keys.
    if (busy || dialog.current) return;
    if (e.key === "Escape") {
      e.preventDefault();
      if (zoom) zoom = null;
      else onclose([]);
      return;
    }
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    e.preventDefault();
    const step = e.key === "ArrowRight" ? 1 : -1;
    if (zoom) {
      const i = group.indexOf(zoom);
      zoom = group[(i + step + group.length) % group.length];
    } else go(step);
  }

  const base = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const folder = (p: string) => p.replace(/[\\/][^\\/]*$/, "");
  const home = /^\/(var\/)?home\/[^/]+/;
  // Where each picture is: in a group spread over folders, only what tells
  // them apart ("…/Phone", "…/WhatsApp"); the whole folder otherwise.
  function where(p: string) {
    const dirs = group.map((g) => folder(g).split(/[\\/]/));
    const own = folder(p).split(/[\\/]/);
    let k = 0;
    while (k < own.length && dirs.every((d) => d.length > k && d[k] === own[k])) k++;
    if (dirs.every((d) => d.length === k) || k === 0) return folder(p).replace(home, "~");
    const sep = p.includes("\\") && !p.includes("/") ? "\\" : "/";
    // A folder holding the others: all start from its name.
    const from = dirs.some((d) => d.length === k) ? k - 1 : k;
    return `…${sep}${own.slice(from).join(sep)}`;
  }
  const ext = (p: string) => (base(p).includes(".") ? base(p).split(".").pop()!.toUpperCase() : "");
  function size(n: number) {
    if (n < 1024) return `${n} B`;
    const units = ["KB", "MB", "GB", "TB"];
    let v = n / 1024;
    let u = 0;
    while (v >= 1024 && u < units.length - 1) { v /= 1024; u++; }
    return `${v.toFixed(v < 10 ? 1 : 0)} ${units[u]}`;
  }
  const fmt = $derived(new Intl.DateTimeFormat(LOCALES[settings.lang], { dateStyle: "short", timeStyle: "short" }));
  const takenAt = (c: PictureCard) => (c.taken ? fmt.format(new Date(c.taken)) : null);
  const changedAt = (c: PictureCard) => (c.modified === null ? "" : fmt.format(new Date(c.modified * 1000)));
</script>

<!-- Before the page's own keys, which close a question on Esc. -->
<svelte:window onkeydowncapture={onkeydown} />

<div class="overlay">
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="modal compare" use:dialogBox onclick={(e) => e.stopPropagation()}>
    <header>
      <h2>{t(kind === "similar" ? "compareSimilarTitle" : "compareIdenticalTitle")}</h2>
      <span class="muted">{t("compareGroupOf", { i: index + 1, n: groups.length })}</span>
      <button class="icon close" title={t("close")} aria-label={t("close")} disabled={busy} onclick={() => onclose([])}><Icon name="x" size={18} /></button>
    </header>

    {#if zoom}
      {@const c = cards[zoom]}
      <div class="zoom">
        <button class="zoom-pic" title={t("compareBack")} onclick={() => (zoom = null)}>
          {#if big[zoom]}<img src={big[zoom]} alt={base(zoom)} />{:else if c?.image}<img src={c.image} alt={base(zoom)} />{/if}
        </button>
        <div class="zoom-bar">
          <span class="name" title={zoom}>{base(zoom)}</span>
          <span class="muted path" title={folder(zoom)}><bdi>{where(zoom)}</bdi></span>
          {#if c?.width}<span class="muted">{c.width}×{c.height}</span>{/if}
          <span class="spacer"></span>
          <label class="mark"><input type="checkbox" checked={marked.has(zoom)} onchange={() => zoom && toggle(zoom)} /> {t("compareMark")}</label>
          <button onclick={() => (zoom = null)}>{t("compareBack")}</button>
        </div>
      </div>
    {:else}
      <p class="muted hint">{t(kind === "similar" ? "compareSimilarHint" : "compareIdenticalHint")}</p>
      <div class="grid">
        {#each group as p (p)}
          {@const c = cards[p]}
          <div class="card" class:marked={marked.has(p)}>
            <button class="pic" title={t("compareZoom")} onclick={() => (zoom = p)} disabled={!c?.image}>
              {#if c?.image}<img src={c.image} alt={base(p)} />{:else if c}<span class="noimg">{ext(p) || "?"}</span>{:else}<span class="noimg">…</span>{/if}
            </button>
            <div class="info">
              <b class="name" title={p}>{base(p)}</b>
              <span class="muted path" title={folder(p)}><bdi>{where(p)}</bdi></span>
              {#if c}
                <span>{[c.width ? `${c.width}×${c.height}` : "", size(c.size)].filter(Boolean).join(" · ")}</span>
                <span class="muted">{takenAt(c) ? t("compareTaken", { d: takenAt(c) ?? "" }) : t("compareChanged", { d: changedAt(c) })}</span>
              {/if}
              <span class="badges">
                {#if largest === p}<span class="badge">★ {t("compareLargest")}</span>{/if}
                {#if oldest === p}<span class="badge">{t("compareOldest")}</span>{/if}
              </span>
              <label class="mark"><input type="checkbox" checked={marked.has(p)} onchange={() => toggle(p)} /> {t("compareMark")}</label>
            </div>
          </div>
        {/each}
      </div>
      <div class="tools">
        <button disabled={!loaded} onclick={keepBest}>{t("compareKeepBest")}</button>
        <button disabled={!group.some((p) => marked.has(p))} onclick={unmark}>{t("compareUnmark")}</button>
      </div>
    {/if}

    <div class="modal-foot">
      <button disabled={index === 0 || busy} onclick={() => go(-1)}>← {t("reviewPrev")}</button>
      <span class="spacer"></span>
      <span class="muted count">{t("compareMarkedN", { n: marked.size })}</span>
      <button class="danger-fill" disabled={!marked.size || busy} onclick={moveToTrash}>{t("compareTrash")}</button>
      <button disabled={index >= groups.length - 1 || busy} onclick={() => go(1)}>{t("reviewNext")} →</button>
    </div>
  </div>
</div>

<style>
  .compare { width: min(1100px, 95vw); height: 88vh; max-height: 88vh; gap: 10px; }
  header { display: flex; align-items: center; gap: 12px; }
  h2 { margin: 0; font-size: var(--fs-lg); flex: 1; }
  .hint { margin: 0; font-size: var(--fs-md); }
  .grid {
    flex: 1;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 12px;
    align-content: start;
  }
  .card {
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface-2);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .card.marked { border-color: var(--danger, #c0392b); box-shadow: 0 0 0 1px var(--danger, #c0392b); }
  .card.marked .pic img { opacity: 0.55; }
  .pic {
    all: unset;
    cursor: zoom-in;
    height: 200px;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.06);
  }
  .pic:disabled { cursor: default; }
  .pic:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .pic img { max-width: 100%; max-height: 200px; object-fit: contain; display: block; }
  .noimg { font-size: 22px; font-weight: 600; color: var(--muted); }
  .info { padding: 8px 10px 10px; display: flex; flex-direction: column; gap: 2px; font-size: var(--fs-md); min-width: 0; }
  .name, .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* Cut at the start: the end tells two folders apart. */
  .path { direction: rtl; text-align: left; }
  .zoom-bar .name { flex-shrink: 0; max-width: 40%; }
  .zoom-bar .path { min-width: 0; flex: 0 1 auto; }
  .badges { display: flex; gap: 6px; min-height: 18px; margin-top: 2px; }
  .badge { font-size: var(--fs-xs); padding: 1px 6px; border-radius: 8px; background: var(--accent-soft, rgb(37 99 235 / 0.12)); color: var(--accent); }
  .mark { display: flex; align-items: center; gap: 6px; margin-top: 4px; cursor: pointer; }
  .tools { display: flex; gap: 8px; }
  .zoom { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: 8px; }
  .zoom-pic { all: unset; cursor: zoom-out; flex: 1; min-height: 0; position: relative; background: rgb(0 0 0 / 0.06); border-radius: 6px; }
  .zoom-pic img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; }
  .modal-foot .count { align-self: center; }
  .zoom-bar { display: flex; align-items: center; gap: 10px; font-size: var(--fs-md); }
  .spacer { flex: 1; }
</style>
