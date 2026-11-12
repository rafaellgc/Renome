<script lang="ts">
  // The rule sets saved by name (kept with the settings): a menu to open
  // one, save the current rules as one, rename or delete them, and to
  // import or export a set as a .json file.
  import { tick } from "svelte";
  import Icon from "./Icon.svelte";
  import { confirm } from "./dialogs.svelte";
  import { LOCALES, t } from "./i18n";
  import { menu as menuNav } from "./modal";
  import { settings, type RuleSet } from "./settings.svelte";
  import type { Step } from "./types";

  let {
    current,
    onload,
    onimport,
    onexport,
    onnotice,
  }: {
    /** The rules as they are now (null: there are none). */
    current: () => Step[] | null;
    onload: (set: RuleSet) => void;
    onimport: () => void;
    onexport: () => void;
    onnotice: (text: string) => void;
  } = $props();

  let open = $state(false);
  /** The set being renamed (its index) or -1 for a new one; null: none. */
  let editing = $state<number | null>(null);
  let draft = $state("");
  let field = $state<HTMLInputElement>();

  const sets = $derived(settings.ruleSets ?? []);
  const sorted = $derived(
    sets.map((s, i) => ({ s, i })).sort((a, b) => a.s.name.localeCompare(b.s.name, LOCALES[settings.lang])),
  );

  let root = $state<HTMLElement>();
  /** A click outside closes the menu. */
  function outside(e: PointerEvent) {
    if (open && root && !root.contains(e.target as Node)) close();
  }

  function close() {
    open = false;
    editing = null;
  }

  async function edit(i: number) {
    editing = i;
    draft = i >= 0 ? sets[i].name : "";
    await tick();
    field?.focus();
    field?.select();
  }

  async function commit() {
    const name = draft.trim();
    const at = editing;
    if (at === null) return;
    if (!name) {
      editing = null;
      return;
    }
    const same = sets.findIndex((s, i) => i !== at && s.name.toLocaleLowerCase() === name.toLocaleLowerCase());
    const list = [...sets];
    if (at < 0) {
      const steps = current();
      if (!steps) return;
      if (same >= 0) {
        open = false;
        const ok = await confirm({ title: t("ruleSetsTitle"), text: t("ruleSetReplace", { name: sets[same].name }), ok: t("ruleSetReplaceOk"), cancel: t("cancel") });
        if (!ok) return;
        list[same] = { name: sets[same].name, steps };
      } else list.push({ name, steps });
      settings.ruleSets = list;
      onnotice(t("ruleSetSaved", { name }));
      close();
      return;
    }
    if (same >= 0) {
      onnotice(t("ruleSetExists", { name }));
      field?.focus();
      return;
    }
    list[at] = { ...list[at], name };
    settings.ruleSets = list;
    editing = null;
  }

  async function remove(i: number) {
    const name = sets[i].name;
    open = false;
    const ok = await confirm({ title: t("ruleSetsTitle"), text: t("ruleSetDelete", { name }), ok: t("ruleSetDeleteOk"), cancel: t("cancel"), danger: true });
    if (!ok) return;
    settings.ruleSets = sets.filter((_, k) => k !== i);
    onnotice(t("ruleSetDeleted", { name }));
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      commit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      editing = null;
    }
  }
</script>

<svelte:window onpointerdown={outside} />

<div class="menu-host sets" bind:this={root}>
  <button class="with-icon" aria-haspopup="menu" aria-expanded={open} title={t("ruleSetsTitle")} onclick={() => (open ? close() : (open = true))}>
    <Icon name="layers" />{t("ruleSets")}<Icon name="chevron-down" size={14} />
  </button>
  {#if open}
    <!-- svelte-ignore a11y_interactive_supports_focus -->
    <div class="menu right sets-menu" role="menu" use:menuNav onkeydown={(e) => { if (e.key === "Escape" && editing === null) close(); }}>
      <p class="menu-group">{t("ruleSetsTitle")}</p>
      {#if !sorted.length}
        <p class="empty">{t("ruleSetsEmpty")}</p>
      {/if}
      {#each sorted as { s, i } (s.name)}
        {#if editing === i}
          <div class="edit-row">
            <input bind:this={field} bind:value={draft} onkeydown={onkey} spellcheck="false" aria-label={t("ruleSetName")} />
            <button class="icon" title={t("ruleSetSaveBtn")} aria-label={t("ruleSetSaveBtn")} onclick={commit}><Icon name="check" size={15} /></button>
          </div>
        {:else}
          <div class="set-row">
            <button role="menuitem" class="set-name" title={t("ruleSetOpen")} onclick={() => { close(); onload(s); }}>
              <span>{s.name}</span><small>{t("ruleSetCount", { n: s.steps.length })}</small>
            </button>
            <button class="icon act" title={t("ruleSetRename")} aria-label={t("ruleSetRename")} onclick={() => edit(i)}><Icon name="pencil" size={14} /></button>
            <button class="icon act danger" title={t("ruleSetDeleteBtn")} aria-label={t("ruleSetDeleteBtn")} onclick={() => remove(i)}><Icon name="trash" size={14} /></button>
          </div>
        {/if}
      {/each}
      <hr />
      {#if editing === -1}
        <div class="edit-row">
          <input bind:this={field} bind:value={draft} onkeydown={onkey} placeholder={t("ruleSetName")} spellcheck="false" aria-label={t("ruleSetName")} />
          <button class="icon" title={t("ruleSetSaveBtn")} aria-label={t("ruleSetSaveBtn")} disabled={!draft.trim()} onclick={commit}><Icon name="check" size={15} /></button>
        </div>
      {:else}
        <button role="menuitem" disabled={!current()} onclick={() => edit(-1)}><Icon name="save" />{t("ruleSetSave")}</button>
      {/if}
      <button role="menuitem" onclick={() => { close(); onimport(); }}><Icon name="upload" />{t("ruleSetImport")}</button>
      <button role="menuitem" disabled={!current()} onclick={() => { close(); onexport(); }}><Icon name="download" />{t("ruleSetExport")}</button>
    </div>
  {/if}
</div>

<style>
  .sets { position: relative; }
  .sets-menu { min-width: 280px; max-width: 340px; max-height: calc(100vh - 140px); overflow-y: auto; }
  .sets-menu > button, .set-row button { flex: none; }
  .empty { margin: 2px 10px 6px; font-size: var(--fs-sm); color: var(--muted); }
  .set-row { display: flex; align-items: center; gap: 2px; border-radius: var(--r-sm); }
  .set-row:hover { background: var(--hover); }
  .set-name { flex: 1 !important; min-width: 0; height: auto; min-height: 30px; padding: 4px 10px; display: flex; flex-direction: column; align-items: flex-start !important; gap: 0 !important; }
  .set-name span { max-width: 100%; overflow: hidden; text-overflow: ellipsis; }
  .set-name small { font-size: var(--fs-xs); color: var(--muted); }
  .act { width: 26px; height: 26px; opacity: 0.55; }
  .set-row:hover .act, .act:focus-visible { opacity: 1; }
  .act.danger:hover:not(:disabled) { color: var(--danger); }
  .edit-row { display: flex; gap: 4px; padding: 2px 4px; }
  .edit-row input { flex: 1; min-width: 0; }
  hr { margin: 4px 6px; border: none; border-top: 1px solid var(--row-line); }
</style>
