<script lang="ts">
  import { api } from "./api";
  import { t } from "./i18n";

  let {
    own,
    url = "",
    name = "",
    onclose,
  }: {
    /** The file's cover (a data address), shown on the left. */
    own: string | null;
    /** A catalogue's cover, shown beside it; empty: the file's alone. */
    url?: string;
    name?: string;
    onclose: () => void;
  } = $props();

  /** A larger copy (MangaDex keeps a 512 px one beside the 256 px). */
  const big = $derived(url.replace(/\.256\.jpg$/, ".512.jpg"));
  /** Read by the program when the page cannot show it by its address
   *  (MangaDex refuses its pictures to other pages). */
  let fetched = $state<string | null>(null);
  let tried = false;
  async function fallback() {
    if (tried) return;
    tried = true;
    fetched = await api.remoteImage(big).catch(() => null);
  }

  // Esc closes this alone, before the window under it; no other key goes through.
  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
    e.preventDefault();
    e.stopImmediatePropagation();
  }
</script>

<svelte:window onkeydowncapture={onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="zoom" onclick={(e) => { e.stopPropagation(); onclose(); }} title={t("compareBack")}>
  {#if own}
    <figure>
      <img src={own} alt={t("ownCover")} />
      <figcaption>{t("ownCover")}</figcaption>
    </figure>
  {/if}
  {#if url}
    <figure>
      <img src={fetched ?? big} alt={name} onerror={fallback} />
      <figcaption>{name}</figcaption>
    </figure>
  {/if}
</div>

<style>
  .zoom {
    position: absolute;
    inset: 0;
    z-index: 5;
    display: flex;
    gap: 24px;
    justify-content: center;
    align-items: center;
    padding: 24px;
    background: rgb(0 0 0 / 0.82);
    border-radius: inherit;
    cursor: zoom-out;
  }
  figure { margin: 0; height: 100%; min-width: 0; flex: 1 1 0; max-width: 50%; display: flex; flex-direction: column; align-items: center; gap: 8px; }
  img { flex: 1; min-height: 0; width: 100%; object-fit: contain; }
  figcaption { color: #fff; font-size: var(--fs-md); text-align: center; }
</style>
