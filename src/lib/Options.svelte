<script lang="ts">
  import Icon from "./Icon.svelte";
  import ServerOptions from "./ServerOptions.svelte";
  import { dialog } from "./modal";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "./api";
  import { TABLE } from "./rules";
  import { LANG_NAMES, MOD, said, t, tError, type Key } from "./i18n";
  import { applyTextSize, applyTheme, loadSecrets, secrets, settings, type Lang } from "./settings.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const TABS = [
    ["general", "sectionGeneral"],
    ["rename", "sectionRename"],
    ["online", "sectionOnline"],
    ["accounts", "optTabAccounts"],
    ["server", "sectionServer"],
    ["shortcuts", "sectionShortcuts"],
  ] as const;
  let tab = $state<(typeof TABS)[number][0]>("general");
  /** Services with a key or account typed in. */
  const accountsSet = $derived(
    [settings.tmdbKey, settings.osKey || settings.subdlKey || settings.subsourceKey, settings.googleBooksKey, settings.comicVineKey, settings.metronToken, settings.gcdEmail, settings.discogsToken].filter((v) => v.trim()).length,
  );

  /** An option's menu (the Rust options table's, the command line's too). */
  const words = (name: string) => (TABLE.options.menus[name] ?? []) as { value: string; label: string }[];
  const numbers = (name: string) => (TABLE.options.menus[name] ?? []) as number[];
  /** The TMDB languages offered (empty: the interface's, shown apart). */
  const TMDB_LANGS = words("tmdbLang").map((c) => c.value).filter(Boolean);
  let showKey = $state(false);

  /** The Flatpak is updated by Flatpak: no option for it. */
  let updateWay = $state<"itself" | "page" | "system">("itself");
  api.updateWay().then((w) => (updateWay = w)).catch(() => {});

  let showDgKey = $state(false);

  let showOs = $state(false);

  let showSubdl = $state(false);
  let showSubsource = $state(false);

  let showGcd = $state(false);

  let showGbKey = $state(false);

  let showMtKey = $state(false);

  let showCvKey = $state(false);

  /** What each Test button said (by service), and those still asking:
   *  the shared check says (`checks::test`, the command line's too). */
  let status = $state<Record<string, { ok: boolean; text: string } | null>>({});
  let testing = $state<Record<string, boolean>>({});
  async function check(name: string) {
    testing[name] = true;
    status[name] = null;
    const values = Object.fromEntries(Object.entries(settings).filter((e): e is [string, string] => typeof e[1] === "string"));
    try {
      status[name] = { ok: true, text: said([await api.accountCheck(name, values)])[0] };
    } catch (e) {
      status[name] = { ok: false, text: tError(e) };
    } finally {
      testing[name] = false;
    }
  }

  /** The kept answers' size: bytes, and as shown (Rust's). */
  let cache = $state<[number, string] | null>(null);
  api.cacheSize().then((n) => (cache = n)).catch(() => {});

  async function clearCache() {
    await api.clearCache().catch(() => 0);
    cache = await api.cacheSize().catch((): [number, string] => [0, ""]);
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
  <div class="modal options" use:dialog onclick={(e) => e.stopPropagation()}>
    <header class="modal-head">
      <h2>{t("optionsTitle")}</h2>
      <button class="icon close" title={t("close")} aria-label={t("close")} onclick={onclose}><Icon name="x" size={18} /></button>
    </header>

    <div class="opt-body">
      <nav class="opt-nav" aria-label={t("optionsTitle")}>
        {#each TABS as [id, label] (id)}
          <button class:on={tab === id} aria-current={tab === id ? "page" : undefined} onclick={() => (tab = id)}>{t(label)}{#if id === "accounts"}<span class="n">{accountsSet}/7</span>{/if}</button>
        {/each}
      </nav>
      <div class="opt-pane">
      {#if tab === "general"}
    <section>
      <h3>{t("sectionGeneral")}</h3>
      <div class="grid">
        <label for="opt-lang">{t("language")}</label>
        <select id="opt-lang" bind:value={settings.lang}>
          {#each words("lang") as { value } (value)}
            <option value={value as Lang}>{LANG_NAMES[value as Lang]}</option>
          {/each}
        </select>

        <span>{t("theme")}</span>
        <div class="seg">
          {#each words("theme") as { value, label } (value)}
            <button class:on={settings.theme === value} onclick={() => { settings.theme = value as typeof settings.theme; applyTheme(); }}>{t(label as Key)}</button>
          {/each}
        </div>

        <span>{t("textSize")}</span>
        <div class="seg">
          {#each words("textSize") as { value, label } (value)}
            <button class:on={settings.textSize === value} onclick={() => { settings.textSize = value as typeof settings.textSize; applyTextSize(); }}>{t(label as Key)}</button>
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
      {#if updateWay !== "system"}
      <label class="check">
        <input type="checkbox" bind:checked={settings.checkUpdates} />
        <span>{t("checkUpdates")}<small>{t("checkUpdatesHint")}</small></span>
      </label>
      {/if}
    </section>
      {/if}
      {#if tab === "rename"}
    <section>
      <h3>{t("sectionRename")}</h3>
      <label class="check">
        <input type="checkbox" bind:checked={settings.confirmApply} />
        <span>{t("confirmApply")}</span>
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={settings.numberClashes} />
        <span>{t("numberClashes")}<small>{t("numberClashesHint")}</small></span>
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
      {/if}
      {#if tab === "online"}
    <section>
      <h3>{t("sectionOnline")}</h3>
      <div class="grid">
        <label for="opt-cache">{t("cacheDays")}</label>
        <select id="opt-cache" bind:value={settings.cacheDays}>
          {#each numbers("cacheDays") as n (n)}<option value={n}>{t(n ? (`cache${n}` as Key) : "cacheOff")}</option>{/each}
        </select>
      </div>
      <small>{t("cacheHint")}</small>
      <div class="cache">
        <small>{cache === null ? "…" : t("cacheSize", { size: cache[1] })}</small>
        <button onclick={clearCache} disabled={!cache?.[0]}>{t("cacheClear")}</button>
      </div>
    </section>
      {/if}
      {#if tab === "accounts"}
    <section>
      <h3>{t("sectionTmdb")}{#if settings.tmdbKey.trim()}<span class="set"><Icon name="check" size={12} />{t("keySet")}</span>{/if}</h3>
      <div class="grid">
        <label for="opt-tmdb-key">{t("tmdbKey")}</label>
        <div class="key">
          <input
            id="opt-tmdb-key"
            type={showKey ? "text" : "password"}
            bind:value={settings.tmdbKey}
            disabled={!secrets.ready}
            oninput={() => (status["tmdb"] = null)}
            spellcheck="false"
            autocomplete="off"
          />
          <button onclick={() => (showKey = !showKey)}>{t(showKey ? "hide" : "show")}</button>
          <button disabled={!settings.tmdbKey.trim() || testing["tmdb"]} onclick={() => check("tmdb")}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if status["tmdb"]}{@const st = status["tmdb"]}<b class:good={st.ok} class:bad={!st.ok}>{st.text}</b> · {/if}
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
      <h3>{t("sectionSubs")}{#if settings.osKey.trim() || settings.subdlKey.trim() || settings.subsourceKey.trim()}<span class="set"><Icon name="check" size={12} />{t("keySet")}</span>{/if}</h3>
      <p class="hint-top"><small>{t("subsOptionsHint")}</small></p>
      <h4>OpenSubtitles.com</h4>
      <div class="grid">
        <label for="opt-os-key">{t("tmdbKey")}</label>
        <div class="key">
          <input
            id="opt-os-key"
            type={showOs ? "text" : "password"}
            bind:value={settings.osKey}
            disabled={!secrets.ready}
            oninput={() => (status["opensubtitles"] = null)}
            spellcheck="false"
            autocomplete="off"
          />
          <button onclick={() => (showOs = !showOs)}>{t(showOs ? "hide" : "show")}</button>
        </div>
        <label for="opt-os-user">{t("osUser")}</label>
        <input
          id="opt-os-user"
          bind:value={settings.osUser}
          disabled={!secrets.ready}
          oninput={() => (status["opensubtitles"] = null)}
          spellcheck="false"
          autocomplete="off"
        />
        <label for="opt-os-password">{t("gcdPassword")}</label>
        <div class="key">
          <input
            id="opt-os-password"
            type={showOs ? "text" : "password"}
            bind:value={settings.osPassword}
            disabled={!secrets.ready}
            oninput={() => (status["opensubtitles"] = null)}
            autocomplete="off"
          />
          <button disabled={!settings.osKey.trim() || testing["opensubtitles"]} onclick={() => check("opensubtitles")}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if status["opensubtitles"]}{@const st = status["opensubtitles"]}<b class:good={st.ok} class:bad={!st.ok}>{st.text}</b> · {/if}
          {t("osKeyHint")}
          <button class="link" onclick={() => openUrl("https://www.opensubtitles.com/consumers")}>{t("osGetKey")}</button>
        </small>
      </div>
      <h4>SubDL</h4>
      <div class="grid">
        <label for="opt-subdl-key">{t("tmdbKey")}</label>
        <div class="key">
          <input
            id="opt-subdl-key"
            type={showSubdl ? "text" : "password"}
            bind:value={settings.subdlKey}
            disabled={!secrets.ready}
            oninput={() => (status["subdl"] = null)}
            spellcheck="false"
            autocomplete="off"
          />
          <button onclick={() => (showSubdl = !showSubdl)}>{t(showSubdl ? "hide" : "show")}</button>
          <button disabled={!settings.subdlKey.trim() || testing["subdl"]} onclick={() => check("subdl")}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if status["subdl"]}{@const st = status["subdl"]}<b class:good={st.ok} class:bad={!st.ok}>{st.text}</b> · {/if}
          {t("subdlKeyHint")}
          <button class="link" onclick={() => openUrl("https://subdl.com/panel/api")}>{t("subdlGetKey")}</button>
        </small>
      </div>
      <h4>SubSource</h4>
      <div class="grid">
        <label for="opt-subsource-key">{t("tmdbKey")}</label>
        <div class="key">
          <input
            id="opt-subsource-key"
            type={showSubsource ? "text" : "password"}
            bind:value={settings.subsourceKey}
            disabled={!secrets.ready}
            oninput={() => (status["subsource"] = null)}
            spellcheck="false"
            autocomplete="off"
          />
          <button onclick={() => (showSubsource = !showSubsource)}>{t(showSubsource ? "hide" : "show")}</button>
          <button disabled={!settings.subsourceKey.trim() || testing["subsource"]} onclick={() => check("subsource")}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if status["subsource"]}{@const st = status["subsource"]}<b class:good={st.ok} class:bad={!st.ok}>{st.text}</b> · {/if}
          {t("subsourceKeyHint")}
          <button class="link" onclick={() => openUrl("https://subsource.net/api-docs")}>{t("subdlGetKey")}</button>
        </small>
      </div>
    </section>

    <section>
      <h3>{t("sectionGoogleBooks")}{#if settings.googleBooksKey.trim()}<span class="set"><Icon name="check" size={12} />{t("keySet")}</span>{/if}</h3>
      <div class="grid">
        <label for="opt-gb-key">{t("tmdbKey")}</label>
        <div class="key">
          <input
            id="opt-gb-key"
            type={showGbKey ? "text" : "password"}
            bind:value={settings.googleBooksKey}
            disabled={!secrets.ready}
            oninput={() => (status["google-books"] = null)}
            spellcheck="false"
            autocomplete="off"
          />
          <button onclick={() => (showGbKey = !showGbKey)}>{t(showGbKey ? "hide" : "show")}</button>
          <button disabled={!settings.googleBooksKey.trim() || testing["google-books"]} onclick={() => check("google-books")}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if status["google-books"]}{@const st = status["google-books"]}<b class:good={st.ok} class:bad={!st.ok}>{st.text}</b> · {/if}
          {t("gbKeyHint")}
          <button class="link" onclick={() => openUrl("https://developers.google.com/books/docs/v1/using#APIKey")}>{t("tmdbGetKey")}</button>
        </small>
      </div>
    </section>

    <section>
      <h3>{t("sectionComicVine")}{#if settings.comicVineKey.trim()}<span class="set"><Icon name="check" size={12} />{t("keySet")}</span>{/if}</h3>
      <div class="grid">
        <label for="opt-cv-key">{t("tmdbKey")}</label>
        <div class="key">
          <input
            id="opt-cv-key"
            type={showCvKey ? "text" : "password"}
            bind:value={settings.comicVineKey}
            disabled={!secrets.ready}
            oninput={() => (status["comicvine"] = null)}
            spellcheck="false"
            autocomplete="off"
          />
          <button onclick={() => (showCvKey = !showCvKey)}>{t(showCvKey ? "hide" : "show")}</button>
          <button disabled={!settings.comicVineKey.trim() || testing["comicvine"]} onclick={() => check("comicvine")}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if status["comicvine"]}{@const st = status["comicvine"]}<b class:good={st.ok} class:bad={!st.ok}>{st.text}</b> · {/if}
          {t("cvKeyHint")}
          <button class="link" onclick={() => openUrl("https://comicvine.gamespot.com/api/")}>{t("tmdbGetKey")}</button>
        </small>
      </div>
    </section>

    <section>
      <h3>{t("sectionMetron")}{#if settings.metronToken.trim()}<span class="set"><Icon name="check" size={12} />{t("keySet")}</span>{/if}</h3>
      <div class="grid">
        <label for="opt-mt-key">{t("mtToken")}</label>
        <div class="key">
          <input
            id="opt-mt-key"
            type={showMtKey ? "text" : "password"}
            bind:value={settings.metronToken}
            disabled={!secrets.ready}
            oninput={() => (status["metron"] = null)}
            spellcheck="false"
            autocomplete="off"
          />
          <button onclick={() => (showMtKey = !showMtKey)}>{t(showMtKey ? "hide" : "show")}</button>
          <button disabled={!settings.metronToken.trim() || testing["metron"]} onclick={() => check("metron")}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if status["metron"]}{@const st = status["metron"]}<b class:good={st.ok} class:bad={!st.ok}>{st.text}</b> · {/if}
          {t("mtKeyHint")}
          <button class="link" onclick={() => openUrl("https://metron.cloud")}>{t("mtGetKey")}</button>
        </small>
      </div>
    </section>

    <section>
      <h3>Grand Comics Database{#if settings.gcdEmail.trim()}<span class="set"><Icon name="check" size={12} />{t("keySet")}</span>{/if}</h3>
      <div class="grid">
        <label for="opt-gcd-email">{t("gcdEmail")}</label>
        <input
          id="opt-gcd-email"
          type="email"
          bind:value={settings.gcdEmail}
            disabled={!secrets.ready}
          oninput={() => (status["gcd"] = null)}
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
            oninput={() => (status["gcd"] = null)}
            autocomplete="off"
          />
          <button onclick={() => (showGcd = !showGcd)}>{t(showGcd ? "hide" : "show")}</button>
          <button disabled={!settings.gcdEmail.trim() || !settings.gcdPassword || testing["gcd"]} onclick={() => check("gcd")}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if status["gcd"]}{@const st = status["gcd"]}<b class:good={st.ok} class:bad={!st.ok}>{st.text}</b> · {/if}
          {t("gcdAccountHint")}
          <button class="link" onclick={() => openUrl("https://www.comics.org/accounts/register/")}>{t("gcdRegister")}</button>
        </small>
      </div>
    </section>

    <section>
      <h3>Discogs{#if settings.discogsToken.trim()}<span class="set"><Icon name="check" size={12} />{t("keySet")}</span>{/if}</h3>
      <div class="grid">
        <label for="opt-dg-key">{t("dgToken")}</label>
        <div class="key">
          <input
            id="opt-dg-key"
            type={showDgKey ? "text" : "password"}
            bind:value={settings.discogsToken}
            disabled={!secrets.ready}
            oninput={() => (status["discogs"] = null)}
            spellcheck="false"
            autocomplete="off"
          />
          <button onclick={() => (showDgKey = !showDgKey)}>{t(showDgKey ? "hide" : "show")}</button>
          <button disabled={!settings.discogsToken.trim() || testing["discogs"]} onclick={() => check("discogs")}>{t("tmdbTest")}</button>
        </div>
        <span></span>
        <small>
          {#if status["discogs"]}{@const st = status["discogs"]}<b class:good={st.ok} class:bad={!st.ok}>{st.text}</b> · {/if}
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
      {/if}
      {#if tab === "server"}
        <ServerOptions />
      {/if}
      {#if tab === "shortcuts"}
    <section>
      <h3>{t("sectionShortcuts")}</h3>
      <dl class="keys">
        {#each shortcuts as [keys, label] (label)}
          <dt><kbd>{keys}</kbd></dt>
          <dd>{t(label)}</dd>
        {/each}
      </dl>
    </section>
      {/if}
      </div>
    </div>

    <div class="foot"><button class="primary" onclick={onclose}>{t("close")}</button></div>
  </div>
</div>

<style>
  .options { width: min(820px, 94vw); height: min(640px, 88vh); max-height: none; overflow: hidden; }
  .opt-body { flex: 1; min-height: 0; display: grid; grid-template-columns: 180px 1fr; gap: 18px; }
  .opt-nav { display: flex; flex-direction: column; gap: 2px; padding-right: 12px; border-right: 1px solid var(--row-line); }
  .opt-nav button { display: flex; align-items: center; justify-content: space-between; border: none; background: none; text-align: left; color: var(--muted); }
  .opt-nav button:hover:not(:disabled) { color: var(--text); }
  .opt-nav button.on { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .opt-nav .n { font-size: var(--fs-xs); font-variant-numeric: tabular-nums; opacity: 0.8; }
  /* A pane of one section: the menu already names it. */
  .opt-pane > section:only-of-type > h3:first-child { display: none; }
  .opt-pane { min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 22px; padding-right: 6px; }
  .set { display: inline-flex; align-items: center; gap: 3px; margin-left: 8px; padding: 0 6px; border-radius: 9px; background: var(--success-soft); color: var(--success); font-size: var(--fs-xs); letter-spacing: 0; text-transform: none; vertical-align: 1px; }
  h2 { margin: 0; font-size: var(--fs-lg); }
  h3 { margin: 0 0 8px; font-size: var(--fs-sm); text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
  section { display: flex; flex-direction: column; gap: 10px; }
  h4 { margin: 4px 0 0; font-size: var(--fs-md); }
  .hint-top { margin: 0; }
  .grid { display: grid; grid-template-columns: 140px 1fr; align-items: center; gap: 10px; }
  .seg { display: inline-flex; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; width: fit-content; }
  .seg button { border: none; border-radius: 0; height: 28px; }
  .seg button + button { border-left: 1px solid var(--border); }
  .seg button.on { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .check { display: flex; align-items: flex-start; gap: 8px; }
  .check input { margin-top: 2px; }
  .check span { display: flex; flex-direction: column; }
  small { color: var(--muted); font-size: var(--fs-sm); }
  .stored { margin: 0 0 16px; font-size: var(--fs-sm); color: var(--muted); }
  .foot { display: flex; justify-content: flex-end; }
  .cache { display: flex; align-items: center; gap: 10px; }
  .key { display: flex; gap: 6px; }
  .key input { flex: 1; }
  .good { color: var(--success); }
  .bad { color: var(--danger); }
  .link { border: none; background: none; height: auto; padding: 0; color: var(--accent); font-size: var(--fs-sm); }
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
