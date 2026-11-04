<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "./api";
  import { LANG_NAMES, MOD, t, tError, type Key } from "./i18n";
  import { applyTheme, loadSecrets, secrets, settings, type Lang } from "./settings.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const TMDB_LANGS = ["pt-BR", "pt-PT", "en-US", "en-GB", "es-ES", "es-MX", "fr-FR", "it-IT", "de-DE", "ja-JP"];
  let showKey = $state(false);
  let keyStatus = $state<{ ok: boolean; text: string } | null>(null);
  let testing = $state(false);

  let showDgKey = $state(false);
  let dgStatus = $state<{ ok: boolean; text: string } | null>(null);
  let dgTesting = $state(false);

  async function testDgKey() {
    dgTesting = true;
    dgStatus = null;
    try {
      await api.discogsCheck(settings.discogsToken.trim());
      dgStatus = { ok: true, text: t("tmdbKeyOk") };
    } catch (e) {
      dgStatus = { ok: false, text: tError(e) };
    } finally {
      dgTesting = false;
    }
  }

  let showGcd = $state(false);
  let gcdStatus = $state<{ ok: boolean; text: string } | null>(null);
  let gcdTesting = $state(false);

  async function testGcd() {
    gcdTesting = true;
    gcdStatus = null;
    try {
      await api.gcdCheck(settings.gcdEmail.trim(), settings.gcdPassword);
      gcdStatus = { ok: true, text: t("gcdAccountOk") };
    } catch (e) {
      gcdStatus = { ok: false, text: tError(e) };
    } finally {
      gcdTesting = false;
    }
  }

  let showMtKey = $state(false);
  let mtStatus = $state<{ ok: boolean; text: string } | null>(null);
  let mtTesting = $state(false);

  async function testMtKey() {
    mtTesting = true;
    mtStatus = null;
    try {
      await api.metronCheck(settings.metronToken.trim());
      mtStatus = { ok: true, text: t("tmdbKeyOk") };
    } catch (e) {
      mtStatus = { ok: false, text: tError(e) };
    } finally {
      mtTesting = false;
    }
  }

  let showCvKey = $state(false);
  let cvStatus = $state<{ ok: boolean; text: string } | null>(null);
  let cvTesting = $state(false);

  async function testCvKey() {
    cvTesting = true;
    cvStatus = null;
    try {
      await api.comicCheck(settings.comicVineKey.trim());
      cvStatus = { ok: true, text: t("tmdbKeyOk") };
    } catch (e) {
      cvStatus = { ok: false, text: tError(e) };
    } finally {
      cvTesting = false;
    }
  }

  async function testKey() {
    testing = true;
    keyStatus = null;
    try {
      await api.tmdbCheck(settings.tmdbKey);
      keyStatus = { ok: true, text: t("tmdbKeyOk") };
    } catch (e) {
      keyStatus = { ok: false, text: tError(e) };
    } finally {
      testing = false;
    }
  }

  let cacheBytes = $state<number | null>(null);
  api.cacheSize().then((n) => (cacheBytes = n)).catch(() => {});

  async function clearCache() {
    await api.clearCache().catch(() => 0);
    cacheBytes = await api.cacheSize().catch(() => 0);
  }

  function size(bytes: number): string {
    if (bytes < 1024 * 1024) return `${Math.ceil(bytes / 1024)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }

  const shortcuts: [string, Key][] = $derived([
    [`${MOD}+Enter`, "kRename"],
    [`${MOD}+Z`, "kUndo"],
    [`${MOD}+O`, "kAddFiles"],
    ["F2", "kEdit"],
    [t("keyDelete"), "kRemove"],
    [`${MOD}+A`, "kSelectAll"],
    ["F1", "kHelp"],
    ["Esc", "kClose"],
  ]);
</script>

<div class="overlay">
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
      <h3>{t("sectionOnline")}</h3>
      <div class="grid">
        <label for="opt-cache">{t("cacheDays")}</label>
        <select id="opt-cache" bind:value={settings.cacheDays}>
          <option value={0}>{t("cacheOff")}</option>
          <option value={1}>{t("cache1")}</option>
          <option value={7}>{t("cache7")}</option>
          <option value={30}>{t("cache30")}</option>
          <option value={180}>{t("cache180")}</option>
        </select>
      </div>
      <small>{t("cacheHint")}</small>
      <div class="cache">
        <small>{cacheBytes === null ? "…" : t("cacheSize").replace("{size}", size(cacheBytes))}</small>
        <button onclick={clearCache} disabled={!cacheBytes}>{t("cacheClear")}</button>
      </div>
    </section>

    <section>
      <h3>{t("sectionTmdb")}</h3>
      <div class="grid">
        <label for="opt-tmdb-key">{t("tmdbKey")}</label>
        <div class="key">
          <input
            id="opt-tmdb-key"
            type={showKey ? "text" : "password"}
            bind:value={settings.tmdbKey}
            disabled={!secrets.ready}
            oninput={() => (keyStatus = null)}
            spellcheck="false"
            autocomplete="off"
          />
          <button onclick={() => (showKey = !showKey)}>{t(showKey ? "hide" : "show")}</button>
          <button disabled={!settings.tmdbKey.trim() || testing} onclick={testKey}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if keyStatus}<b class:good={keyStatus.ok} class:bad={!keyStatus.ok}>{keyStatus.text}</b> · {/if}
          {t("tmdbKeyHint")}
          <button class="link" onclick={() => openUrl("https://www.themoviedb.org/settings/api")}>{t("tmdbGetKey")}</button>
        </small>

        <label for="opt-tmdb-lang">{t("tmdbLangLabel")}</label>
        <select id="opt-tmdb-lang" bind:value={settings.tmdbLang}>
          <option value="">{t("tmdbLangAuto")}</option>
          {#each TMDB_LANGS as code (code)}<option value={code}>{code}</option>{/each}
        </select>
      </div>
    </section>

    <section>
      <h3>{t("sectionComicVine")}</h3>
      <div class="grid">
        <label for="opt-cv-key">{t("tmdbKey")}</label>
        <div class="key">
          <input
            id="opt-cv-key"
            type={showCvKey ? "text" : "password"}
            bind:value={settings.comicVineKey}
            disabled={!secrets.ready}
            oninput={() => (cvStatus = null)}
            spellcheck="false"
            autocomplete="off"
          />
          <button onclick={() => (showCvKey = !showCvKey)}>{t(showCvKey ? "hide" : "show")}</button>
          <button disabled={!settings.comicVineKey.trim() || cvTesting} onclick={testCvKey}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if cvStatus}<b class:good={cvStatus.ok} class:bad={!cvStatus.ok}>{cvStatus.text}</b> · {/if}
          {t("cvKeyHint")}
          <button class="link" onclick={() => openUrl("https://comicvine.gamespot.com/api/")}>{t("tmdbGetKey")}</button>
        </small>
      </div>
    </section>

    <section>
      <h3>{t("sectionMetron")}</h3>
      <div class="grid">
        <label for="opt-mt-key">{t("mtToken")}</label>
        <div class="key">
          <input
            id="opt-mt-key"
            type={showMtKey ? "text" : "password"}
            bind:value={settings.metronToken}
            disabled={!secrets.ready}
            oninput={() => (mtStatus = null)}
            spellcheck="false"
            autocomplete="off"
          />
          <button onclick={() => (showMtKey = !showMtKey)}>{t(showMtKey ? "hide" : "show")}</button>
          <button disabled={!settings.metronToken.trim() || mtTesting} onclick={testMtKey}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if mtStatus}<b class:good={mtStatus.ok} class:bad={!mtStatus.ok}>{mtStatus.text}</b> · {/if}
          {t("mtKeyHint")}
          <button class="link" onclick={() => openUrl("https://metron.cloud")}>{t("mtGetKey")}</button>
        </small>
      </div>
    </section>

    <section>
      <h3>Grand Comics Database</h3>
      <div class="grid">
        <label for="opt-gcd-email">{t("gcdEmail")}</label>
        <input
          id="opt-gcd-email"
          type="email"
          bind:value={settings.gcdEmail}
            disabled={!secrets.ready}
          oninput={() => (gcdStatus = null)}
          spellcheck="false"
          autocomplete="off"
        />
        <label for="opt-gcd-password">{t("gcdPassword")}</label>
        <div class="key">
          <input
            id="opt-gcd-password"
            type={showGcd ? "text" : "password"}
            bind:value={settings.gcdPassword}
            disabled={!secrets.ready}
            oninput={() => (gcdStatus = null)}
            autocomplete="off"
          />
          <button onclick={() => (showGcd = !showGcd)}>{t(showGcd ? "hide" : "show")}</button>
          <button disabled={!settings.gcdEmail.trim() || !settings.gcdPassword || gcdTesting} onclick={testGcd}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if gcdStatus}<b class:good={gcdStatus.ok} class:bad={!gcdStatus.ok}>{gcdStatus.text}</b> · {/if}
          {t("gcdAccountHint")}
          <button class="link" onclick={() => openUrl("https://www.comics.org/accounts/register/")}>{t("gcdRegister")}</button>
        </small>
      </div>
    </section>

    <section>
      <h3>Discogs</h3>
      <div class="grid">
        <label for="opt-dg-key">{t("dgToken")}</label>
        <div class="key">
          <input
            id="opt-dg-key"
            type={showDgKey ? "text" : "password"}
            bind:value={settings.discogsToken}
            disabled={!secrets.ready}
            oninput={() => (dgStatus = null)}
            spellcheck="false"
            autocomplete="off"
          />
          <button onclick={() => (showDgKey = !showDgKey)}>{t(showDgKey ? "hide" : "show")}</button>
          <button disabled={!settings.discogsToken.trim() || dgTesting} onclick={testDgKey}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if dgStatus}<b class:good={dgStatus.ok} class:bad={!dgStatus.ok}>{dgStatus.text}</b> · {/if}
          {t("dgKeyHint")}
          <button class="link" onclick={() => openUrl("https://www.discogs.com/settings/developers")}>{t("dgGetKey")}</button>
        </small>
      </div>
    </section>

    <p class="stored" class:bad={secrets.ready && secrets.status !== "stored"}>
      {#if !secrets.ready}
        {t("secretsWaiting")}
      {:else if secrets.status === "stored"}
        {t("secretsInStore")}
      {:else if secrets.status === "refused"}
        {t("secretsRefused")}
        <button class="link" onclick={() => loadSecrets()}>{t("secretsRetry")}</button>
      {:else}
        {t("secretsInFile")}
      {/if}
    </p>

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
  .grid { display: grid; grid-template-columns: 120px 1fr; align-items: center; gap: 10px; }
  .seg { display: inline-flex; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; width: fit-content; }
  .seg button { border: none; border-radius: 0; height: 28px; }
  .seg button + button { border-left: 1px solid var(--border); }
  .seg button.on { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .check { display: flex; align-items: flex-start; gap: 8px; }
  .check input { margin-top: 2px; }
  .check span { display: flex; flex-direction: column; }
  small { color: var(--muted); font-size: 12px; }
  .stored { margin: 0 0 16px; font-size: 12px; color: var(--muted); }
  .foot { display: flex; justify-content: flex-end; }
  .cache { display: flex; align-items: center; gap: 10px; }
  .key { display: flex; gap: 6px; }
  .key input { flex: 1; }
  .good { color: #138a4e; }
  .bad { color: var(--danger); }
  .link { border: none; background: none; height: auto; padding: 0; color: var(--accent); font-size: 12px; }
  .link:hover:not(:disabled) { background: none; text-decoration: underline; }
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
