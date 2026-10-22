<script lang="ts">
  import { LANG_NAMES, MOD, t, type Key } from "./i18n";
  import { applyTheme, settings, type Lang } from "./settings.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const shortcuts: [string, Key][] = [
    [`${MOD}+Enter`, "kRename"],
    [`${MOD}+Z`, "kUndo"],
    [`${MOD}+O`, "kAddFiles"],
    ["F2", "kEdit"],
    ["Delete", "kRemove"],
    [`${MOD}+A`, "kSelectAll"],
    ["Esc", "kClose"],
  ];
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="overlay" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="modal" onclick={(e) => e.stopPropagation()}>
    <h2>{t("optionsTitle")}</h2>

    <section>
      <h3>{t("sectionGeneral")}</h3>
      <div class="grid">
        <label for="opt-lang">{t("language")}</label>
        <select id="opt-lang" bind:value={settings.lang}>
          {#each Object.entries(LANG_NAMES) as [code, name] (code)}
            <option value={code as Lang}>{name}</option>
          {/each}
        </select>

        <span>{t("theme")}</span>
        <div class="seg">
          {#each [["light", "themeLight"], ["dark", "themeDark"], ["system", "themeSystem"]] as const as [value, label] (value)}
            <button class:on={settings.theme === value} onclick={() => { settings.theme = value; applyTheme(); }}>{t(label)}</button>
          {/each}
        </div>
      </div>
      <label class="check">
        <input type="checkbox" bind:checked={settings.rememberRules} />
        <span>{t("rememberRules")}</span>
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={settings.includeHidden} />
        <span>{t("includeHidden")}<small>{t("includeHiddenHint")}</small></span>
      </label>
    </section>

    <section>
      <h3>{t("sectionRename")}</h3>
      <label class="check">
        <input type="checkbox" bind:checked={settings.confirmApply} />
        <span>{t("confirmApply")}</span>
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={settings.disableRulesAfterApply} />
        <span>{t("disableAfterApply")}<small>{t("disableAfterApplyHint")}</small></span>
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={settings.warnWindows} />
        <span>{t("warnWindows")}</span>
      </label>
    </section>

    <section>
      <h3>{t("sectionShortcuts")}</h3>
      <dl class="keys">
        {#each shortcuts as [keys, label] (label)}
          <dt><kbd>{keys}</kbd></dt>
          <dd>{t(label)}</dd>
        {/each}
      </dl>
    </section>

    <div class="foot"><button class="primary" onclick={onclose}>{t("close")}</button></div>
  </div>
</div>

<style>
  h2 { margin: 0; font-size: 16px; }
  h3 { margin: 0 0 8px; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
  section { display: flex; flex-direction: column; gap: 10px; }
  .grid { display: grid; grid-template-columns: 90px 1fr; align-items: center; gap: 10px; }
  .seg { display: inline-flex; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; width: fit-content; }
  .seg button { border: none; border-radius: 0; height: 28px; }
  .seg button + button { border-left: 1px solid var(--border); }
  .seg button.on { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .check { display: flex; align-items: flex-start; gap: 8px; }
  .check input { margin-top: 2px; }
  .check span { display: flex; flex-direction: column; }
  small { color: var(--muted); font-size: 12px; }
  .foot { display: flex; justify-content: flex-end; }
  .keys { display: grid; grid-template-columns: auto 1fr; gap: 6px 12px; margin: 0; align-items: center; }
  dd { margin: 0; }
  kbd {
    font: 12px ui-monospace, monospace;
    padding: 1px 6px;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 4px;
    background: var(--surface-2);
    white-space: nowrap;
  }
</style>
