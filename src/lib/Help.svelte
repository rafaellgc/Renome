<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { en } from "./help/en";
  import { es } from "./help/es";
  import { pt } from "./help/pt";
  import { t } from "./i18n";
  import { settings } from "./settings.svelte";
  import appLicense from "../../LICENSE?raw";

  type Tab = "manual" | "about" | "licenses";
  let { onclose, tab = $bindable("manual") }: { onclose: () => void; tab?: Tab } = $props();

  /** Written by scripts/licenses.js when building. */
  interface Licenses {
    packages: { name: string; version: string; license: string; url: string; kind: "rust" | "npm"; texts: number[] }[];
    texts: string[];
  }
  /** null: loading; false: not there (development build). */
  let licenses = $state<Licenses | null | false>(null);
  let licenseQuery = $state("");
  let expanded = $state<string | null>(null);
  const shownPackages = $derived.by(() => {
    if (!licenses) return [];
    const q = licenseQuery.trim().toLowerCase();
    return q ? licenses.packages.filter((p) => `${p.name} ${p.license}`.toLowerCase().includes(q)) : licenses.packages;
  });

  $effect(() => {
    if (tab !== "licenses" || licenses !== null) return;
    fetch("/licenses.json")
      .then((r) => (r.ok ? r.json() : false))
      .then((data) => (licenses = data))
      .catch(() => (licenses = false));
  });

  const AUTHOR = "Rafael Luciano Gonçalves Correia";
  const EMAIL = "rafaellgcorreia@gmail.com";
  const REPO = "https://github.com/rafaellgc/Renome";

  const sections = $derived({ pt, en, es }[settings.lang]);
  let current = $state("start");
  let content = $state<HTMLElement>();
  let version = $state("");

  onMount(() => {
    getVersion().then((v) => (version = v));
  });

  function go(id: string) {
    current = id;
    content?.querySelector(`#help-${id}`)?.scrollIntoView({ block: "start" });
  }

  /** Highlights in the index the section being read. */
  function onscroll() {
    if (!content) return;
    const top = content.getBoundingClientRect().top + 24;
    let id = sections[0].id;
    for (const el of content.querySelectorAll<HTMLElement>("section[data-id]")) {
      if (el.getBoundingClientRect().top <= top) id = el.dataset.id!;
    }
    current = id;
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="overlay" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="modal help" onclick={(e) => e.stopPropagation()}>
    <header>
      <div class="tabs" role="tablist">
        <button role="tab" aria-selected={tab === "manual"} class:on={tab === "manual"} onclick={() => (tab = "manual")}>{t("helpManual")}</button>
        <button role="tab" aria-selected={tab === "about"} class:on={tab === "about"} onclick={() => (tab = "about")}>{t("helpAbout")}</button>
        <button role="tab" aria-selected={tab === "licenses"} class:on={tab === "licenses"} onclick={() => (tab = "licenses")}>{t("helpLicenses")}</button>
      </div>
      <button class="icon" title={t("close")} aria-label={t("close")} onclick={onclose}>✕</button>
    </header>

    {#if tab === "manual"}
      <div class="manual">
        <nav aria-label={t("helpContents")}>
          {#each sections as s (s.id)}
            <button class:sub={s.sub} class:on={current === s.id} onclick={() => go(s.id)}>{s.title}</button>
          {/each}
        </nav>
        <div class="content" bind:this={content} {onscroll}>
          {#each sections as s (s.id)}
            <section id="help-{s.id}" data-id={s.id}>
              <h3>{s.title}</h3>
              {@html s.html}
            </section>
          {/each}
        </div>
      </div>
    {:else if tab === "licenses"}
      <div class="licenses">
        <h3>{t("licAppTitle")}</h3>
        <p>{t("licAppIntro")}</p>
        <pre>{appLicense.trim()}</pre>
        <h3>{t("licThirdTitle")}</h3>
        {#if licenses === null}
          <p class="muted">…</p>
        {:else if licenses === false}
          <p class="muted">{t("licMissing")}</p>
        {:else}
          <p>{t("licThirdIntro", { n: licenses.packages.length })}</p>
          <input class="filter" bind:value={licenseQuery} placeholder={t("licFilter")} spellcheck="false" />
          <ul>
            {#each shownPackages as p (`${p.kind}:${p.name}@${p.version}`)}
              {@const key = `${p.kind}:${p.name}@${p.version}`}
              <li>
                <button class="pkg" aria-expanded={expanded === key} onclick={() => (expanded = expanded === key ? null : key)}>
                  <span class="chev" class:open={expanded === key}>▸</span>
                  <b>{p.name}</b>
                  <span class="muted">{p.version}</span>
                  <span class="spacer"></span>
                  <span class="lic">{p.license}</span>
                  <span class="kind">{p.kind === "rust" ? "Rust" : "npm"}</span>
                </button>
                {#if expanded === key}
                  <div class="detail">
                    <button class="link" onclick={() => openUrl(p.url)}>{p.url}</button>
                    {#each p.texts as i (i)}<pre>{licenses.texts[i]}</pre>{/each}
                  </div>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {:else}
      <div class="about">
        <img src="/app-icon.png" alt="" width="96" height="96" />
        <h2>Renome</h2>
        {#if version}<p class="muted">{t("version", { v: version })}</p>{/if}
        <p>{t("aboutDesc")}</p>
        <p>
          {t("aboutAuthor", { name: AUTHOR })}<br />
          <button class="link" onclick={() => openUrl(`mailto:${EMAIL}`)}>{EMAIL}</button><br />
          {t("aboutClaude")}
        </p>
        <p class="muted">
          {t("aboutBuilt")}<br />
          <button class="link" onclick={() => (tab = "licenses")}>{t("aboutLicense")}</button><br />
          © 2026 {AUTHOR}
        </p>
        <button class="primary-soft" onclick={() => openUrl(REPO)}>{t("aboutRepo")}</button>
        <div class="tmdb">
          <button class="logo" title="themoviedb.org" onclick={() => openUrl("https://www.themoviedb.org")}>
            <img src="/tmdb-logo.svg" alt="TMDB" width="140" />
          </button>
          <p class="muted">{t("tmdbNotice")}</p>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .help {
    width: min(920px, 94vw);
    height: 84vh;
    max-height: 84vh;
    padding: 0;
    gap: 0;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
  }
  .tabs { display: flex; gap: 4px; }
  .tabs button { border: none; background: none; font-weight: 600; color: var(--muted); }
  .tabs button.on { background: var(--accent-soft); color: var(--accent); }
  .manual { flex: 1; display: flex; min-height: 0; }
  nav {
    flex: none;
    width: 220px;
    overflow-y: auto;
    padding: 10px 8px;
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  nav button {
    border: none;
    background: none;
    text-align: left;
    height: auto;
    min-height: 28px;
    padding: 4px 8px;
    font-size: 13px;
    white-space: normal;
  }
  nav button.sub { padding-left: 22px; color: var(--muted); }
  nav button.on { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 6px 24px 40vh;
    user-select: text;
    line-height: 1.55;
  }
  section { padding-top: 14px; }
  section + section { border-top: 1px solid var(--row-line); margin-top: 14px; }
  .content h3 { margin: 0 0 6px; font-size: 17px; }
  .content :global(h4) { margin: 14px 0 4px; font-size: 14px; }
  .content :global(p) { margin: 6px 0; }
  .content :global(ul), .content :global(ol) { margin: 6px 0; padding-left: 22px; }
  .content :global(li) { margin: 3px 0; }
  .content :global(code) {
    font: 12.5px ui-monospace, monospace;
    background: var(--surface-2);
    border: 1px solid var(--row-line);
    border-radius: 4px;
    padding: 0 4px;
    /* Keep short snippets like "1 - 3 - 2" on one line. */
    white-space: pre;
  }
  .content :global(kbd) {
    font: 12px ui-monospace, monospace;
    padding: 0 5px;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 4px;
    background: var(--surface-2);
  }
  .content :global(table) { border-collapse: collapse; margin: 8px 0; font-size: 13px; }
  .content :global(td) { padding: 4px 12px 4px 0; border-bottom: 1px solid var(--row-line); vertical-align: top; }
  .content :global(td:first-child) { white-space: nowrap; }
  .content :global(.ex) {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 8px 0;
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--surface-2);
  }
  .content :global(.ex span) { color: var(--accent); font-weight: 700; }
  .content :global(.tip) {
    padding: 8px 10px;
    border-left: 3px solid var(--accent);
    background: var(--accent-soft);
    border-radius: 0 6px 6px 0;
  }
  .content :global(.ins) { color: var(--accent); background: var(--accent-soft); font-weight: 600; border-radius: 3px; padding: 0 2px; }
  .content :global(.del) { color: var(--danger); text-decoration: line-through; }
  .content :global(.mov) { text-decoration: underline dotted; text-decoration-thickness: 1px; text-underline-offset: 3px; }
  .content :global(.bad) { color: white; background: var(--danger); border-radius: 50%; padding: 0 5px; font-weight: 700; }
  .content :global(.warn) { color: black; background: var(--warn); border-radius: 50%; padding: 0 5px; font-weight: 700; }
  .about {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 24px;
    gap: 4px;
    user-select: text;
  }
  .about h2 { margin: 8px 0 0; font-size: 24px; }
  .about p { margin: 6px 0; line-height: 1.5; }
  .about .primary-soft { margin-top: 10px; }
  .link { border: none; background: none; height: auto; padding: 0; color: var(--accent); }
  .link:hover:not(:disabled) { background: none; text-decoration: underline; }
  .muted { color: var(--muted); }
  .tmdb { margin-top: 18px; display: flex; flex-direction: column; align-items: center; gap: 6px; max-width: 420px; }
  .tmdb .logo { border: none; background: none; height: auto; padding: 4px; }
  .tmdb p { font-size: 12px; margin: 0; }
  .licenses {
    flex: 1;
    overflow-y: auto;
    padding: 6px 24px 24px;
    user-select: text;
    line-height: 1.5;
  }
  .licenses h3 { margin: 16px 0 6px; font-size: 17px; }
  .licenses p { margin: 6px 0; }
  .licenses pre {
    margin: 8px 0;
    padding: 10px 12px;
    border-radius: 6px;
    background: var(--surface-2);
    font: 12px/1.45 ui-monospace, monospace;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .licenses .filter { width: min(320px, 100%); margin: 4px 0 8px; }
  .licenses ul { list-style: none; margin: 0; padding: 0; }
  .licenses li { border-bottom: 1px solid var(--row-line); }
  .pkg {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: auto;
    min-height: 30px;
    padding: 4px 6px;
    border: none;
    background: none;
    text-align: left;
    font-size: 13px;
  }
  .pkg .chev { color: var(--muted); transition: transform 0.15s; }
  .pkg .chev.open { transform: rotate(90deg); }
  .pkg .spacer { flex: 1; }
  .pkg .lic { color: var(--muted); font-size: 12px; text-align: right; }
  .pkg .kind {
    flex: none;
    font-size: 11px;
    padding: 0 6px;
    border-radius: 8px;
    background: var(--surface-2);
    color: var(--muted);
  }
  .detail { padding: 0 6px 8px 26px; font-size: 13px; }
  @media (max-width: 640px) {
    nav { display: none; }
  }
</style>
