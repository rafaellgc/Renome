<script lang="ts">
  import { dialog } from "./modal";
  import Icon from "./Icon.svelte";
  import { t } from "./i18n";
  import type { EditField } from "./types";

  let {
    heading,
    hint,
    fields,
    rows,
    onsave,
    onclose,
  }: {
    heading: string;
    hint: string;
    fields: EditField[];
    /** What each file chosen says now, by field (empty: nothing). */
    rows: Record<string, string>[];
    /** The fields changed here, as typed (empty: cleared). */
    onsave: (changed: Record<string, string>) => void;
    onclose: () => void;
  } = $props();

  /** A field's value when every file has the same; null when they differ. */
  function common(k: string): string | null {
    const values = new Set(rows.map((r) => r[k] ?? ""));
    return values.size === 1 ? [...values][0] : null;
  }
  // svelte-ignore state_referenced_locally
  const start: Record<string, string | null> = Object.fromEntries(fields.map((f) => [f.key, common(f.key)]));
  // svelte-ignore state_referenced_locally
  let values = $state<Record<string, string>>(Object.fromEntries(fields.map((f) => [f.key, start[f.key] ?? ""])));
  /** Fields typed in (those left alone keep each file's own). */
  let touched = $state(new Set<string>());

  const changed = $derived(
    Object.fromEntries(fields.filter((f) => touched.has(f.key) && values[f.key].trim() !== (start[f.key] ?? "")).map((f) => [f.key, values[f.key].trim()])),
  );

  function save(e: SubmitEvent) {
    e.preventDefault();
    onsave(changed);
  }
</script>

<div class="overlay">
  <form class="modal edit" use:dialog onsubmit={save}>
    <header>
      <h2>{heading}</h2>
      <button type="button" class="icon close" title={t("close")} aria-label={t("close")} onclick={onclose}><Icon name="x" size={18} /></button>
    </header>
    <p class="muted hint">{hint}{rows.length > 1 ? ` ${t("editHintMany")}` : ""}</p>
    <div class="grid">
      {#each fields as f (f.key)}
        <label class:wide={f.wide}>
          <span>{f.label}</span>
          <input
            bind:value={values[f.key]}
            oninput={() => {
              touched.add(f.key);
              touched = new Set(touched);
              if (f.numeric) values[f.key] = values[f.key].replace(/\D/g, "");
            }}
            inputmode={f.numeric ? "numeric" : undefined}
            maxlength={f.numeric ? 4 : undefined}
            placeholder={start[f.key] === null ? t("editMixed") : ""}
            spellcheck="false"
          />
        </label>
      {/each}
    </div>
    <div class="modal-foot">
      <span class="muted">{t("editChangedN", { n: Object.keys(changed).length })}</span>
      <span class="spacer"></span>
      <button type="button" onclick={onclose}>{t("cancel")}</button>
      <button type="submit" class="primary" disabled={!Object.keys(changed).length}>{t("editApply")}</button>
    </div>
  </form>
</div>

<style>
  .edit { width: min(640px, 95vw); gap: 10px; }
  header { display: flex; align-items: center; gap: 12px; }
  h2 { margin: 0; font-size: var(--fs-lg); flex: 1; }
  .hint { margin: 0; font-size: var(--fs-md); }
  .grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px 12px; }
  .grid label { display: flex; flex-direction: column; gap: 3px; font-size: var(--fs-md); min-width: 0; }
  .grid label.wide { grid-column: span 3; }
  .grid input { width: 100%; box-sizing: border-box; }
  .spacer { flex: 1; }
  .modal-foot { display: flex; align-items: center; gap: 8px; }
  @media (max-width: 520px) {
    .grid { grid-template-columns: minmax(0, 1fr); }
    .grid label.wide { grid-column: span 1; }
  }
</style>
