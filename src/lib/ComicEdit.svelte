<script lang="ts">
  import { t } from "./i18n";
  import type { ComicMatch } from "./types";

  type Field = "series" | "seriesYear" | "number" | "volume" | "title" | "year" | "publisher" | "writer" | "artist";

  let {
    comics,
    onsave,
    onclose,
  }: {
    /** What each comic chosen says now. */
    comics: ComicMatch[];
    /** The fields changed here, as typed (empty: cleared). */
    onsave: (changed: Partial<Record<Field, string>>) => void;
    onclose: () => void;
  } = $props();

  const FIELDS: { key: Field; label: string; numeric?: boolean; wide?: boolean }[] = [
    { key: "series", label: "comicEditSeries", wide: true },
    { key: "seriesYear", label: "comicEditSeriesYear", numeric: true },
    { key: "volume", label: "comicEditVolume", numeric: true },
    { key: "number", label: "comicEditNumber" },
    { key: "title", label: "comicEditTitle", wide: true },
    { key: "year", label: "comicEditYear", numeric: true },
    { key: "publisher", label: "comicEditPublisher" },
    { key: "writer", label: "comicEditWriter" },
    { key: "artist", label: "comicEditArtist" },
  ];

  const text = (m: ComicMatch, k: Field) => {
    const v = m[k];
    return v == null ? "" : String(v);
  };
  /** A field's value when every comic has the same; null when they differ. */
  function common(k: Field): string | null {
    const values = new Set(comics.map((m) => text(m, k)));
    return values.size === 1 ? [...values][0] : null;
  }
  // svelte-ignore state_referenced_locally
  const start = Object.fromEntries(FIELDS.map((f) => [f.key, common(f.key)])) as Record<Field, string | null>;
  let values = $state(Object.fromEntries(FIELDS.map((f) => [f.key, start[f.key] ?? ""])) as Record<Field, string>);
  /** Fields typed in (those left alone keep each comic's own). */
  let touched = $state(new Set<Field>());

  const changed = $derived(
    Object.fromEntries(FIELDS.filter((f) => touched.has(f.key) && values[f.key].trim() !== (start[f.key] ?? "")).map((f) => [f.key, values[f.key].trim()])),
  );

  function save(e: SubmitEvent) {
    e.preventDefault();
    onsave(changed);
  }
</script>

<div class="overlay">
  <form class="modal edit" onsubmit={save}>
    <header>
      <h2>{t("comicEditTitleN", { n: comics.length })}</h2>
      <button type="button" class="icon" title={t("close")} aria-label={t("close")} onclick={onclose}>✕</button>
    </header>
    <p class="muted hint">{t(comics.length > 1 ? "comicEditHintMany" : "comicEditHint")}</p>
    <div class="grid">
      {#each FIELDS as f (f.key)}
        <label class:wide={f.wide}>
          <span>{t(f.label as "comicEditSeries")}</span>
          <input
            bind:value={values[f.key]}
            oninput={() => {
              touched.add(f.key);
              touched = new Set(touched);
              if (f.numeric) values[f.key] = values[f.key].replace(/\D/g, "");
            }}
            inputmode={f.numeric ? "numeric" : undefined}
            maxlength={f.numeric ? 4 : undefined}
            placeholder={start[f.key] === null ? t("comicEditMixed") : ""}
            spellcheck="false"
          />
        </label>
      {/each}
    </div>
    <div class="modal-foot">
      <span class="muted">{t("comicEditChangedN", { n: Object.keys(changed).length })}</span>
      <span class="spacer"></span>
      <button type="button" onclick={onclose}>{t("cancel")}</button>
      <button type="submit" class="primary" disabled={!Object.keys(changed).length}>{t("comicEditApply")}</button>
    </div>
  </form>
</div>

<style>
  .edit { width: min(640px, 95vw); gap: 10px; }
  header { display: flex; align-items: center; gap: 12px; }
  h2 { margin: 0; font-size: 16px; flex: 1; }
  .hint { margin: 0; font-size: 13px; }
  .grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px 12px; }
  .grid label { display: flex; flex-direction: column; gap: 3px; font-size: 12.5px; min-width: 0; }
  .grid label.wide { grid-column: span 3; }
  .grid input { width: 100%; box-sizing: border-box; }
  .spacer { flex: 1; }
  .modal-foot { display: flex; align-items: center; gap: 8px; }
  @media (max-width: 520px) {
    .grid { grid-template-columns: minmax(0, 1fr); }
    .grid label.wide { grid-column: span 1; }
  }
</style>
