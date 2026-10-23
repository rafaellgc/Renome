<script lang="ts">
  import { onMount, tick } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api } from "$lib/api";
  import Dialog from "$lib/Dialog.svelte";
  import { closeDialog, confirm, dialog, notify } from "$lib/dialogs.svelte";
  import FileTable from "$lib/FileTable.svelte";
  import Help from "$lib/Help.svelte";
  import Options from "$lib/Options.svelte";
  import RuleCard from "$lib/RuleCard.svelte";
  import { LOCALES, MOD, t, tError } from "$lib/i18n";
  import { RULE_TYPES, newRule, newStep } from "$lib/rules";
  import { loadSettings, saveSettings, settings } from "$lib/settings.svelte";
  import type { ApplyReport, HistoryEntry, OpError, Preview, RuleType, Step, View, ViewSort } from "$lib/types";

  let ready = $state(false);
  let steps = $state<Step[]>([]);
  let overrides = $state<Record<string, string>>({});
  let preview = $state<Preview>({ items: [], ruleErrors: [], filterError: null, total: 0, changed: 0, conflicts: 0 });
  let selected = $state(new Set<string>());
  let filter = $state("");
  let sortKey = $state<ViewSort>("newName");
  let sortDesc = $state(false);
  let notice = $state("");
  let busy = $state(false);
  let dragging = $state(false);
  let history = $state<HistoryEntry[] | null>(null);
  let showOptions = $state(false);
  let help = $state<"manual" | "about" | null>(null);
  let addMenu = $state(false);
  let ruleMenu = $state(false);
  let table = $state<FileTable>();

  const view = $derived<View>({ filter, sortByNewName: sortKey === "newName", descending: sortDesc });
  /** Example name for rule cards: the first file in natural name order, so
   *  it does not jump around while the preview re-sorts. */
  const collator = new Intl.Collator(undefined, { numeric: true });
  /** "+ Rule" menu in alphabetical order of the current language. */
  const ruleTypes = $derived(
    [...RULE_TYPES].sort((a, b) => t(`rule.${a}`).localeCompare(t(`rule.${b}`), LOCALES[settings.lang])),
  );
  const sample = $derived(
    preview.items.reduce<string>((min, i) => (!min || collator.compare(i.original, min) < 0 ? i.original : min), ""),
  );
  const canMove = $derived(selected.size > 0 && sortKey !== "newName");

  function ruleError(i: number) {
    const e = preview.ruleErrors.find((e) => e.index === i);
    return e ? `${t(e.code)}: ${e.detail}` : null;
  }

  // Empty numeric fields become 0 so Rust accepts them.
  function payload(): Step[] {
    return $state.snapshot(steps).map((s) => {
      const rule: Record<string, unknown> = { ...s.rule };
      for (const [k, v] of Object.entries(rule)) {
        if (v === null || (typeof v === "number" && !Number.isFinite(v))) rule[k] = 0;
        else if (typeof v === "number" && k !== "start" && k !== "step") rule[k] = Math.max(0, Math.trunc(v));
      }
      return { ...s, rule } as Step;
    });
  }

  async function refresh() {
    try {
      preview = await api.preview(payload(), $state.snapshot(overrides), $state.snapshot(view));
    } catch (e) {
      notice = `${t("error")}: ${tError(e)}`;
    }
  }

  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    JSON.stringify(steps);
    JSON.stringify(overrides);
    JSON.stringify(view);
    if (!ready) return;
    clearTimeout(timer);
    timer = setTimeout(refresh, 120);
  });

  // Save the settings (and the rules, if asked) when they change.
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const deps = [settings.lang, settings.theme, settings.includeHidden, settings.confirmApply,
      settings.disableRulesAfterApply, settings.warnWindows, settings.rememberRules, JSON.stringify(steps)];
    if (!ready || !deps) return;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      settings.rules = settings.rememberRules ? payload() : [];
      saveSettings();
    }, 400);
  });

  $effect(() => {
    document.documentElement.lang = LOCALES[settings.lang];
  });

  /** Re-sorts the list in Rust by the chosen column. */
  async function applySort() {
    if (sortKey === "manual") return;
    if (sortKey === "newName") await api.sortFiles("name", false);
    else await api.sortFiles(sortKey, sortDesc);
  }

  async function sortBy(key: Exclude<ViewSort, "manual">) {
    if (key === sortKey) sortDesc = !sortDesc;
    else {
      sortKey = key;
      sortDesc = false;
    }
    await applySort();
    await refresh();
  }

  async function addPaths(paths: string[], foldersAsItems = false) {
    if (!paths.length) return;
    busy = true;
    try {
      await remember();
      const r = await api.addPaths(paths, { recursive: true, includeHidden: settings.includeHidden, foldersAsItems });
      if (!r.added.length) await api.dropCheckpoint();
      await applySort();
      await refresh();
      // Highlight what was just added.
      selected = new Set(r.added);
      await tick();
      table?.reveal(r.added);
      notice = t("added", { n: r.added.length }) + (r.skipped.length ? ` · ${t("skipped", { n: r.skipped.length })}` : "");
    } catch (e) {
      notice = `${t("error")}: ${tError(e)}`;
    } finally {
      busy = false;
    }
  }

  async function pickFiles() {
    addMenu = false;
    const r = await open({ multiple: true, title: t("addFiles") });
    if (r) await addPaths(Array.isArray(r) ? r : [r]);
  }

  async function pickFolders(asItems = false) {
    addMenu = false;
    const title = t(asItems ? "addFolderItems" : "addFolders");
    const r = await open({ directory: true, multiple: true, title });
    if (r) await addPaths(Array.isArray(r) ? r : [r], asItems);
  }

  async function removeSelected() {
    if (!selected.size) return;
    await remember();
    await api.removeFiles([...selected]);
    selected = new Set();
    await refresh();
  }

  async function clearAll() {
    await remember();
    await api.clearFiles();
    selected = new Set();
    overrides = {};
    await refresh();
  }

  async function move(delta: -1 | 1) {
    if (!canMove) return;
    await remember();
    await api.moveFiles([...selected], delta);
    sortKey = "manual";
    await refresh();
  }

  function addRule(type: RuleType) {
    steps.push(newStep(newRule(type)));
    ruleMenu = false;
  }

  function moveStep(i: number, delta: -1 | 1) {
    const j = i + delta;
    [steps[i], steps[j]] = [steps[j], steps[i]];
  }

  async function setOverride(path: string, name: string | null) {
    await remember();
    if (name === null) delete overrides[path];
    else overrides[path] = name;
  }

  const basename = (p: string) => p.split("/").pop() ?? p;
  const errText = (e: OpError) => {
    if (e.kind === "history") return `${t("err.history")} (${e.detail})`;
    const why = e.kind === "io" ? e.detail || t("err.io") : t(`err.${e.kind}`);
    return `${basename(e.from)} — ${why}`;
  };

  /** Shows the files that failed, if any. */
  function showErrors(r: ApplyReport, title: string, text: string) {
    if (!r.errors.length) return;
    return notify({ title, text, items: r.errors.map(errText) });
  }

  async function applyAll() {
    // A shortcut may come before the debounced preview update.
    clearTimeout(timer);
    await refresh();
    if (!preview.changed) return;
    if (settings.confirmApply) {
      const extra = preview.conflicts ? `\n${t("confirmConflicts", { n: preview.conflicts })}` : "";
      const ok = await confirm({
        title: t("confirmTitle"),
        text: `${t("confirmText", { n: preview.changed })}${extra}\n\n${t("confirmUndoHint")}`,
        ok: t("confirmOk"),
        cancel: t("cancel"),
      });
      if (!ok) return;
    }
    busy = true;
    try {
      const r = await api.apply(payload(), $state.snapshot(overrides), $state.snapshot(view));
      if (settings.disableRulesAfterApply) for (const s of steps) s.enabled = false;
      overrides = {};
      notice = t("renamedN", { n: r.done.length }) + (r.errors.length ? ` · ${t("errorsN", { n: r.errors.length })}` : "");
      await refresh();
      await showErrors(r, t("resultTitle"), `${t("renamedN", { n: r.done.length })}\n${t("errorsN", { n: r.errors.length })}:`);
    } catch (e) {
      notice = `${t("error")}: ${tError(e)}`;
    } finally {
      busy = false;
    }
  }

  async function openHistory() {
    history = await api.history();
  }

  async function undo(id: string) {
    busy = true;
    try {
      const r = await api.undo(id);
      notice = t("restoredN", { n: r.done.length });
      if (history) history = await api.history();
      await refresh();
      await showErrors(
        r,
        r.done.length ? t("resultTitle") : t("undoFailed"),
        `${r.done.length ? `${t("restoredN", { n: r.done.length })}\n` : ""}${t("undoFailedHint")}`,
      );
    } catch (e) {
      await notify({ title: t("undoFailed"), text: tError(e) });
    } finally {
      busy = false;
    }
  }

  /** Saves the list and the hand-edited names before an edit, for Ctrl+Z. */
  const remember = () => api.checkpoint($state.snapshot(overrides));

  /** Undoes the last edit to the list (Ctrl+Z). Renames are undone in the History. */
  async function undoListEdit() {
    const prev = await api.undoList();
    if (prev === null) {
      notice = t("nothingToUndo");
      return;
    }
    overrides = prev;
    await refresh();
    const paths = new Set(preview.items.map((i) => i.path));
    selected = new Set([...selected].filter((p) => paths.has(p)));
    notice = t("listEditUndone");
  }

  async function deleteEntry(id: string) {
    const ok = await confirm({
      title: t("deleteEntry"),
      text: t("deleteEntryText"),
      ok: t("delete"),
      cancel: t("cancel"),
      danger: true,
    });
    if (!ok) return;
    try {
      await api.deleteHistory(id);
    } catch (e) {
      await notify({ title: t("deleteEntry"), text: tError(e) });
    }
    history = await api.history();
  }

  const presetFilter = () => [{ name: t("presetFile"), extensions: ["json"] }];

  async function savePreset() {
    const path = await save({ title: t("saveRules"), filters: presetFilter(), defaultPath: t("defaultPresetName") });
    if (path) {
      await api.savePreset(path, payload());
      notice = t("rulesSaved");
    }
  }

  async function loadPreset() {
    const path = await open({ title: t("openRules"), filters: presetFilter() });
    if (typeof path !== "string") return;
    try {
      steps = (await api.loadPreset(path)).map((s) => newStep(s.rule, s.enabled));
      notice = t("rulesLoaded");
    } catch (e) {
      await notify({ title: t("openRules"), text: tError(e) });
    }
  }

  function onkeydown(e: KeyboardEvent) {
    // An open dialog takes the keyboard; Esc closes only it.
    if (dialog.current) {
      if (e.key === "Escape") closeDialog(false);
      return;
    }
    if (e.key === "Escape") {
      addMenu = ruleMenu = false;
      history = null;
      showOptions = false;
      help = null;
    }
    if (e.key === "F1") {
      e.preventDefault();
      help ??= "manual";
      return;
    }
    if (history || showOptions || help || busy) return;
    const mod = e.ctrlKey || e.metaKey;
    const key = e.key.toLowerCase();
    // These work even while typing in a rule.
    if (mod && key === "enter") {
      e.preventDefault();
      applyAll();
      return;
    }
    if (mod && key === "o" && !e.shiftKey) {
      e.preventDefault();
      pickFiles();
      return;
    }
    // Inside a text field, keys keep their usual meaning (Ctrl+Z undoes typing).
    if ((e.target as HTMLElement).closest("input, select, textarea")) return;
    if (e.key === "Delete") removeSelected();
    if (mod && key === "a") {
      e.preventDefault();
      selected = new Set(preview.items.map((i) => i.path));
    }
    if (mod && key === "z" && !e.shiftKey) {
      e.preventDefault();
      undoListEdit();
    }
    if (e.key === "F2" && selected.size === 1) {
      e.preventDefault();
      table?.edit([...selected][0]);
    }
  }

  function onpointerdown(e: PointerEvent) {
    if (!(e.target as Element).closest(".menu-host")) addMenu = ruleMenu = false;
  }

  onMount(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((e) => {
      dragging = e.payload.type === "enter" || e.payload.type === "over";
      if (e.payload.type === "drop") addPaths(e.payload.paths);
    });
    (async () => {
      await loadSettings();
      if (settings.rememberRules) steps = settings.rules.map((s) => newStep(s.rule, s.enabled));
      ready = true;
      await refresh();
    })();
    return () => {
      unlisten.then((f) => f());
    };
  });
</script>

<svelte:window {onkeydown} {onpointerdown} />

{#if ready}
<div class="app" class:busy>
  <header class="toolbar">
    <div class="group menu-host">
      <button class="primary-soft" onclick={() => (addMenu = !addMenu)}>{t("add")} ▾</button>
      {#if addMenu}
        <div class="menu">
          <button onclick={pickFiles}>{t("addFiles")}</button>
          <button onclick={() => pickFolders()}>{t("addFolders")}</button>
          <button onclick={() => pickFolders(true)}>{t("addFolderItems")}</button>
        </div>
      {/if}
    </div>
    <div class="group filter-box">
      <input
        class="filter"
        class:invalid={!!preview.filterError}
        type="search"
        bind:value={filter}
        placeholder={t("filterPlaceholder")}
        title={preview.filterError ? tError(preview.filterError) : t("filterTitle")}
        spellcheck="false"
      />
    </div>
    <div class="group">
      <button class="icon" title={canMove || !selected.size ? t("moveUp") : t("moveDisabled")} disabled={!canMove} onclick={() => move(-1)}>⤒</button>
      <button class="icon" title={canMove || !selected.size ? t("moveDown") : t("moveDisabled")} disabled={!canMove} onclick={() => move(1)}>⤓</button>
      <button disabled={!selected.size} onclick={removeSelected}>{t("remove")}</button>
      <button disabled={!preview.total} onclick={clearAll}>{t("clear")}</button>
    </div>
    <div class="spacer"></div>
    <div class="group">
      <button onclick={openHistory}>{t("history")}</button>
      <button class="icon help-btn" title="{t("help")} (F1)" aria-label={t("help")} onclick={() => (help = "manual")}>?</button>
      <button class="icon" title={t("options")} aria-label={t("options")} onclick={() => (showOptions = true)}>
        <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
          <path fill="currentColor" fill-rule="evenodd" stroke="currentColor" stroke-width="1" stroke-linejoin="round" d="M12.15 3.76L11.96 1.50L8.04 1.50L7.85 3.76L5.67 5.02L3.62 4.05L1.66 7.45L3.52 8.74L3.52 11.26L1.66 12.55L3.62 15.95L5.67 14.98L7.85 16.24L8.04 18.50L11.96 18.50L12.15 16.24L14.33 14.98L16.38 15.95L18.34 12.55L16.48 11.26L16.48 8.74L18.34 7.45L16.38 4.05L14.33 5.02ZM13 10a3 3 0 1 0-6 0a3 3 0 1 0 6 0Z" />
        </svg>
      </button>
      <button class="primary" title="{MOD}+Enter" disabled={!preview.changed || busy} onclick={applyAll}>
        {t("rename")} {preview.changed || ""}
      </button>
    </div>
  </header>

  <main>
    <aside class="rules">
      <div class="rules-head">
        <h2>{t("rules")}</h2>
        <div class="group menu-host">
          <button class="primary-soft" onclick={() => (ruleMenu = !ruleMenu)}>{t("addRule")}</button>
          {#if ruleMenu}
            <div class="menu">
              {#each ruleTypes as type (type)}
                <button onclick={() => addRule(type)}>{t(`rule.${type}`)}</button>
              {/each}
            </div>
          {/if}
          <button class="icon" title={t("openRules")} onclick={loadPreset}>📂</button>
          <button class="icon" title={t("saveRules")} disabled={!steps.length} onclick={savePreset}>💾</button>
        </div>
      </div>
      <div class="rule-list">
        {#each steps as step, i (step.id)}
          <RuleCard
            bind:step={steps[i]}
            error={ruleError(i)}
            {sample}
            first={i === 0}
            last={i === steps.length - 1}
            onmove={(d) => moveStep(i, d)}
            onremove={() => steps.splice(i, 1)}
          />
        {:else}
          <p class="empty">{t("emptyRules")}</p>
        {/each}
      </div>
    </aside>

    <section class="files" class:dragging>
      {#if preview.total}
        <FileTable
          bind:this={table}
          items={preview.items}
          bind:selected
          {sortKey}
          descending={sortDesc}
          onsort={sortBy}
          onoverride={setOverride}
        />
        {#if !preview.items.length}<p class="no-match">{t("noMatch")}</p>{/if}
      {:else}
        <div class="drop">
          <p class="big">{t("dropTitle")}</p>
          <p>{t("dropHint")}</p>
        </div>
      {/if}
    </section>
  </main>

  <footer>
    <span>
      {preview.items.length === preview.total
        ? t("files", { n: preview.total })
        : t("shownOf", { shown: preview.items.length, total: preview.total })}
    </span>
    <span class="ok">{t("changed", { n: preview.changed })}</span>
    {#if preview.conflicts}<span class="bad">{t("conflicts", { n: preview.conflicts })}</span>{/if}
    {#if selected.size}<span>{t("selected", { n: selected.size })}</span>{/if}
    <span class="spacer"></span>
    <span class="notice">{notice}</span>
  </footer>
</div>
{/if}

{#if history}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="overlay" onclick={() => (history = null)}>
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <h2>{t("historyTitle")}</h2>
      {#if history.length}
        <ul>
          {#each history as h (h.id)}
            <li>
              <span>{new Date(h.created).toLocaleString(LOCALES[settings.lang])}</span>
              <span class="muted">{t("files", { n: h.count })}</span>
              {#if h.undone}
                <span class="muted">{t("undone")}</span>
              {:else if h.failed || h.reverted}
                <span class="failed" title={t("undoFailedHint")}>
                  {h.reverted ? t("partlyUndone", { n: h.reverted, total: h.count }) : t("undoFailed")}
                </span>
                <button onclick={() => undo(h.id)} disabled={busy} title={t("undoFailedHint")}>{t("retry")}</button>
              {:else}
                <button onclick={() => undo(h.id)} disabled={busy}>{t("undo")}</button>
              {/if}
              <button class="icon danger" title={t("deleteEntry")} aria-label={t("deleteEntry")} onclick={() => deleteEntry(h.id)} disabled={busy}>✕</button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="muted">{t("noHistory")}</p>
      {/if}
      <div class="modal-foot"><button onclick={() => (history = null)}>{t("close")}</button></div>
    </div>
  </div>
{/if}

{#if help}
  <Help bind:tab={help} onclose={() => (help = null)} />
{/if}

{#if showOptions}
  <Options onclose={() => (showOptions = false)} />
{/if}

<Dialog />

<style>
  .app { display: flex; flex-direction: column; height: 100vh; }
  .app.busy { cursor: progress; }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    flex-wrap: wrap;
  }
  .group { display: flex; align-items: center; gap: 6px; }
  .menu-host { position: relative; }
  .spacer { flex: 1; }
  .filter-box { flex: 0 1 280px; }
  .filter { width: 100%; }
  .filter.invalid { border-color: var(--danger); outline-color: var(--danger); }
  main { flex: 1; display: flex; min-height: 0; gap: 10px; padding: 10px; }
  .rules { width: clamp(280px, 30vw, 340px); flex: none; display: flex; flex-direction: column; min-height: 0; }
  .rules-head { display: flex; align-items: center; justify-content: space-between; padding: 0 2px 8px; }
  h2 { margin: 0; font-size: 15px; }
  .rule-list { flex: 1; overflow-y: auto; overflow-x: hidden; display: flex; flex-direction: column; gap: 8px; padding-right: 2px; }
  .empty { color: var(--muted); font-size: 13px; margin: 4px 2px; }
  .menu {
    position: absolute;
    left: 0;
    top: calc(100% + 4px);
    min-width: 230px;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: var(--shadow);
    padding: 4px;
    z-index: 5;
  }
  .menu button { border: none; background: none; text-align: left; }
  .menu button:hover { background: var(--hover); }
  .files { flex: 1; display: flex; min-width: 0; border-radius: 8px; position: relative; }
  .files.dragging { outline: 2px dashed var(--accent); outline-offset: 2px; }
  .no-match { position: absolute; top: 60px; left: 0; right: 0; text-align: center; color: var(--muted); margin: 0; }
  .drop {
    flex: 1;
    display: grid;
    place-content: center;
    text-align: center;
    border: 2px dashed var(--border);
    border-radius: 8px;
    color: var(--muted);
  }
  .drop .big { font-size: 18px; color: var(--text); margin: 0 0 4px; }
  footer {
    display: flex;
    gap: 16px;
    padding: 6px 12px;
    font-size: 12px;
    border-top: 1px solid var(--border);
    background: var(--surface);
    color: var(--muted);
  }
  footer .ok { color: var(--accent); }
  footer .bad { color: var(--danger); }
  .modal ul { list-style: none; margin: 0; padding: 0; overflow-y: auto; }
  .modal li { display: flex; align-items: center; gap: 12px; padding: 6px 0; border-bottom: 1px solid var(--row-line); }
  .modal li > :first-child { flex: 1; }
  .muted { color: var(--muted); }
  .help-btn { font-size: 16px; font-weight: 700; }
  .failed { color: var(--danger); font-size: 13px; }
</style>
