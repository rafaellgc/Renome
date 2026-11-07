<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "./i18n";
  import { settings } from "./settings.svelte";
  import type { SubCandidate } from "./types";

  type Row = {
    path: string;
    name: string;
    /** Per language asked for: whether the video has it, and the one
     *  chosen (an index in `candidates`; -1: none). */
    langs: { lang: string; have: boolean; pick: number }[];
    candidates: SubCandidate[];
  };

  let {
    rows,
    remaining,
    onpick,
    onclose,
  }: {
    rows: Row[];
    /** OpenSubtitles downloads left today, when known. */
    remaining: number | null;
    onpick: (path: string, lang: string, index: number) => void;
    onclose: () => void;
  } = $props();

  const SOURCE = { openSubtitles: "OpenSubtitles", subdl: "SubDL" } as const;
  const count = new Intl.NumberFormat();
  /** A language's name in the window's language (`pt-BR` → "Português
   *  (Brasil)"); the code when it has none. */
  const names = $derived.by(() => {
    try {
      return new Intl.DisplayNames([settings.lang], { type: "language" });
    } catch {
      return null;
    }
  });
  function langName(code: string) {
    let name: string | undefined;
    try {
      name = names?.of(code);
    } catch {
      name = undefined;
    }
    return name && name !== code ? name.charAt(0).toLocaleUpperCase(settings.lang) + name.slice(1) : code;
  }

  function label(c: SubCandidate) {
    return [
      `${c.exact ? "★ " : ""}${c.release || t("subsNoRelease")}`,
      SOURCE[c.source],
      c.downloads ? t("subsDownloads", { n: count.format(c.downloads) }) : "",
      c.hearingImpaired ? "SDH" : "",
    ]
      .filter(Boolean)
      .join(" · ");
  }

  const chosen = $derived(rows.reduce((n, r) => n + r.langs.filter((l) => !l.have && l.pick >= 0).length, 0));
  const fromOs = $derived(
    rows.reduce((n, r) => n + r.langs.filter((l) => !l.have && l.pick >= 0 && r.candidates[l.pick]?.source === "openSubtitles").length, 0),
  );
</script>

<div class="overlay">
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="modal subs" onclick={(e) => e.stopPropagation()}>
    <header>
      <h2>{t("subsReviewTitle")}</h2>
      <button class="icon" title={t("close")} aria-label={t("close")} onclick={onclose}>✕</button>
    </header>
    <p class="muted hint">{t("subsReviewHint")}</p>
    <div class="list">
      {#each rows as r (r.path)}
        <div class="video">
          <b class="name" title={r.path}>{r.name}</b>
          {#each r.langs as l (l.lang)}
            {@const options = r.candidates.map((c, i) => ({ c, i })).filter((o) => o.c.lang === l.lang)}
            {@const c = r.candidates[l.pick]}
            <div class="lang">
              <span class="code" title={l.lang}>{langName(l.lang)}</span>
              {#if l.have}
                <span class="muted">{t("subsHave")}</span>
              {:else if !options.length}
                <span class="muted">{t("subsNoneFound")}</span>
              {:else}
                <select value={l.pick} onchange={(e) => onpick(r.path, l.lang, Number(e.currentTarget.value))}>
                  <option value={-1}>{t("subsSkip")}</option>
                  {#each options as o (o.i)}<option value={o.i}>{label(o.c)}</option>{/each}
                </select>
                {#if c?.url}<button class="link" onclick={() => openUrl(c.url)}>{t("subsSee")}</button>{/if}
              {/if}
            </div>
          {/each}
        </div>
      {/each}
    </div>
    <div class="modal-foot">
      <span class="muted">
        {t("subsChosenN", { n: chosen })}
        {#if remaining !== null}· {t("subsRemaining", { n: remaining })}{/if}
        {#if remaining !== null && fromOs > remaining}· <b class="bad">{t("subsOverQuota")}</b>{/if}
      </span>
      <span class="spacer"></span>
      <button class="primary" onclick={onclose}>{t("close")}</button>
    </div>
  </div>
</div>

<style>
  .subs { width: min(980px, 95vw); max-height: 88vh; gap: 10px; }
  header { display: flex; align-items: center; gap: 12px; }
  h2 { margin: 0; font-size: 16px; flex: 1; }
  .hint { margin: 0; font-size: 13px; }
  .list { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 12px; }
  .video { display: flex; flex-direction: column; gap: 4px; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; }
  .lang { display: flex; align-items: center; gap: 8px; font-size: 12.5px; min-width: 0; }
  .lang select { flex: 1; min-width: 0; }
  .code { width: 150px; flex-shrink: 0; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .link { border: none; background: none; height: auto; padding: 0; color: var(--accent); font-size: 12px; }
  .bad { color: var(--danger); }
  .spacer { flex: 1; }
  .modal-foot { display: flex; align-items: center; gap: 8px; }
</style>
