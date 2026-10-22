<script lang="ts">
  import { onMount, tick } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { ask, message, open, save } from "@tauri-apps/plugin-dialog";
  import { api } from "$lib/api";
  import FileTable from "$lib/FileTable.svelte";
  import Options from "$lib/Options.svelte";
  import RuleCard from "$lib/RuleCard.svelte";
  import { LOCALES, t, tError } from "$lib/i18n";
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
  let addMenu = $state(false);
  let ruleMenu = $state(false);
  let table = $state<FileTable>();

  const view = $derived<View>({ filter, sortByNewName: sortKey === "newName", descending: sortDesc });
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

  async function addPaths(paths: string[]) {
    if (!paths.length) return;
    busy = true;
    try {
      const r = await api.addPaths(paths, { recursive: true, includeHidden: settings.includeHidden });
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

  async function pickFolders() {
    addMenu = false;
    const r = await open({ directory: true, multiple: true, title: t("addFolders") });
    if (r) await addPaths(Array.isArray(r) ? r : [r]);
  }

  async function removeSelected() {
    if (!selected.size) return;
    await api.removeFiles([...selected]);
    selected = new Set();
    await refresh();
  }

  async function clearAll() {
    await api.clearFiles();
    selected = new Set();
    overrides = {};
    await refresh();
  }

  async function move(delta: -1 | 1) {
    if (!canMove) return;
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

  function setOverride(path: string, name: string | null) {
    if (name === null) delete overrides[path];
    else overrides[path] = name;
  }

  const basename = (p: string) => p.split("/").pop() ?? p;
  const errText = (e: OpError) =>
    e.kind === "history"
      ? `${t("err.history")} (${e.detail})`
      : `${basename(e.from)}: ${t(`err.${e.kind}`)}${e.detail ? ` (${e.detail})` : ""}`;

  function summary(r: ApplyReport, doneKey: "renamedN" | "restoredN") {
    let text = t(doneKey, { n: r.done.length });
    if (r.errors.length) {
      const list = r.errors.slice(0, 8).map((e) => `• ${errText(e)}`).join("\n");
      text += `\n\n${t("errorsN", { n: r.errors.length })}:\n${list}` + (r.errors.length > 8 ? "\n…" : "");
    }
    return text;
  }

  async function applyAll() {
    if (!preview.changed) return;
    if (settings.confirmApply) {
      const extra = preview.conflicts ? `\n${t("confirmConflicts", { n: preview.conflicts })}` : "";
      const ok = await ask(`${t("confirmText", { n: preview.changed })}${extra}\n\n${t("confirmUndoHint")}`, {
        title: t("confirmTitle"),
        kind: "warning",
        okLabel: t("confirmOk"),
        cancelLabel: t("cancel"),
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
      if (r.errors.length) await message(summary(r, "renamedN"), { title: t("resultTitle"), kind: "warning" });
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
      await message(summary(r, "restoredN"), { title: t("undo"), kind: r.errors.length ? "warning" : "info" });
      history = await api.history();
      await refresh();
    } catch (e) {
      await message(tError(e), { title: t("undo"), kind: "error" });
    } finally {
      busy = false;
    }
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
      await message(tError(e), { title: t("openRules"), kind: "error" });
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      addMenu = ruleMenu = false;
      history = null;
      showOptions = false;
    }
    if ((e.target as HTMLElement).closest("input, select, textarea")) return;
    if (e.key === "Delete") removeSelected();
    if (e.key === "a" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      selected = new Set(preview.items.map((i) => i.path));
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
          <button onclick={pickFolders}>{t("addFolders")}</button>
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
      <button class="icon" title={t("options")} aria-label={t("options")} onclick={() => (showOptions = true)}>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" aria-hidden="true">
          <path d="M10.3 2.5h3.4l.5 2.6a7.5 7.5 0 0 1 1.9 1.1l2.5-.9 1.7 2.9-2 1.7a7.6 7.6 0 0 1 0 2.2l2 1.7-1.7 2.9-2.5-.9a7.5 7.5 0 0 1-1.9 1.1l-.5 2.6h-3.4l-.5-2.6a7.5 7.5 0 0 1-1.9-1.1l-2.5.9-1.7-2.9 2-1.7a7.6 7.6 0 0 1 0-2.2l-2-1.7 1.7-2.9 2.5.9a7.5 7.5 0 0 1 1.9-1.1z" />
          <circle cx="12" cy="12" r="3" />
        </svg>
      </button>
      <button class="primary" disabled={!preview.changed || busy} onclick={applyAll}>
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
              {#each RULE_TYPES as type (type)}
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
              {:else if h.reverted}
                <span class="muted">{t("partlyUndone", { n: h.reverted, total: h.count })}</span>
                <button onclick={() => undo(h.id)} disabled={busy}>{t("undoRetry")}</button>
              {:else}
                <button onclick={() => undo(h.id)} disabled={busy}>{t("undo")}</button>
              {/if}
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

{#if showOptions}
  <Options onclose={() => (showOptions = false)} />
{/if}

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
  .rules { width: 340px; flex: none; display: flex; flex-direction: column; min-height: 0; }
  .rules-head { display: flex; align-items: center; justify-content: space-between; padding: 0 2px 8px; }
  h2 { margin: 0; font-size: 15px; }
  .rule-list { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 8px; padding-right: 2px; }
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
  .modal-foot { display: flex; justify-content: flex-end; }
</style>
