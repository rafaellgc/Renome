<script lang="ts">
  import { slide } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { t, type Key } from "./i18n";
  import { TABLE, choicesOf, comicPreset, comicPresetOf, limitsOf, menuOf, type ComicPreset } from "./rules";
  import { settings } from "./settings.svelte";
  import type { ComicDevice, RuleType, Step } from "./types";
  /** Rules that pick their own kind of file: no filter of their own (the
   *  Rust catalog's group). */
  const byKind = (type: RuleType) => TABLE.media.includes(type);
  /** The readers a comic e-book can be made for, by maker, in the menu's
   *  order. */
  /** A reader model's label: name, screen and whether it is colour. */
  const modelName = (m: Reader) => (m.name.startsWith("@") ? t(m.name.slice(1) as Key) : m.name);
  const readerLabel = (m: Reader) => {
    const [w, h] = SCREENS[m.device]?.size ?? [0, 0];
    return `${modelName(m)} · ${w} × ${h}${isColourInk(m.device) ? ` · ${t("comicColour")}` : ""}`;
  };
  /** A format's name in a menu, marked when the reader does not open it as
   *  it comes (it can still be chosen). */
  const formatOption = (label: string, f: Format, device: ComicDevice | undefined, model: string | undefined, any: boolean | undefined) =>
    device && !any && !opens(device, model, f) ? `${label} — ${t("readerNotOpensShort")}` : label;
  /** Why a format may not suit the reader, if it may not. */
  const notOpens = (f: Format | "" | undefined, device: ComicDevice | undefined, model: string | undefined, any: boolean | undefined) => {
    if (!f || !device || any || opens(device, model, f)) return "";
    const m = modelOf(readerOf(device, model));
    const best = formatsOf(device, model)?.[0];
    return t("readerNotOpens", { reader: m ? modelName(m) : "", format: f.toUpperCase(), best: (best ?? "").toUpperCase() });
  };
  import { api } from "./api";
  import { OTHER_DEVICES, POPULAR, READERS, SCREENS, chooseReader, formatsOf, isColourInk, modelOf, opens, readerOf, type Format, type Reader } from "./readers";

  /** Whether this system can print books to PDF (asked once). */
  let canPrint = $state(false);
  api.pdfPrintAvailable().then((v) => (canPrint = v)).catch(() => {});

  let {
    step = $bindable(),
    error = null,
    sample = "",
    first = false,
    last = false,
    onmove,
    onremove,
    lookup,
    subs,
    edit,
    summary = null,
    currentNames = [],
    hint = null,
  }: {
    step: Step;
    error?: string | null;
    /** A file name from the list, to show examples. */
    sample?: string;
    first?: boolean;
    last?: boolean;
    onmove: (delta: -1 | 1) => void;
    onremove: () => void;
    /** A line about the rule's effect (songs without tags). */
    summary?: string | null;
    /** The names in the table now (without extensions), in its order: what
     *  the list rule's box can start from. */
    currentNames?: string[];
    /** What the preview says of this rule: a list rule's names, a
     *  rearrange rule's parts of the sample. */
    hint?: { names?: number; parts?: string[] } | null;
    /** Photo rule: dates typed in for the files chosen in the list (the
     *  button's text and hint, already translated). */
    edit?: { n: number; typed: number; label: string; hint: string; onclick: () => void };
    /** Movie and series rules: search state and actions. */
    lookup?: {
      /** The show found last (series rule). */
      show?: string | null;
      progress: string | null;
      found: number;
      choose: number;
      none: number;
      onsearch: () => void;
      onreview: () => void;
      /** Comic and music rules: details typed in for the files chosen in
       *  the list (the button's text and hint, already translated). */
      edit?: { n: number; typed: number; label: string; hint: string; onclick: () => void };
    };
    /** Movie and series rules: the subtitles' search state and actions. */
    subs?: {
      progress: string | null;
      summary: string | null;
      /** Videos with subtitles found (the review opens on them). */
      found: number;
      onsearch: () => void;
      onreview: () => void;
    };
  } = $props();

  /** What each comic catalogue is, under the choice. */
  const SOURCE_HINT: Record<string, Key> = {
    gcd: "gcdHint",
    comicvine: "cvHint",
    metron: "mtHint",
    mangadex: "mdHint",
    anilist: "alHint",
    mangaupdates: "muHint",
  };

  let open = $state(true);
  const r = $derived(step.rule);
  /** The options that do nothing as the rule is set (the Rust catalog
   *  says: `catalog::inert`, the flows take them as off): hidden. */
  let inert = $state<string[]>([]);
  $effect(() => {
    const rule = $state.snapshot(step.rule);
    api.ruleInert(rule).then((v) => (inert = v), () => {});
  });
  const applies = (name: string) => !inert.includes(name);
  /** A whole-number option's bounds in this rule (the Rust catalog's). */
  const lim = (name: string) => limitsOf(r.type, name);
  /** Bounds in milliseconds as a field in seconds. */
  const seconds = (b: { min?: number; max?: number }) => ({ min: b.min === undefined ? undefined : b.min / 1000, max: b.max === undefined ? undefined : b.max / 1000 });
  /** The screen of the reader a book rule converts for, if one is chosen. */
  const bookDev = $derived(step.rule.type === "book" ? modelOf(step.rule.model)?.device : undefined);

  /** What a closed card does, in a few words. */
  const brief = $derived.by(() => {
    // Spaces shown, so a replaced space is seen.
    const q = (x: string) => `“${x.replace(/ /g, "␣")}”`;
    // An option's label without its examples in brackets.
    const short = (k: Key) => t(k).replace(/\s*\(.*\)\s*$/, "");
    const list = (pairs: [boolean, string][]) => pairs.filter(([on]) => on).map(([, x]) => x).join(" · ");
    switch (r.type) {
      case "replace":
        return r.find ? (r.replace ? `${q(r.find)} → ${q(r.replace)}` : t("briefDrop", { x: q(r.find) })) : "";
      case "regex":
        return r.pattern ? `/${r.pattern}/ → ${r.replace || "∅"}` : "";
      case "rearrange":
        return r.order ? `${t("newOrder")}: ${r.order}` : "";
      case "list": {
        const n = hint?.names ?? 0;
        return n ? t("listOrderN", { n }) : "";
      }
      case "case":
        return t(({ lower: "caseLower", upper: "caseUpper", sentence: "caseSentence", title: "caseTitle", titleSmart: "caseTitleSmart" } as const)[r.mode]);
      case "insert": {
        const what = r.source === "text" ? (r.text ? q(r.text) : "") : short(({ now: "srcNow", modified: "srcModified", taken: "srcTaken", folder: "srcFolder" } as const)[r.source]);
        const where = r.at === "start" ? t("atStart") : r.at === "end" ? t("atEnd") : `${t("position")} ${r.position}`;
        return what ? `${what} · ${where.toLowerCase()}` : "";
      }
      case "number":
        return `${r.template} · ${t("start").toLowerCase()} ${r.start}`;
      case "remove":
        return list([[r.extraSpaces, short("rmExtraSpaces")], [r.allSpaces, short("rmAllSpaces")], [r.accents, short("rmAccents")], [r.digits, short("rmDigits")], [r.leadingNumber, short("rmLeadingNumber")], [!!r.chars, q(r.chars)]]);
      case "convert":
        return list([[r.mojibake, short("cvMojibake")], [r.transliterate, short("cvLatin")], [!!r.splitWords, short("cvSplit")], [!!r.webSafe, short("cvWeb")]]);
      case "removeRange":
      {
        const bound = (b: string, pos: number, text: string) => (b === "text" ? q(text) : b === "start" ? t("atStart").toLowerCase() : b === "end" ? t("atEnd").toLowerCase() : String(pos));
        return `${bound(r.from, r.fromPosition, r.fromText)} → ${bound(r.to, r.toPosition, r.toText)}`;
      }
      case "charAt":
        return `${t("position")} ${r.position}${r.replace ? ` → ${q(r.replace)}` : ""}`;
      case "extension":
        return r.mode === "set" ? `.${r.value}` : short(({ lower: "extLower", upper: "extUpper", remove: "extRemove", content: "extContent" } as const)[r.mode]);
      case "subtitle":
        return list([[r.toSrt, "SRT"], [r.utf8, "UTF-8"], [r.ads, short("subAds")], [r.hearing, short("subHearing")], [!!r.shiftMs || !!r.fpsFrom, short("sectTime")], [r.extract, short("subExtract")]]);
      case "movie":
      case "series":
      case "music":
      case "book":
      case "comic":
      case "photo": {
        const n = lookup ? lookup.found + lookup.choose + lookup.none : 0;
        return n ? `${r.template} · ${t("briefFound", { found: lookup!.found + lookup!.choose, n })}` : r.template;
      }
      default:
        return "";
    }
  });

  /** The explanations that are cut short (not the template variables). */
  const HINTS = "label.check small, p.hint:not(.mono)";
  /** A clamped hint (two lines) opens on a click, without ticking its box. */
  function hintClick(e: MouseEvent) {
    const hint = (e.target as HTMLElement).closest(HINTS);
    if (!(hint instanceof HTMLElement) || !(hint.classList.contains("more") || hint.classList.contains("open"))) return;
    e.preventDefault();
    hint.classList.toggle("open");
  }
  /** Marks a hint that has more than shows, to be opened. */
  function hintOver(e: PointerEvent) {
    const hint = (e.target as HTMLElement).closest(HINTS);
    if (hint instanceof HTMLElement && !hint.classList.contains("open")) hint.classList.toggle("more", hint.scrollHeight > hint.clientHeight + 1);
  }

  /** A rearrange rule's parts of the sample name (the preview's). */
  const parts = $derived(hint?.parts ?? []);

  /** Appends a part number to the order (clicking a part). */
  function addPart(n: number) {
    if (r.type !== "rearrange") return;
    const sep = r.separator.trim() ? r.separator : " ";
    r.order = r.order.trim() ? `${r.order}${sep}${n}` : `${n}`;
  }
</script>

<article class="card" class:off={!step.enabled} class:has-error={!!error}>
  <header>
    <input type="checkbox" bind:checked={step.enabled} title={t("toggleRule")} />
    <button class="title" onclick={() => (open = !open)} aria-expanded={open}>
      <span class="chev" class:closed={!open}><Icon name="chevron-down" size={14} /></span>
      {t(`rule.${r.type}`)}
    </button>
    <div class="actions">
      <button class="icon" disabled={first} onclick={() => onmove(-1)} title={t("up")} aria-label={t("up")}><Icon name="arrow-up" size={15} /></button>
      <button class="icon" disabled={last} onclick={() => onmove(1)} title={t("down")} aria-label={t("down")}><Icon name="arrow-down" size={15} /></button>
      <button class="icon danger" onclick={onremove} title={t("removeRule")} aria-label={t("removeRule")}><Icon name="x" size={15} /></button>
    </div>
  </header>

  {#if !open && brief}<p class="brief" title={brief}>{brief}</p>{/if}
  {#if open}
    <!-- A rule turned off is left as it is: no edits, no searches. -->
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <fieldset class="body" disabled={!step.enabled} onclick={hintClick} onpointerover={hintOver} transition:slide={{ duration: 160 }}>
      {#if r.type === "replace"}
        <label>{t("find")} <input bind:value={r.find} spellcheck="false" /></label>
        <label>{t("replaceWith")} <input bind:value={r.replace} spellcheck="false" /></label>
        <div class="row">
          <label class="check"><input type="checkbox" bind:checked={r.caseSensitive} /> {t("caseSensitive")}</label>
          <label class="check" title={t("firstOnlyHint")}><input type="checkbox" bind:checked={r.firstOnly} /> {t("firstOnly")}</label>
        </div>
        {@render target()}
      {:else if r.type === "regex"}
        <label>{t("expression")} <input class="mono" bind:value={r.pattern} spellcheck="false" placeholder={"(\\d+) - (.*)"} /></label>
        <label>{t("replaceWith")} <input class="mono" bind:value={r.replace} spellcheck="false" placeholder="$2 ($1)" /></label>
        <div class="row">
          <label class="check"><input type="checkbox" bind:checked={r.caseSensitive} /> {t("caseSensitive")}</label>
          <label class="check" title={t("firstOnlyHint")}><input type="checkbox" bind:checked={r.firstOnly} /> {t("firstOnly")}</label>
        </div>
        {@render target()}
        <p class="hint mono">{t("groupsHint")}</p>
      {:else if r.type === "rearrange"}
        <label>{t("separator")} <input class="mono sep" bind:value={r.separator} spellcheck="false" placeholder=" - " /></label>
        {#if parts.length}
          <div class="parts">
            <span class="hint">{t("partsOfSample")}</span>
            {#each parts as part, i (i)}
              <button class="part" title={t("addPartHint")} onclick={() => addPart(i + 1)}>
                <b>{i + 1}</b><span>{part}</span>
              </button>
            {/each}
          </div>
        {:else if sample}
          <p class="hint">{t("noParts")}</p>
        {/if}
        <label>{t("newOrder")} <input class="mono" bind:value={r.order} spellcheck="false" placeholder="1 - 3 - 2" /></label>
        <p class="hint">{t("newOrderHint")}</p>
        {@render target()}
      {:else if r.type === "list"}
        <label>{t("listNames")}
          <textarea class="mono list" bind:value={r.names} spellcheck="false" rows="8" wrap="off"></textarea>
        </label>
        {#if !r.names.trim() && currentNames.length}
          <button class="fill" title={t("listFillHint")} onclick={() => (r.names = currentNames.join("\n"))}>{t("listFill")}</button>
        {/if}
        <label class="check" title={t("pairsHint")}><input type="checkbox" bind:checked={r.pairs} /> {t("pairs")}</label>
        <p class="hint">{t("listOrderHint")}</p>
      {:else if r.type === "case"}
        <label>{t("mode")}
          <select bind:value={r.mode}>
            {@render options("mode")}
          </select>
        </label>
        <label>{t("caseKeep")} <input bind:value={r.keepWords} spellcheck="false" placeholder="swat, dvd" title={t("caseKeepHint")} /></label>
        {@render target()}
      {:else if r.type === "insert"}
        <label>{t("what")}
          <select bind:value={r.source}>
            {@render options("source")}
          </select>
        </label>
        {#if r.source === "text"}
          <label>{t("text")} <input bind:value={r.text} spellcheck="false" /></label>
        {:else if r.source !== "folder"}
          <label>{t("format")} <input class="mono" bind:value={r.dateFormat} spellcheck="false" /></label>
          <p class="hint mono">{t("formatHint")}</p>
          {#if r.source === "taken"}
            <label class="check"><input type="checkbox" bind:checked={r.fallback} /> {t("takenFallback")}</label>
            <p class="hint">{t("takenHint")}</p>
          {/if}
        {/if}
        <div class="row">
          <label>{t("where")}
            <select bind:value={r.at}>
              {@render options("at")}
            </select>
          </label>
          {#if r.at === "index" || r.at === "fromEnd"}
            <label class="narrow">{t("position")} <input type="number" {...lim("position")} bind:value={r.position} /></label>
          {/if}
        </div>
      {:else if r.type === "number"}
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("templateHint")}</p>
        <div class="row">
          <label class="narrow">{t("start")} <input type="number" {...lim("start")} bind:value={r.start} /></label>
          <label class="narrow">{t("step")} <input type="number" {...lim("step")} bind:value={r.step} /></label>
          <label class="narrow">{t("digits")} <input type="number" {...lim("digits")} bind:value={r.digits} /></label>
        </div>
        <label class="check"><input type="checkbox" bind:checked={r.perFolder} /> {t("perFolder")}</label>
        <label class="check" title={t("pairsHint")}><input type="checkbox" bind:checked={r.pairs} /> {t("pairs")}</label>
      {:else if r.type === "remove"}
        <div class="checks">
          <label class="check"><input type="checkbox" bind:checked={r.extraSpaces} /> {t("rmExtraSpaces")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.allSpaces} /> {t("rmAllSpaces")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.accents} /> {t("rmAccents")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.digits} /> {t("rmDigits")}</label>
          <label class="check" title={t("rmLeadingNumberHint")}><input type="checkbox" bind:checked={r.leadingNumber} /> {t("rmLeadingNumber")}</label>
        </div>
        <label>{t("chars")} <input class="mono" bind:value={r.chars} spellcheck="false" placeholder="_#[]" /></label>
        <div class="row">
          <label>{t("maxLength")} <input type="number" {...lim("maxLength")} bind:value={r.maxLength} /></label>
        </div>
        {@render target()}
      {:else if r.type === "convert"}
        <div class="checks">
          <label class="check" title={t("cvMojibakeHint")}><input type="checkbox" bind:checked={r.mojibake} /> {t("cvMojibake")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.transliterate} /> {t("cvLatin")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.splitWords} /> {t("cvSplit")}</label>
        </div>
        <label>{t("cvWeb")}
          <select bind:value={r.webSafe} title={t("cvWebHint")}>
            {@render options("webSafe")}
          </select>
        </label>
        {@render target()}
      {:else if r.type === "removeRange"}
        <div class="row">
          <label>{t("from")}
            <select bind:value={r.from}>
              {@render options("from")}
            </select>
          </label>
          {#if r.from === "position"}
            <label class="narrow">{t("position")} <input type="number" {...lim("fromPosition")} bind:value={r.fromPosition} /></label>
          {:else if r.from === "text"}
            <label>{t("text")} <input bind:value={r.fromText} spellcheck="false" /></label>
          {/if}
        </div>
        <div class="row">
          <label>{t("to")}
            <select bind:value={r.to}>
              {@render options("to")}
            </select>
          </label>
          {#if r.to === "position"}
            <label class="narrow">{t("position")} <input type="number" {...lim("toPosition")} bind:value={r.toPosition} /></label>
          {:else if r.to === "text"}
            <label>{t("text")} <input bind:value={r.toText} spellcheck="false" /></label>
          {/if}
        </div>
        {#if r.from === "text" || r.to === "text"}
          <label class="check"><input type="checkbox" bind:checked={r.inclusive} /> {t("inclusive")}</label>
        {/if}
      {:else if r.type === "charAt"}
        <div class="row">
          <label class="narrow">{t("position")} <input type="number" {...lim("position")} bind:value={r.position} /></label>
          <label class="check"><input type="checkbox" bind:checked={r.fromEnd} /> {t("countFromEnd")}</label>
        </div>
        <div class="row">
          <label>{t("onlyIf")} <input bind:value={r.onlyIf} placeholder={t("any")} /></label>
          <label>{t("replaceBy")} <input bind:value={r.replace} placeholder={t("deleteIt")} /></label>
        </div>
        {#if r.onlyIf}
          <label class="check"><input type="checkbox" bind:checked={r.caseSensitive} /> {t("caseSensitive")}</label>
        {/if}
      {:else if r.type === "extension"}
        <div class="row">
          <label>{t("action")}
            <select bind:value={r.mode}>
              {@render options("mode")}
            </select>
          </label>
          {#if r.mode === "set"}
            <label>{t("newExt")} <input bind:value={r.value} spellcheck="false" placeholder="jpg" /></label>
          {/if}
        </div>
        {#if r.mode === "content"}
          <p class="hint">{t("extContentHint")}</p>
        {/if}
      {:else if r.type === "movie"}
        <p class="hint top">{t("movieHint")}</p>
        <h4 class="sect">{t("sectName")}</h4>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("movieTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colonAs} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.folder} /> <span>{t("folderLabel")}<small>{t("folderHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.trashAds} /> <span>{t("trashAdsLabel")}<small>{t("trashAdsHint")}</small></span></label>
        <h4 class="sect">{t("sectSearch")}</h4>
        {@render actions(t("tmdbSearch"), "movieSummary")}
        <h4 class="sect">{t("sectBeside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.nfo} /> <span>{t("sidecarLabel")}<small>{t("sidecarHint")}</small></span></label>
        {#if r.nfo}
          <label class="check sub"><input type="checkbox" bind:checked={r.backdrop} /> {t("fanartLabel")}</label>
        {/if}
        <h4 class="sect">{t("sectInside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.videoTags} /> <span>{t("videoTagsLabel")}<small>{t("videoTagsHint")}</small></span></label>
        {#if r.videoTags}{@render videoOptions()}{/if}
        <h4 class="sect">{t("sectList")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{t("movieMissingLabel")}<small>{t("movieMissingHint")}</small></span></label>
        <h4 class="sect">{t("sectSubs")}</h4>
        {@render subtitles()}
      {:else if r.type === "series"}
        <p class="hint top">{t("seriesHint")}</p>
        <h4 class="sect">{t("sectName")}</h4>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("seriesTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colonAs} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{t("seriesFoldersLabel")}<small>{t("seriesFoldersHint")}</small></span></label>
        {#if r.folders}
          <label class="sub">{t("seriesFolderName")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.trashAds} /> <span>{t("trashAdsLabel")}<small>{t("trashAdsHint")}</small></span></label>
        <h4 class="sect">{t("sectSearch")}</h4>
        <label>{t("seriesSource")}
          <select bind:value={r.source}>
            {@render options("source")}
          </select>
        </label>
        {#if r.source === "anilist"}<p class="hint">{t("animeHint")}</p>{/if}
        <label>{t("seriesShow")} <input bind:value={r.show} placeholder={t(r.source === "anilist" ? "seriesShowPlaceholderAnilist" : "seriesShowPlaceholder")} spellcheck="false" /></label>
        {#if lookup?.show}<p class="hint">{t("seriesCurrent", { show: lookup.show })}</p>{/if}
        {@render actions(t("tvSearch"), "seriesSummary")}
        {#if applies("nfo")}
          <h4 class="sect">{t("sectBeside")}</h4>
          <label class="check"><input type="checkbox" bind:checked={r.nfo} /> <span>{t("seriesSidecarLabel")}<small>{t("seriesSidecarHint")}</small></span></label>
          {#if r.nfo}
            <label class="check sub"><input type="checkbox" bind:checked={r.backdrop} /> {t("fanartLabel")}</label>
          {/if}
          <h4 class="sect">{t("sectInside")}</h4>
          <label class="check"><input type="checkbox" bind:checked={r.videoTags} /> <span>{t("videoTagsLabel")}<small>{t("videoTagsHintTv")}</small></span></label>
          {#if r.videoTags}{@render videoOptions()}{/if}
        {/if}
        <h4 class="sect">{t("sectList")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{t("seriesMissingLabel")}<small>{t("seriesMissingHint")}</small></span></label>
        {#if applies("subtitles")}
          <h4 class="sect">{t("sectSubs")}</h4>
          {@render subtitles()}
        {/if}
      {:else if r.type === "photo"}
        {@const ready = TABLE.photoTemplates}
        <p class="hint top">{t("photoHint")}</p>
        <h4 class="sect">{t("sectName")}</h4>
        <label>{t("photoReady")}
          <select value={ready.names.includes(r.template as never) ? r.template : ""} onchange={(e) => { const v = (e.currentTarget as HTMLSelectElement).value; if (v) r.template = v; }}>
            <option value="" disabled>{t("comicPresetCustom")}</option>
            {#each ready.names as n, i (n)}<option value={n}>{t(`photoReady${i}` as Key)}</option>{/each}
          </select>
        </label>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("photoTemplateHint")}</p>
        <h4 class="sect">{t("sectDate")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.fromName} /> <span>{t("photoFromName")}<small>{t("photoFromNameHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.modified} /> <span>{t("photoModified")}<small>{t("photoModifiedHint")}</small></span></label>
        <label class="shift">{t("photoShift")}
          <input type="number" {...lim("shiftHours")} step="1" bind:value={r.shiftHours} />
        </label>
        <p class="hint">{t("photoShiftHint")}</p>
        {#if edit}
          <div class="movie-actions">
            <button disabled={!edit.n} onclick={edit.onclick}>{edit.label}</button>
          </div>
          <p class="hint">{edit.hint}{edit.typed ? ` ${t("photoEditTypedN", { n: edit.typed })}` : ""}</p>
        {/if}
        <h4 class="sect">{t("sectFolders")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{t("photoFolders")}<small>{t("photoFoldersHint")}</small></span></label>
        {#if r.folders}
          <label class="sub">{t("photoFolderReady")}
            <select value={ready.folders.includes(r.folderTemplate as never) ? r.folderTemplate : ""} onchange={(e) => { const v = (e.currentTarget as HTMLSelectElement).value; if (v) r.folderTemplate = v; }}>
              <option value="" disabled>{t("comicPresetCustom")}</option>
              {#each ready.folders as f (f)}<option value={f}>{f}</option>{/each}
            </select>
          </label>
          <label class="sub">{t("musicFolderName")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <h4 class="sect">{t("sectInside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.setModified} /> <span>{t("photoSetModified")}<small>{t("photoSetModifiedHint")}</small></span></label>
        <p class="hint">{t("photoCopiesHint")}</p>
      {:else if r.type === "subtitle"}
        {@const fps = (n: number) => n.toLocaleString(settings.lang === "en" ? "en-US" : "pt-BR")}
        <p class="hint top">{t("subToolsHint")}</p>
        <h4 class="sect">{t("sectFormat")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.toSrt} /> <span>{t("subToSrt")}<small>{t("subToSrtHint")}</small></span></label>
        {#if r.toSrt}
          <label class="sub">{t("comicOriginal")}
            <select bind:value={r.original}>
              {@render options("original")}
            </select>
          </label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.utf8} /> <span>{t("subUtf8")}<small>{t("subUtf8Hint")}</small></span></label>
        <h4 class="sect">{t("sectClean")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.ads} /> <span>{t("subAds")}<small>{t("subAdsHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.hearing} /> <span>{t("subHearing")}<small>{t("subHearingHint")}</small></span></label>
        <h4 class="sect">{t("sectTime")}</h4>
        <label class="shift">{t("subShift")}
          <input
            type="number"
            step="0.1"
            {...seconds(lim("shiftMs"))}
            value={r.shiftMs / 1000}
            oninput={(e) => {
              const v = Number((e.currentTarget as HTMLInputElement).value);
              r.shiftMs = Number.isFinite(v) ? Math.round(v * 1000) : 0;
            }}
          />
        </label>
        <p class="hint">{t("subShiftHint")}</p>
        <label>{t("subFps")}
          <select
            value={r.fpsFrom ? `${r.fpsFrom}:${r.fpsTo}` : ""}
            onchange={(e) => {
              const [a, b] = (e.currentTarget as HTMLSelectElement).value.split(":").map(Number);
              r.fpsFrom = a || 0;
              r.fpsTo = b || 0;
            }}
          >
            <option value="">{t("subFpsNone")}</option>
            {#each TABLE.fpsPairs as [a, b] (`${a}:${b}`)}<option value={`${a}:${b}`}>{t("subFpsPair", { from: fps(a), to: fps(b) })}</option>{/each}
          </select>
        </label>
        <p class="hint">{t("subFpsHint")}</p>
        <h4 class="sect">{t("sectExtract")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.extract} /> <span>{t("subExtract")}<small>{t("subExtractHint")}</small></span></label>
        {#if r.extract}
          <label class="sub">{t("subExtractLangs")} <input class="mono" bind:value={r.extractLangs} placeholder={t("subExtractAll")} spellcheck="false" /></label>
        {/if}
      {:else if r.type === "music"}
        <p class="hint top">{t("musicHint")}</p>
        <h4 class="sect">{t("sectName")}</h4>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("musicTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colonAs} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.discPrefix} /> <span>{t("musicDiscPrefix")}<small>{t("musicDiscPrefixHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.fromName} /> <span>{t("musicFromName")}<small>{t("musicFromNameHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{t("musicFolders")}<small>{t("musicFoldersHint")}</small></span></label>
        {#if r.folders}
          <label class="sub">{t("musicFolderName")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <h4 class="sect">{t("sectBeside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.playlist} /> <span>{t("musicPlaylist")}<small>{t("musicPlaylistHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.cover} /> <span>{t("musicCover")}<small>{t("musicCoverHint")}</small></span></label>
        <h4 class="sect">{t("sectSearch")}</h4>
        <p class="hint top">{t("mbHint")}</p>
        <label class="check"><input type="checkbox" bind:checked={r.identifyBySound} /> <span>{t("musicListen")}<small>{t("musicListenHint")}</small></span></label>
        {@render actions(t("mbSearch"), "albumSummary")}
        <h4 class="sect">{t("sectInside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.writeTags} /> <span>{t("musicWriteTags")}<small>{t("musicWriteTagsHint")}</small></span></label>
        {#if r.writeTags}
          <label class="check sub"><input type="checkbox" bind:checked={r.embedCover} /> <span>{t("musicEmbedCover")}<small>{t("musicEmbedCoverHint")}</small></span></label>
          {#if r.embedCover}
            <label class="sub">{t("musicCoverSize")}
              <select bind:value={r.coverSize}>
                {#each choicesOf("music", "coverSize") as n (n)}<option value={n}>{n === 1200 ? t("musicCoverSize1200") : `${n} px`}</option>{/each}
              </select>
            </label>
          {/if}
          <label class="check sub"><input type="checkbox" bind:checked={r.cleanTags} /> <span>{t("musicCleanTags")}<small>{t("musicCleanTagsHint")}</small></span></label>
        {/if}
        <h4 class="sect">{t("sectList")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{t("musicMissingLabel")}<small>{t("musicMissingHint")}</small></span></label>
      {:else if r.type === "book"}
        <p class="hint top">{t("bookHint")}</p>
        <h4 class="sect">{t("sectName")}</h4>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("bookTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colonAs} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{t("bookFolders")}<small>{t("bookFoldersHint")}</small></span></label>
        {#if r.folders}
          <label class="sub">{t("musicFolderName")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.audiobooks} /> <span>{t("bookAudio")}<small>{t("bookAudioHint")}</small></span></label>
        <h4 class="sect">{t("sectSearch")}</h4>
        {@render actions(t("olSearch"), "bookSummary")}
        <h4 class="sect">{t("sectBeside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.cover} /> <span>{t("bookCover")}<small>{t("bookCoverHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.metadataOpf} /> <span>{t("bookOpf")}<small>{t("bookOpfHint")}</small></span></label>
        <h4 class="sect">{t("sectInside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.writeEpub} /> <span>{t("bookWriteEpub")}<small>{t("bookWriteEpubHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.epub3} /> <span>{t("bookEpub3")}<small>{t("bookEpub3Hint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.shrinkImages} /> <span>{t("bookShrink")}<small>{t("bookShrinkHint")}</small></span></label>
        {#if r.audiobooks}
          <label class="check"><input type="checkbox" bind:checked={r.audioTags} /> <span>{t("bookAudioTags")}<small>{t("bookAudioTagsHint")}</small></span></label>
          {#if r.audioTags}
            <label class="check sub"><input type="checkbox" bind:checked={r.audioClean} /> <span>{t("bookAudioClean")}<small>{t("bookAudioCleanHint")}</small></span></label>
          {/if}
        {/if}
        <h4 class="sect">{t("sectConvert")}</h4>
        <label>{t("bookConvert")}
          <select bind:value={r.convertTo}>
            {#each menuOf("book", "convertTo") as c (c.value)}
              {#if c.value !== "pdf" || canPrint || r.convertTo === "pdf"}<option value={c.value}>{c.value ? formatOption(t(c.label as Key), c.value as Format, bookDev, r.model, r.otherReaderApps) : t(c.label as Key)}</option>{/if}
            {/each}
          </select>
        </label>
        <p class="hint">{t("bookConvertHint")}</p>
        {#if r.convertTo}
          <label class="sub">{t("readerForOptional")}
            <select bind:value={r.model}>
              <option value="">{t("readerAny")}</option>
              {@render readerOptions(false)}
            </select>
          </label>
          {@const why = notOpens(r.convertTo, bookDev, r.model, r.otherReaderApps)}
          {#if why}<p class="warn sub">{why}</p>{/if}
          {#if bookDev}
            <label class="check sub"><input type="checkbox" bind:checked={r.otherReaderApps} /> <span>{t("comicAnyProgram")}<small>{t("comicAnyProgramHint")}</small></span></label>
          {/if}
          <p class="hint sub">{t(r.convertTo === "pdf" ? (bookDev ? "bookPdfScreen" : "bookPdfA5") : "bookReaderHint")}</p>
          <label class="sub">{t("comicOriginal")}
            <select bind:value={r.original}>
              {@render options("original")}
            </select>
          </label>
        {/if}
        <h4 class="sect">{t("sectList")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{t("bookMissingLabel")}<small>{t("bookMissingHint")}</small></span></label>
      {:else if r.type === "comic"}
        <p class="hint top">{t("comicHint")}</p>
        <label>{t("comicPreset")}
          <select
            value={comicPresetOf(r)}
            onchange={(e) => {
              const v = (e.currentTarget as HTMLSelectElement).value as ComicPreset | "";
              if (v) Object.assign(r, comicPreset(v));
            }}
          >
            <option value="" disabled>{t("comicPresetCustom")}</option>
            {#each Object.keys(TABLE.comicPresets) as p (p)}<option value={p}>{t(`comicPreset${p[0].toUpperCase()}${p.slice(1)}` as Key)}</option>{/each}
          </select>
        </label>
        <p class="hint">{t("comicPresetHint")}</p>
        <h4 class="sect">{t("sectName")}</h4>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("comicTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colonAs} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.seriesByFolder} /> <span>{t("comicByFolder")}<small>{t("comicByFolderHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{t("comicFolders")}<small>{t("comicFoldersHint")}</small></span></label>
        {#if r.folders}
          <label class="sub">{t("musicFolderName")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <h4 class="sect">{t("sectSearch")}</h4>
        <label>{t("comicSource")}
          <select bind:value={r.source}>
            {@render options("source")}
          </select>
        </label>
        <p class="hint">{t(SOURCE_HINT[r.source] ?? "gcdHint")}</p>
        {#if applies("barcode")}
          <label class="check"><input type="checkbox" bind:checked={r.barcode} /> <span>{t("comicBarcodeLabel")}<small>{t("comicBarcodeHint")}</small></span></label>
        {/if}
        <label>{t("comicSeries")} <input bind:value={r.series} placeholder={t("comicSeriesPlaceholder")} spellcheck="false" /></label>
        {#if lookup?.show}<p class="hint">{t("seriesCurrent", { show: lookup.show })}</p>{/if}
        <label>{t("comicSkipPublishers")} <input bind:value={r.skipPublishers} placeholder={t("comicSkipPublishersPlaceholder")} spellcheck="false" /></label>
        <p class="hint">{t("comicSkipPublishersHint")}</p>
        {@render actions(t("comicSearch"), "comicSummary")}
        <h4 class="sect">{t("sectInside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.writeInfo} /> <span>{t("comicWriteInfo")}<small>{t("comicWriteInfoHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.cleanInfo} /> <span>{t(r.writeInfo ? "comicCleanInfoWrite" : "comicCleanInfo")}<small>{t(r.writeInfo ? "comicCleanInfoWriteHint" : "comicCleanInfoHint")}</small></span></label>
        <h4 class="sect">{t("sectConvert")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.convert} /> <span>{t("comicConvert")}<small>{t("comicConvertHint")}</small></span></label>
        {#if r.convert}
          <label class="sub">{t("comicOriginal")}
            <select bind:value={r.original}>
              {@render options("original")}
            </select>
          </label>
          <label class="sub" title={t("comicDrawPagesHint")}>{t("comicDrawPages")}
            <select bind:value={r.drawPdfPages}>
              <option value={true}>{t("comicDrawPagesYes")}</option>
              <option value={false}>{t("comicDrawPagesNo")}</option>
            </select>
          </label>
          {#if r.drawPdfPages}
            <label class="sub" title={t("comicPageSizeHint")}>{t("comicPageSize")}
              <select bind:value={r.pageSize}>
                {#each choicesOf("comic", "pageSize") as n (n)}<option value={n}>{n === 2400 ? t("comicPageSize2400") : `${n} px`}</option>{/each}
              </select>
            </label>
          {/if}
        {/if}
        <h4 class="sect">{t("sectEbook")}</h4>
        <label>{t("comicEbook")}
          <select bind:value={r.ebook}>
            {#each menuOf("comic", "ebook") as c (c.value)}<option value={c.value}>{c.value ? formatOption(t(c.label as Key), c.value as Format, r.device, r.model, r.otherReaderApps) : t(c.label as Key)}</option>{/each}
          </select>
        </label>
        <p class="hint">{t("comicEbookHint")}</p>
        {#if r.ebook}
          <label class="sub">{t("readerFor")}
            <select
              value={readerOf(r.device, r.model)}
              onchange={(e) => {
                const c = chooseReader((e.currentTarget as HTMLSelectElement).value);
                r.device = c.device;
                r.model = c.model;
              }}
            >
              {@render readerOptions(true)}
              <optgroup label={t("comicDevices_other")}>
                {#each OTHER_DEVICES as d (d)}<option value={d}>{t(`comicDevice_${d}` as Key)}</option>{/each}
              </optgroup>
            </select>
          </label>
          {@const why = notOpens(r.ebook, r.device, r.model, r.otherReaderApps)}
          {#if why}<p class="warn sub">{why}</p>{/if}
          {#if formatsOf(r.device, r.model)}
            <label class="check sub"><input type="checkbox" bind:checked={r.otherReaderApps} /> <span>{t("comicAnyProgram")}<small>{t("comicAnyProgramHint")}</small></span></label>
          {/if}
          {#if r.device === "custom"}
            <div class="sub size">
              <label>{t("comicWidth")} <input type="number" {...lim("width")} bind:value={r.width} /></label>
              <label>{t("comicHeight")} <input type="number" {...lim("height")} bind:value={r.height} /></label>
            </div>
          {/if}
          <label class="check sub"><input type="checkbox" bind:checked={r.crop} /> <span>{t("comicCrop")}<small>{t("comicCropHint")}</small></span></label>
          {#if r.crop}
            <label class="check sub2"><input type="checkbox" bind:checked={r.cropPageNumbers} /> <span>{t("comicNumbers")}<small>{t("comicNumbersHint")}</small></span></label>
          {/if}
          <label class="sub">{t("comicSpread")}
            <select bind:value={r.spread}>
              {@render options("spread")}
            </select>
          </label>
          <p class="hint sub">{t("comicSpreadHint")}</p>
          <label class="check sub"><input type="checkbox" bind:checked={r.contrast} /> <span>{t("comicContrast")}<small>{t("comicContrastHint")}</small></span></label>
          {#if applies("upscale")}
            <label class="check sub"><input type="checkbox" bind:checked={r.upscale} /> <span>{t("comicUpscale")}<small>{t("comicUpscaleHint")}</small></span></label>
          {/if}
          {#if applies("sixteenGreys")}
            <label class="check sub"><input type="checkbox" bind:checked={r.sixteenGreys} /> <span>{t("comicGreys16")}<small>{t("comicGreys16Hint")}</small></span></label>
          {/if}
          {#if applies("reduceRainbow")}
            <label class="check sub"><input type="checkbox" bind:checked={r.reduceRainbow} /> <span>{t("comicSoften")}<small>{t("comicSoftenHint")}</small></span></label>
          {/if}
          <label class="check sub"><input type="checkbox" bind:checked={r.webtoon} /> <span>{t("comicWebtoon")}<small>{t("comicWebtoonHint")}</small></span></label>
          <label class="check sub"><input type="checkbox" bind:checked={r.join} /> <span>{t("comicJoin")}<small>{t("comicJoinHint")}</small></span></label>
          <label class="sub">{t("comicMaxMb")}
            <select bind:value={r.maxMb}>
              {#each choicesOf("comic", "maxMb") as n (n)}<option value={n}>{n === 0 ? t("comicMaxMbNone") : n === 50 ? t("comicMaxMb50") : n === 200 ? t("comicMaxMb200") : `${n} MB`}</option>{/each}
            </select>
          </label>
          {#if r.maxMb}<p class="hint sub">{t("comicMaxMbHint")}</p>{/if}
          {#if !r.convert}
            <label class="sub">{t("comicOriginal")}
              <select bind:value={r.original}>
                {@render options("original")}
              </select>
            </label>
          {/if}
        {/if}
        <h4 class="sect">{t("sectList")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{t("comicMissingLabel")}<small>{t("comicMissingHint")}</small></span></label>
      {/if}

      {#if !byKind(r.type)}
        <label class="only">{t("onlyFiles")} <input bind:value={step.only} spellcheck="false" placeholder={t("onlyFilesPlaceholder")} title={t("onlyFilesHint")} /></label>
      {/if}
      {#if summary}<p class="hint">{summary}</p>{/if}
      {#if error}<p class="error">{error}</p>{/if}
    </fieldset>
  {/if}
</article>

{#snippet readerOptions(sizes: boolean)}
  <optgroup label={t("readerPopular")}>
    {#each POPULAR as m (m.id)}<option value={m.id}>{sizes ? readerLabel(m) : modelName(m)}</option>{/each}
  </optgroup>
  {#each READERS as [group, , list] (group)}
    <optgroup label={t(group as Key)}>
      {#each list as m (m.id)}<option value={m.id}>{sizes ? readerLabel(m) : modelName(m)}</option>{/each}
    </optgroup>
  {/each}
{/snippet}

{#snippet videoOptions()}
  {#if r.type === "movie" || r.type === "series"}
    <label class="check sub"><input type="checkbox" bind:checked={r.videoClean} /> <span>{t("videoCleanLabel")}<small>{t("videoCleanHint")}</small></span></label>
    <label class="check sub"><input type="checkbox" bind:checked={r.videoTracks} /> <span>{t("videoTracksLabel")}<small>{t("videoTracksHint")}</small></span></label>
  {/if}
{/snippet}

{#snippet options(name: string)}
  {#each menuOf(r.type, name) as c (c.value)}<option value={c.value}>{t(c.label as Key)}</option>{/each}
{/snippet}

{#snippet subtitles()}
  {#if r.type === "movie" || r.type === "series"}
    <label class="check"><input type="checkbox" bind:checked={r.subtitles} /> <span>{t("subsLabel")}<small>{t("subsHint")}</small></span></label>
    {#if r.subtitles}
      <label class="sub">{t("subsLangs")} <input bind:value={r.subLangs} placeholder="pt-BR, en" spellcheck="false" /></label>
      {#if subs}
        <div class="movie-actions sub">
          <button disabled={!!subs.progress} onclick={subs.onsearch}>{t("subsSearch")}</button>
          {#if subs.found}<button onclick={subs.onreview}>{t("subsReview")}</button>{/if}
        </div>
        {#if subs.progress || subs.summary}<p class="hint">{subs.progress ?? subs.summary}</p>{/if}
      {/if}
    {/if}
  {/if}
{/snippet}

{#snippet actions(label: string, summary: "movieSummary" | "seriesSummary" | "albumSummary" | "bookSummary" | "comicSummary")}
  {#if lookup}
    <div class="movie-actions">
      <button class="primary-soft" disabled={!!lookup.progress} onclick={lookup.onsearch}>{label}</button>
      {#if lookup.choose || lookup.none}
        <button onclick={lookup.onreview}>{t("movieReview")} ({lookup.choose + lookup.none})</button>
      {/if}
    </div>
    {#if lookup.progress}
      <p class="hint">{lookup.progress}</p>
    {:else if lookup.found || lookup.choose || lookup.none}
      <p class="hint">{t(summary, { found: lookup.found, choose: lookup.choose, none: lookup.none })}</p>
    {/if}
    {#if lookup.edit}
      <div class="movie-actions">
        <button disabled={!lookup.edit.n || !!lookup.progress} onclick={lookup.edit.onclick}>{lookup.edit.label}</button>
      </div>
      <p class="hint">{lookup.edit.hint}{lookup.edit.typed ? ` ${t("editTypedN", { n: lookup.edit.typed })}` : ""}</p>
    {/if}
  {/if}
{/snippet}

{#snippet target()}
  {#if "target" in r}
    <label>{t("applyTo")}
      <select bind:value={r.target}>
        {@render options("target")}
      </select>
    </label>
  {/if}
{/snippet}

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    transition: opacity 0.15s;
  }
  .card.off { opacity: 0.55; }
  .card.has-error { border-color: var(--danger); }
  header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
  }
  .title {
    flex: 1;
    text-align: left;
    background: none;
    border: none;
    font-weight: 600;
    padding: 2px 4px;
    color: var(--text);
    /* Long titles shrink so the buttons stay visible in a narrow panel. */
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title:hover:not(:disabled) { background: none; }
  .brief {
    margin: -4px 0 0;
    padding: 0 12px 8px 46px;
    font-size: var(--fs-sm);
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chev { display: inline-flex; vertical-align: -2px; transition: transform 0.15s var(--ease); color: var(--muted); }
  .chev.closed { transform: rotate(-90deg); }
  .actions { display: flex; gap: 2px; opacity: 0.45; transition: opacity 0.12s; }
  .card:hover .actions, .card:focus-within .actions { opacity: 1; }
  .actions button { border-color: transparent; background: transparent; color: var(--muted); }
  .actions button:hover:not(:disabled) { color: var(--text); background: var(--hover); }
  .body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px 12px 12px;
    margin: 0;
    border: none;
    min-width: 0;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: var(--fs-sm);
    color: var(--muted);
    flex: 1;
    min-width: 0;
  }
  label.narrow { flex: 0 0 80px; }
  textarea.list {
    font: inherit;
    font-family: ui-monospace, "JetBrains Mono", "DejaVu Sans Mono", monospace;
    color: var(--text);
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface-2);
    resize: vertical;
    min-height: 80px;
    user-select: text;
  }
  button.fill { align-self: flex-start; }
  textarea.list:focus { outline: 2px solid var(--accent); outline-offset: -1px; }
  label.check {
    flex-direction: row;
    align-items: center;
    gap: 6px;
    color: var(--text);
    flex: 0 1 auto;
  }
  .row { display: flex; gap: 10px; align-items: flex-end; flex-wrap: wrap; }
  .checks { display: flex; flex-direction: column; gap: 4px; }
  .hint { margin: -4px 0 0; font-size: var(--fs-xs); color: var(--muted); }
  .warn { margin: -4px 0 0; font-size: var(--fs-xs); color: var(--warn-text); }
  /* A block of a long rule: a quiet title with a line. */
  .sect {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 6px 0 -2px;
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .sect::after { content: ""; flex: 1; height: 1px; background: var(--row-line); }
  .error { margin: 0; font-size: var(--fs-sm); color: var(--danger); }
  .sep { white-space: pre; }
  .hint.top { margin: 0; }
  label.colon { flex: none; align-self: flex-start; }
  label.colon input { width: 90px; }
  label.check span { display: flex; flex-direction: column; }
  /* With a hint under it, the box lines up with the first line. */
  label.check:has(small) { align-items: flex-start; }
  label.check:has(small) > input { margin-top: 2px; }
  /* Long hints: two lines, the rest on a click. */
  label.check small {
    color: var(--muted);
    font-size: var(--fs-xs);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
  }
  p.hint:not(.mono) {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
  }
  p.hint.top { -webkit-line-clamp: 3; line-clamp: 3; }
  label.check small:global(.more), p.hint:global(.more) { cursor: pointer; }
  label.check small:global(.more):hover, p.hint:global(.more):hover { color: var(--text); }
  label.check small:global(.open), p.hint:global(.open) { -webkit-line-clamp: unset; line-clamp: unset; }
  label.sub { margin-left: 22px; }
  label.sub2 { margin-left: 44px; }
  .hint.sub { margin: -2px 0 0 22px; }
  .size { display: flex; gap: 8px; margin-left: 22px; }
  .size label { flex: 1; }
  .size input { width: 100%; }
  label.shift { flex-direction: row; align-items: center; gap: 8px; }
  label.shift input { width: 70px; }
  .movie-actions { display: flex; gap: 8px; flex-wrap: wrap; }
  .parts { display: flex; flex-direction: column; gap: 3px; }
  .parts .hint { margin: 0 0 2px; }
  .part {
    display: flex;
    align-items: center;
    gap: 8px;
    height: auto;
    min-height: 26px;
    padding: 2px 8px 2px 4px;
    text-align: left;
    font-size: var(--fs-sm);
    white-space: normal;
  }
  .part b {
    flex: none;
    display: inline-grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 4px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: var(--fs-xs);
  }
  .part span { overflow-wrap: anywhere; }
</style>
