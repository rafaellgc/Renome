<script lang="ts">
  import { slide } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { t, type Key } from "./i18n";
  import { TABLE, choicesOf, comicPreset, comicPresetOf, limitsOf, menuOf, type ComicPreset } from "./rules";
  import { settings } from "./settings.svelte";
  import type { RuleType, Step } from "./types";
  /** Rules that pick their own kind of file: no filter of their own (the
   *  Rust catalog's group). */
  const byKind = (type: RuleType) => TABLE.media.includes(type);
  /** A reader model's label: name, screen and whether it is colour. */
  const modelName = (m: Reader) => (m.name.startsWith("@") ? t(m.name.slice(1) as Key) : m.name);
  const readerLabel = (m: Reader) => {
    const [w, h] = SCREENS[m.device]?.size ?? [0, 0];
    return `${modelName(m)} · ${w} × ${h}${isColourInk(m.device) ? ` · ${t("comicColour")}` : ""}`;
  };
  import { api } from "./api";
  import { OTHER_DEVICES, POPULAR, READERS, SCREENS, chooseReader, isColourInk, type Reader } from "./readers";

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
  /** The options hidden as the rule is set (the Rust catalog says:
   *  `catalog::hidden`, those under another that leaves them nothing to
   *  do, and the inert ones, which the flows take as off). */
  let hidden = $state<string[]>([]);
  $effect(() => {
    const rule = $state.snapshot(step.rule);
    api.ruleHidden(rule).then((v) => (hidden = v), () => {});
  });
  const applies = (name: string) => !hidden.includes(name);
  /** An option's label and hint, as the Rust catalog names them (the
   *  command line's `rules` too). */
  const label = (name: string) => t((TABLE.labels[r.type]?.[name]?.[0] ?? name) as Key);
  const optHint = (name: string) => {
    const k = TABLE.labels[r.type]?.[name]?.[1];
    return k ? t(k as Key) : "";
  };
  /** A whole-number option's bounds in this rule (the Rust catalog's). */
  const lim = (name: string) => limitsOf(r.type, name);
  /** Bounds in milliseconds as a field in seconds. */
  const seconds = (b: { min?: number; max?: number }) => ({ min: b.min === undefined ? undefined : b.min / 1000, max: b.max === undefined ? undefined : b.max / 1000 });
  /** The reader a comic or book rule makes its e-book for and the formats
   *  it does not open (the Rust readers' check, the command line's too). */
  let reader = $state<Awaited<ReturnType<typeof api.ruleReader>>>({ reader: "", device: null, limited: false, marked: [], notOpens: null });
  $effect(() => {
    const rule = $state.snapshot(step.rule);
    if (rule.type === "comic" || rule.type === "book") api.ruleReader(rule).then((v) => (reader = v), () => {});
  });
  /** The screen of the reader a book rule converts for, if one is chosen. */
  const bookDev = $derived(reader.device ?? undefined);
  /** A format's name in a menu, marked when the reader does not open it as
   *  it comes (it can still be chosen). */
  const formatOption = (label: string, f: string) => (reader.marked.includes(f) ? `${label} — ${t("readerNotOpensShort")}` : label);
  /** Why the format chosen may not suit the reader, if it may not. */
  const why = $derived.by(() => {
    const n = reader.notOpens;
    if (!n) return "";
    const name = n.name.startsWith("@") ? t(n.name.slice(1) as Key) : n.name;
    return t("readerNotOpens", { reader: name, format: n.format.toUpperCase(), best: n.best.toUpperCase() });
  });

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
        <label>{label("find")} <input bind:value={r.find} spellcheck="false" /></label>
        <label>{label("replace")} <input bind:value={r.replace} spellcheck="false" /></label>
        <div class="row">
          <label class="check"><input type="checkbox" bind:checked={r.caseSensitive} /> {label("caseSensitive")}</label>
          <label class="check" title={optHint("firstOnly")}><input type="checkbox" bind:checked={r.firstOnly} /> {label("firstOnly")}</label>
        </div>
        {@render target()}
      {:else if r.type === "regex"}
        <label>{label("pattern")} <input class="mono" bind:value={r.pattern} spellcheck="false" placeholder={"(\\d+) - (.*)"} /></label>
        <label>{label("replace")} <input class="mono" bind:value={r.replace} spellcheck="false" placeholder="$2 ($1)" /></label>
        <div class="row">
          <label class="check"><input type="checkbox" bind:checked={r.caseSensitive} /> {label("caseSensitive")}</label>
          <label class="check" title={optHint("firstOnly")}><input type="checkbox" bind:checked={r.firstOnly} /> {label("firstOnly")}</label>
        </div>
        {@render target()}
        <p class="hint mono">{t("groupsHint")}</p>
      {:else if r.type === "rearrange"}
        <label>{label("separator")} <input class="mono sep" bind:value={r.separator} spellcheck="false" placeholder=" - " /></label>
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
        <label>{label("order")} <input class="mono" bind:value={r.order} spellcheck="false" placeholder="1 - 3 - 2" /></label>
        <p class="hint">{t("newOrderHint")}</p>
        {@render target()}
      {:else if r.type === "list"}
        <label>{t("listNames")}
          <textarea class="mono list" bind:value={r.names} spellcheck="false" rows="8" wrap="off"></textarea>
        </label>
        {#if !r.names.trim() && currentNames.length}
          <button class="fill" title={t("listFillHint")} onclick={() => (r.names = currentNames.join("\n"))}>{t("listFill")}</button>
        {/if}
        <label class="check" title={optHint("pairs")}><input type="checkbox" bind:checked={r.pairs} /> {label("pairs")}</label>
        <p class="hint">{t("listOrderHint")}</p>
      {:else if r.type === "case"}
        <label>{label("mode")}
          <select bind:value={r.mode}>
            {@render options("mode")}
          </select>
        </label>
        <label>{label("keepWords")} <input bind:value={r.keepWords} spellcheck="false" placeholder="swat, dvd" title={optHint("keepWords")} /></label>
        {@render target()}
      {:else if r.type === "insert"}
        <label>{label("source")}
          <select bind:value={r.source}>
            {@render options("source")}
          </select>
        </label>
        {#if applies("text")}
          <label>{label("text")} <input bind:value={r.text} spellcheck="false" /></label>
        {/if}
        {#if applies("dateFormat")}
          <label>{label("dateFormat")} <input class="mono" bind:value={r.dateFormat} spellcheck="false" /></label>
          <p class="hint mono">{t("formatHint")}</p>
        {/if}
        {#if applies("fallback")}
          <label class="check"><input type="checkbox" bind:checked={r.fallback} /> {label("fallback")}</label>
          <p class="hint">{t("takenHint")}</p>
        {/if}
        <div class="row">
          <label>{label("at")}
            <select bind:value={r.at}>
              {@render options("at")}
            </select>
          </label>
          {#if applies("position")}
            <label class="narrow">{label("position")} <input type="number" {...lim("position")} bind:value={r.position} /></label>
          {/if}
        </div>
      {:else if r.type === "number"}
        <label>{label("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("templateHint")}</p>
        <div class="row">
          <label class="narrow">{label("start")} <input type="number" {...lim("start")} bind:value={r.start} /></label>
          <label class="narrow">{label("step")} <input type="number" {...lim("step")} bind:value={r.step} /></label>
          <label class="narrow">{label("digits")} <input type="number" {...lim("digits")} bind:value={r.digits} /></label>
        </div>
        <label class="check"><input type="checkbox" bind:checked={r.perFolder} /> {label("perFolder")}</label>
        <label class="check" title={optHint("pairs")}><input type="checkbox" bind:checked={r.pairs} /> {label("pairs")}</label>
      {:else if r.type === "remove"}
        <div class="checks">
          <label class="check"><input type="checkbox" bind:checked={r.extraSpaces} /> {label("extraSpaces")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.allSpaces} /> {label("allSpaces")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.accents} /> {label("accents")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.digits} /> {label("digits")}</label>
          <label class="check" title={optHint("leadingNumber")}><input type="checkbox" bind:checked={r.leadingNumber} /> {label("leadingNumber")}</label>
        </div>
        <label>{label("chars")} <input class="mono" bind:value={r.chars} spellcheck="false" placeholder="_#[]" /></label>
        <div class="row">
          <label>{label("maxLength")} <input type="number" {...lim("maxLength")} bind:value={r.maxLength} /></label>
        </div>
        {@render target()}
      {:else if r.type === "convert"}
        <div class="checks">
          <label class="check" title={optHint("mojibake")}><input type="checkbox" bind:checked={r.mojibake} /> {label("mojibake")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.transliterate} /> {label("transliterate")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.splitWords} /> {label("splitWords")}</label>
        </div>
        <label>{label("webSafe")}
          <select bind:value={r.webSafe} title={optHint("webSafe")}>
            {@render options("webSafe")}
          </select>
        </label>
        {@render target()}
      {:else if r.type === "removeRange"}
        <div class="row">
          <label>{label("from")}
            <select bind:value={r.from}>
              {@render options("from")}
            </select>
          </label>
          {#if applies("fromPosition")}
            <label class="narrow">{label("fromPosition")} <input type="number" {...lim("fromPosition")} bind:value={r.fromPosition} /></label>
          {:else if applies("fromText")}
            <label>{label("fromText")} <input bind:value={r.fromText} spellcheck="false" /></label>
          {/if}
        </div>
        <div class="row">
          <label>{label("to")}
            <select bind:value={r.to}>
              {@render options("to")}
            </select>
          </label>
          {#if applies("toPosition")}
            <label class="narrow">{label("toPosition")} <input type="number" {...lim("toPosition")} bind:value={r.toPosition} /></label>
          {:else if applies("toText")}
            <label>{label("toText")} <input bind:value={r.toText} spellcheck="false" /></label>
          {/if}
        </div>
        {#if applies("inclusive")}
          <label class="check"><input type="checkbox" bind:checked={r.inclusive} /> {label("inclusive")}</label>
        {/if}
      {:else if r.type === "charAt"}
        <div class="row">
          <label class="narrow">{label("position")} <input type="number" {...lim("position")} bind:value={r.position} /></label>
          <label class="check"><input type="checkbox" bind:checked={r.fromEnd} /> {label("fromEnd")}</label>
        </div>
        <div class="row">
          <label>{label("onlyIf")} <input bind:value={r.onlyIf} placeholder={t("any")} /></label>
          <label>{label("replace")} <input bind:value={r.replace} placeholder={t("deleteIt")} /></label>
        </div>
        {#if applies("caseSensitive")}
          <label class="check"><input type="checkbox" bind:checked={r.caseSensitive} /> {label("caseSensitive")}</label>
        {/if}
      {:else if r.type === "extension"}
        <div class="row">
          <label>{label("mode")}
            <select bind:value={r.mode}>
              {@render options("mode")}
            </select>
          </label>
          {#if applies("value")}
            <label>{label("value")} <input bind:value={r.value} spellcheck="false" placeholder="jpg" /></label>
          {/if}
        </div>
        {#if r.mode === "content"}
          <p class="hint">{t("extContentHint")}</p>
        {/if}
      {:else if r.type === "movie"}
        <p class="hint top">{t("movieHint")}</p>
        <h4 class="sect">{t("sectName")}</h4>
        <label>{label("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("movieTemplateHint")}</p>
        <label class="colon">{label("colonAs")} <input class="mono sep" bind:value={r.colonAs} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.folder} /> <span>{label("folder")}<small>{optHint("folder")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.trashAds} /> <span>{label("trashAds")}<small>{optHint("trashAds")}</small></span></label>
        <h4 class="sect">{t("sectSearch")}</h4>
        {@render actions(t("tmdbSearch"), "movieSummary")}
        <h4 class="sect">{t("sectBeside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.nfo} /> <span>{label("nfo")}<small>{optHint("nfo")}</small></span></label>
        {#if applies("backdrop")}
          <label class="check sub"><input type="checkbox" bind:checked={r.backdrop} /> {label("backdrop")}</label>
        {/if}
        <h4 class="sect">{t("sectInside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.videoTags} /> <span>{label("videoTags")}<small>{optHint("videoTags")}</small></span></label>
        {#if applies("videoClean")}{@render videoOptions()}{/if}
        <h4 class="sect">{t("sectList")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{label("missing")}<small>{optHint("missing")}</small></span></label>
        <h4 class="sect">{t("sectSubs")}</h4>
        {@render subtitles()}
      {:else if r.type === "series"}
        <p class="hint top">{t("seriesHint")}</p>
        <h4 class="sect">{t("sectName")}</h4>
        <label>{label("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("seriesTemplateHint")}</p>
        <label class="colon">{label("colonAs")} <input class="mono sep" bind:value={r.colonAs} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{label("folders")}<small>{optHint("folders")}</small></span></label>
        {#if applies("folderTemplate")}
          <label class="sub">{label("folderTemplate")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.trashAds} /> <span>{label("trashAds")}<small>{optHint("trashAds")}</small></span></label>
        <h4 class="sect">{t("sectSearch")}</h4>
        <label>{label("source")}
          <select bind:value={r.source}>
            {@render options("source")}
          </select>
        </label>
        {#if r.source === "anilist"}<p class="hint">{t("animeHint")}</p>{/if}
        <label>{label("show")} <input bind:value={r.show} placeholder={t(r.source === "anilist" ? "seriesShowPlaceholderAnilist" : "seriesShowPlaceholder")} spellcheck="false" /></label>
        {#if lookup?.show}<p class="hint">{t("seriesCurrent", { show: lookup.show })}</p>{/if}
        {@render actions(t("tvSearch"), "seriesSummary")}
        {#if applies("nfo")}
          <h4 class="sect">{t("sectBeside")}</h4>
          <label class="check"><input type="checkbox" bind:checked={r.nfo} /> <span>{label("nfo")}<small>{optHint("nfo")}</small></span></label>
          {#if applies("backdrop")}
            <label class="check sub"><input type="checkbox" bind:checked={r.backdrop} /> {label("backdrop")}</label>
          {/if}
          <h4 class="sect">{t("sectInside")}</h4>
          <label class="check"><input type="checkbox" bind:checked={r.videoTags} /> <span>{label("videoTags")}<small>{optHint("videoTags")}</small></span></label>
          {#if applies("videoClean")}{@render videoOptions()}{/if}
        {/if}
        <h4 class="sect">{t("sectList")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{label("missing")}<small>{optHint("missing")}</small></span></label>
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
        <label>{label("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("photoTemplateHint")}</p>
        <h4 class="sect">{t("sectDate")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.fromName} /> <span>{label("fromName")}<small>{optHint("fromName")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.modified} /> <span>{label("modified")}<small>{optHint("modified")}</small></span></label>
        <label class="shift">{label("shiftHours")}
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
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{label("folders")}<small>{optHint("folders")}</small></span></label>
        {#if applies("folderTemplate")}
          <label class="sub">{t("photoFolderReady")}
            <select value={ready.folders.includes(r.folderTemplate as never) ? r.folderTemplate : ""} onchange={(e) => { const v = (e.currentTarget as HTMLSelectElement).value; if (v) r.folderTemplate = v; }}>
              <option value="" disabled>{t("comicPresetCustom")}</option>
              {#each ready.folders as f (f)}<option value={f}>{f}</option>{/each}
            </select>
          </label>
          <label class="sub">{label("folderTemplate")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <h4 class="sect">{t("sectInside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.setModified} /> <span>{label("setModified")}<small>{optHint("setModified")}</small></span></label>
        <p class="hint">{t("photoCopiesHint")}</p>
      {:else if r.type === "subtitle"}
        {@const fps = (n: number) => n.toLocaleString(settings.lang === "en" ? "en-US" : "pt-BR")}
        <p class="hint top">{t("subToolsHint")}</p>
        <h4 class="sect">{t("sectFormat")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.toSrt} /> <span>{label("toSrt")}<small>{optHint("toSrt")}</small></span></label>
        {#if applies("original")}
          <label class="sub">{label("original")}
            <select bind:value={r.original}>
              {@render options("original")}
            </select>
          </label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.utf8} /> <span>{label("utf8")}<small>{optHint("utf8")}</small></span></label>
        <h4 class="sect">{t("sectClean")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.ads} /> <span>{label("ads")}<small>{optHint("ads")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.hearing} /> <span>{label("hearing")}<small>{optHint("hearing")}</small></span></label>
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
        <label class="check"><input type="checkbox" bind:checked={r.extract} /> <span>{label("extract")}<small>{optHint("extract")}</small></span></label>
        {#if applies("extractLangs")}
          <label class="sub">{label("extractLangs")} <input class="mono" bind:value={r.extractLangs} placeholder={t("subExtractAll")} spellcheck="false" /></label>
        {/if}
      {:else if r.type === "music"}
        <p class="hint top">{t("musicHint")}</p>
        <h4 class="sect">{t("sectName")}</h4>
        <label>{label("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("musicTemplateHint")}</p>
        <label class="colon">{label("colonAs")} <input class="mono sep" bind:value={r.colonAs} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.discPrefix} /> <span>{label("discPrefix")}<small>{optHint("discPrefix")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.fromName} /> <span>{label("fromName")}<small>{optHint("fromName")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{label("folders")}<small>{optHint("folders")}</small></span></label>
        {#if applies("folderTemplate")}
          <label class="sub">{label("folderTemplate")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <h4 class="sect">{t("sectBeside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.playlist} /> <span>{label("playlist")}<small>{optHint("playlist")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.cover} /> <span>{label("cover")}<small>{optHint("cover")}</small></span></label>
        <h4 class="sect">{t("sectSearch")}</h4>
        <p class="hint top">{t("mbHint")}</p>
        <label class="check"><input type="checkbox" bind:checked={r.identifyBySound} /> <span>{label("identifyBySound")}<small>{optHint("identifyBySound")}</small></span></label>
        {@render actions(t("mbSearch"), "albumSummary")}
        <h4 class="sect">{t("sectInside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.writeTags} /> <span>{label("writeTags")}<small>{optHint("writeTags")}</small></span></label>
        {#if applies("embedCover")}
          <label class="check sub"><input type="checkbox" bind:checked={r.embedCover} /> <span>{label("embedCover")}<small>{optHint("embedCover")}</small></span></label>
          {#if applies("coverSize")}
            <label class="sub">{label("coverSize")}
              <select bind:value={r.coverSize}>
                {#each choicesOf("music", "coverSize") as n (n)}<option value={n}>{n === 1200 ? t("musicCoverSize1200") : `${n} px`}</option>{/each}
              </select>
            </label>
          {/if}
          <label class="check sub"><input type="checkbox" bind:checked={r.cleanTags} /> <span>{label("cleanTags")}<small>{optHint("cleanTags")}</small></span></label>
        {/if}
        <h4 class="sect">{t("sectList")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{label("missing")}<small>{optHint("missing")}</small></span></label>
      {:else if r.type === "book"}
        <p class="hint top">{t("bookHint")}</p>
        <h4 class="sect">{t("sectName")}</h4>
        <label>{label("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("bookTemplateHint")}</p>
        <label class="colon">{label("colonAs")} <input class="mono sep" bind:value={r.colonAs} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{label("folders")}<small>{optHint("folders")}</small></span></label>
        {#if applies("folderTemplate")}
          <label class="sub">{label("folderTemplate")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.audiobooks} /> <span>{label("audiobooks")}<small>{optHint("audiobooks")}</small></span></label>
        <h4 class="sect">{t("sectSearch")}</h4>
        {@render actions(t("olSearch"), "bookSummary")}
        <h4 class="sect">{t("sectBeside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.cover} /> <span>{label("cover")}<small>{optHint("cover")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.metadataOpf} /> <span>{label("metadataOpf")}<small>{optHint("metadataOpf")}</small></span></label>
        <h4 class="sect">{t("sectInside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.writeEpub} /> <span>{label("writeEpub")}<small>{optHint("writeEpub")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.epub3} /> <span>{label("epub3")}<small>{optHint("epub3")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.shrinkImages} /> <span>{label("shrinkImages")}<small>{optHint("shrinkImages")}</small></span></label>
        {#if applies("audioTags")}
          <label class="check"><input type="checkbox" bind:checked={r.audioTags} /> <span>{label("audioTags")}<small>{optHint("audioTags")}</small></span></label>
          {#if applies("audioClean")}
            <label class="check sub"><input type="checkbox" bind:checked={r.audioClean} /> <span>{label("audioClean")}<small>{optHint("audioClean")}</small></span></label>
          {/if}
        {/if}
        <h4 class="sect">{t("sectConvert")}</h4>
        <label>{label("convertTo")}
          <select bind:value={r.convertTo}>
            {#each menuOf("book", "convertTo") as c (c.value)}
              {#if c.value !== "pdf" || canPrint || r.convertTo === "pdf"}<option value={c.value}>{c.value ? formatOption(t(c.label as Key), c.value) : t(c.label as Key)}</option>{/if}
            {/each}
          </select>
        </label>
        <p class="hint">{t("bookConvertHint")}</p>
        {#if applies("model")}
          <label class="sub">{label("model")}
            <select bind:value={r.model}>
              <option value="">{t("readerAny")}</option>
              {@render readerOptions(false)}
            </select>
          </label>
          {#if why}<p class="warn sub">{why}</p>{/if}
          {#if applies("otherReaderApps")}
            <label class="check sub"><input type="checkbox" bind:checked={r.otherReaderApps} /> <span>{label("otherReaderApps")}<small>{optHint("otherReaderApps")}</small></span></label>
          {/if}
          <p class="hint sub">{t(r.convertTo === "pdf" ? (bookDev ? "bookPdfScreen" : "bookPdfA5") : "bookReaderHint")}</p>
          <label class="sub">{label("original")}
            <select bind:value={r.original}>
              {@render options("original")}
            </select>
          </label>
        {/if}
        <h4 class="sect">{t("sectList")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{label("missing")}<small>{optHint("missing")}</small></span></label>
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
        <label>{label("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("comicTemplateHint")}</p>
        <label class="colon">{label("colonAs")} <input class="mono sep" bind:value={r.colonAs} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.seriesByFolder} /> <span>{label("seriesByFolder")}<small>{optHint("seriesByFolder")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{label("folders")}<small>{optHint("folders")}</small></span></label>
        {#if applies("folderTemplate")}
          <label class="sub">{label("folderTemplate")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <h4 class="sect">{t("sectSearch")}</h4>
        <label>{label("source")}
          <select bind:value={r.source}>
            {@render options("source")}
          </select>
        </label>
        <p class="hint">{t(SOURCE_HINT[r.source] ?? "gcdHint")}</p>
        {#if applies("barcode")}
          <label class="check"><input type="checkbox" bind:checked={r.barcode} /> <span>{label("barcode")}<small>{optHint("barcode")}</small></span></label>
        {/if}
        <label>{label("series")} <input bind:value={r.series} placeholder={t("comicSeriesPlaceholder")} spellcheck="false" /></label>
        {#if lookup?.show}<p class="hint">{t("seriesCurrent", { show: lookup.show })}</p>{/if}
        <label>{label("skipPublishers")} <input bind:value={r.skipPublishers} placeholder={t("comicSkipPublishersPlaceholder")} spellcheck="false" /></label>
        <p class="hint">{t("comicSkipPublishersHint")}</p>
        {@render actions(t("comicSearch"), "comicSummary")}
        <h4 class="sect">{t("sectInside")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.writeInfo} /> <span>{label("writeInfo")}<small>{optHint("writeInfo")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.cleanInfo} /> <span>{t(r.writeInfo ? "comicCleanInfoWrite" : "comicCleanInfo")}<small>{t(r.writeInfo ? "comicCleanInfoWriteHint" : "comicCleanInfoHint")}</small></span></label>
        <h4 class="sect">{t("sectConvert")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.convert} /> <span>{label("convert")}<small>{optHint("convert")}</small></span></label>
        {#if applies("drawPdfPages")}
          <label class="sub">{label("original")}
            <select bind:value={r.original}>
              {@render options("original")}
            </select>
          </label>
          <label class="sub" title={t("comicDrawPagesHint")}>{label("drawPdfPages")}
            <select bind:value={r.drawPdfPages}>
              <option value={true}>{t("comicDrawPagesYes")}</option>
              <option value={false}>{t("comicDrawPagesNo")}</option>
            </select>
          </label>
          {#if applies("pageSize")}
            <label class="sub" title={t("comicPageSizeHint")}>{label("pageSize")}
              <select bind:value={r.pageSize}>
                {#each choicesOf("comic", "pageSize") as n (n)}<option value={n}>{n === 2400 ? t("comicPageSize2400") : `${n} px`}</option>{/each}
              </select>
            </label>
          {/if}
        {/if}
        <h4 class="sect">{t("sectEbook")}</h4>
        <label>{label("ebook")}
          <select bind:value={r.ebook}>
            {#each menuOf("comic", "ebook") as c (c.value)}<option value={c.value}>{c.value ? formatOption(t(c.label as Key), c.value) : t(c.label as Key)}</option>{/each}
          </select>
        </label>
        <p class="hint">{t("comicEbookHint")}</p>
        {#if applies("device")}
          <label class="sub">{t("readerFor")}
            <select
              value={reader.reader || r.device}
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
          {#if why}<p class="warn sub">{why}</p>{/if}
          {#if applies("otherReaderApps")}
            <label class="check sub"><input type="checkbox" bind:checked={r.otherReaderApps} /> <span>{label("otherReaderApps")}<small>{optHint("otherReaderApps")}</small></span></label>
          {/if}
          {#if applies("width")}
            <div class="sub size">
              <label>{label("width")} <input type="number" {...lim("width")} bind:value={r.width} /></label>
              <label>{label("height")} <input type="number" {...lim("height")} bind:value={r.height} /></label>
            </div>
          {/if}
          <label class="check sub"><input type="checkbox" bind:checked={r.crop} /> <span>{label("crop")}<small>{optHint("crop")}</small></span></label>
          {#if applies("cropPageNumbers")}
            <label class="check sub2"><input type="checkbox" bind:checked={r.cropPageNumbers} /> <span>{label("cropPageNumbers")}<small>{optHint("cropPageNumbers")}</small></span></label>
          {/if}
          <label class="sub">{label("spread")}
            <select bind:value={r.spread}>
              {@render options("spread")}
            </select>
          </label>
          <p class="hint sub">{t("comicSpreadHint")}</p>
          <label class="check sub"><input type="checkbox" bind:checked={r.contrast} /> <span>{label("contrast")}<small>{optHint("contrast")}</small></span></label>
          {#if applies("upscale")}
            <label class="check sub"><input type="checkbox" bind:checked={r.upscale} /> <span>{label("upscale")}<small>{optHint("upscale")}</small></span></label>
          {/if}
          {#if applies("sixteenGreys")}
            <label class="check sub"><input type="checkbox" bind:checked={r.sixteenGreys} /> <span>{label("sixteenGreys")}<small>{optHint("sixteenGreys")}</small></span></label>
          {/if}
          {#if applies("reduceRainbow")}
            <label class="check sub"><input type="checkbox" bind:checked={r.reduceRainbow} /> <span>{label("reduceRainbow")}<small>{optHint("reduceRainbow")}</small></span></label>
          {/if}
          <label class="check sub"><input type="checkbox" bind:checked={r.webtoon} /> <span>{label("webtoon")}<small>{optHint("webtoon")}</small></span></label>
          <label class="check sub"><input type="checkbox" bind:checked={r.join} /> <span>{label("join")}<small>{optHint("join")}</small></span></label>
          <label class="sub">{label("maxMb")}
            <select bind:value={r.maxMb}>
              {#each choicesOf("comic", "maxMb") as n (n)}<option value={n}>{n === 0 ? t("comicMaxMbNone") : n === 50 ? t("comicMaxMb50") : n === 200 ? t("comicMaxMb200") : `${n} MB`}</option>{/each}
            </select>
          </label>
          {#if r.maxMb}<p class="hint sub">{t("comicMaxMbHint")}</p>{/if}
          {#if !r.convert}
            <label class="sub">{label("original")}
              <select bind:value={r.original}>
                {@render options("original")}
              </select>
            </label>
          {/if}
        {/if}
        <h4 class="sect">{t("sectList")}</h4>
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{label("missing")}<small>{optHint("missing")}</small></span></label>
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
    <label class="check sub"><input type="checkbox" bind:checked={r.videoClean} /> <span>{label("videoClean")}<small>{optHint("videoClean")}</small></span></label>
    <label class="check sub"><input type="checkbox" bind:checked={r.videoTracks} /> <span>{label("videoTracks")}<small>{optHint("videoTracks")}</small></span></label>
  {/if}
{/snippet}

{#snippet options(name: string)}
  {#each menuOf(r.type, name) as c (c.value)}<option value={c.value}>{t(c.label as Key)}</option>{/each}
{/snippet}

{#snippet subtitles()}
  {#if r.type === "movie" || r.type === "series"}
    <label class="check"><input type="checkbox" bind:checked={r.subtitles} /> <span>{label("subtitles")}<small>{optHint("subtitles")}</small></span></label>
    {#if applies("subLangs")}
      <label class="sub">{label("subLangs")} <input bind:value={r.subLangs} placeholder="pt-BR, en" spellcheck="false" /></label>
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
