<script lang="ts">
  import { tick } from "svelte";
  import { closeDialog, dialog } from "./dialogs.svelte";
  import { t } from "./i18n";

  let box = $state<HTMLDivElement>();

  // Focus the button Enter should press: the confirm button, except in
  // destructive dialogs, where it is "cancel".
  $effect(() => {
    if (dialog.current && box) {
      tick().then(() => box?.querySelector<HTMLButtonElement>("button[data-default]")?.focus());
    }
  });
</script>

{#if dialog.current}
  {@const d = dialog.current}
  <div class="overlay top">
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby="dialog-title" tabindex="-1" bind:this={box} onclick={(e) => e.stopPropagation()}>
      <h2 id="dialog-title">{d.title}</h2>
      {#if d.text}<p class="text">{d.text}</p>{/if}
      {#if d.items?.length}
        <ul class="items">
          {#each d.items as item, i (i)}<li>{item}</li>{/each}
        </ul>
      {/if}
      <div class="modal-foot">
        {#if d.cancel}
          <button data-default={d.danger || undefined} onclick={() => closeDialog(false)}>{d.cancel}</button>
          <button data-default={!d.danger || undefined} class={d.danger ? "danger-fill" : "primary"} onclick={() => closeDialog(true)}>{d.ok ?? t("close")}</button>
        {:else}
          <button data-default class="primary" onclick={() => closeDialog(true)}>{d.ok ?? t("close")}</button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .top { z-index: 20; }
  .text { margin: 0; white-space: pre-line; line-height: 1.45; }
  .items {
    margin: 0;
    padding: 8px 10px 8px 26px;
    max-height: 40vh;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface-2);
    font-size: 13px;
    line-height: 1.5;
  }
  .items li { overflow-wrap: anywhere; }
</style>
