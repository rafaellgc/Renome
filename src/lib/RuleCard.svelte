<script lang="ts">
  import { t, type Key } from "./i18n";
  import { comicPreset, comicPresetOf, defaultSubLangs, PHOTO_TEMPLATES, splitExt, splitParts, type ComicPreset } from "./rules";
  import { settings } from "./settings.svelte";
  import type { Step } from "./types";
  import { api } from "./api";

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

  const parts = $derived.by(() => {
    if (r.type !== "rearrange" || !sample) return [];
    const [stem, ext] = splitExt(sample);
    const text = r.target === "name" ? stem : r.target === "ext" ? ext : sample;
    return splitParts(text, r.separator);
  });

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
      <span class="chev" class:closed={!open}>▾</span>
      {t(`rule.${r.type}`)}
    </button>
    <div class="actions">
      <button class="icon" disabled={first} onclick={() => onmove(-1)} title={t("up")}>↑</button>
      <button class="icon" disabled={last} onclick={() => onmove(1)} title={t("down")}>↓</button>
      <button class="icon danger" onclick={onremove} title={t("removeRule")}>✕</button>
    </div>
  </header>

  {#if open}
    <!-- A rule turned off is left as it is: no edits, no searches. -->
    <fieldset class="body" disabled={!step.enabled}>
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
      {:else if r.type === "case"}
        <label>{t("mode")}
          <select bind:value={r.mode}>
            <option value="lower">{t("caseLower")}</option>
            <option value="upper">{t("caseUpper")}</option>
            <option value="sentence">{t("caseSentence")}</option>
            <option value="title">{t("caseTitle")}</option>
            <option value="titleSmart">{t("caseTitleSmart")}</option>
          </select>
        </label>
        {@render target()}
      {:else if r.type === "insert"}
        <label>{t("what")}
          <select bind:value={r.source}>
            <option value="text">{t("srcText")}</option>
            <option value="now">{t("srcNow")}</option>
            <option value="modified">{t("srcModified")}</option>
            <option value="taken">{t("srcTaken")}</option>
            <option value="folder">{t("srcFolder")}</option>
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
              <option value="start">{t("atStart")}</option>
              <option value="end">{t("atEnd")}</option>
              <option value="index">{t("atIndex")}</option>
              <option value="fromEnd">{t("atFromEnd")}</option>
            </select>
          </label>
          {#if r.at === "index" || r.at === "fromEnd"}
            <label class="narrow">{t("position")} <input type="number" min="0" bind:value={r.position} /></label>
          {/if}
        </div>
      {:else if r.type === "number"}
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("templateHint")}</p>
        <div class="row">
          <label class="narrow">{t("start")} <input type="number" bind:value={r.start} /></label>
          <label class="narrow">{t("step")} <input type="number" bind:value={r.step} /></label>
          <label class="narrow">{t("digits")} <input type="number" min="0" max="12" bind:value={r.digits} /></label>
        </div>
        <label class="check"><input type="checkbox" bind:checked={r.perFolder} /> {t("perFolder")}</label>
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
          <label>{t("maxLength")} <input type="number" min="0" bind:value={r.maxLength} /></label>
        </div>
        {@render target()}
      {:else if r.type === "convert"}
        <div class="checks">
          <label class="check" title={t("cvMojibakeHint")}><input type="checkbox" bind:checked={r.mojibake} /> {t("cvMojibake")}</label>
          <label class="check"><input type="checkbox" bind:checked={r.latin} /> {t("cvLatin")}</label>
        </div>
        <label>{t("cvWeb")}
          <select bind:value={r.web} title={t("cvWebHint")}>
            <option value="">{t("cvWebOff")}</option>
            <option value="-">{t("cvWebDash")}</option>
            <option value="_">{t("cvWebUnderscore")}</option>
          </select>
        </label>
        {@render target()}
      {:else if r.type === "removeRange"}
        <div class="row">
          <label>{t("from")}
            <select bind:value={r.from}>
              <option value="start">{t("bStart")}</option>
              <option value="position">{t("bPosition")}</option>
              <option value="text">{t("bText")}</option>
            </select>
          </label>
          {#if r.from === "position"}
            <label class="narrow">{t("position")} <input type="number" min="1" bind:value={r.fromPosition} /></label>
          {:else if r.from === "text"}
            <label>{t("text")} <input bind:value={r.fromText} spellcheck="false" /></label>
          {/if}
        </div>
        <div class="row">
          <label>{t("to")}
            <select bind:value={r.to}>
              <option value="end">{t("bEnd")}</option>
              <option value="position">{t("bPosition")}</option>
              <option value="text">{t("bText")}</option>
            </select>
          </label>
          {#if r.to === "position"}
            <label class="narrow">{t("position")} <input type="number" min="1" bind:value={r.toPosition} /></label>
          {:else if r.to === "text"}
            <label>{t("text")} <input bind:value={r.toText} spellcheck="false" /></label>
          {/if}
        </div>
        {#if r.from === "text" || r.to === "text"}
          <label class="check"><input type="checkbox" bind:checked={r.inclusive} /> {t("inclusive")}</label>
        {/if}
      {:else if r.type === "charAt"}
        <div class="row">
          <label class="narrow">{t("position")} <input type="number" min="1" bind:value={r.position} /></label>
          <label class="check"><input type="checkbox" bind:checked={r.fromEnd} /> {t("countFromEnd")}</label>
        </div>
        <div class="row">
          <label>{t("onlyIf")} <input bind:value={r.onlyIf} maxlength="4" placeholder={t("any")} /></label>
          <label>{t("replaceBy")} <input bind:value={r.replace} placeholder={t("deleteIt")} /></label>
        </div>
        {#if r.onlyIf}
          <label class="check"><input type="checkbox" bind:checked={r.caseSensitive} /> {t("caseSensitive")}</label>
        {/if}
      {:else if r.type === "extension"}
        <div class="row">
          <label>{t("action")}
            <select bind:value={r.mode}>
              <option value="lower">{t("extLower")}</option>
              <option value="upper">{t("extUpper")}</option>
              <option value="set">{t("extSet")}</option>
              <option value="remove">{t("extRemove")}</option>
              <option value="content">{t("extContent")}</option>
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
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("movieTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colon} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.folder} /> <span>{t("folderLabel")}<small>{t("folderHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.sidecar} /> <span>{t("sidecarLabel")}<small>{t("sidecarHint")}</small></span></label>
        {#if r.sidecar}
          <label class="check sub"><input type="checkbox" bind:checked={r.fanart} /> {t("fanartLabel")}</label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{t("movieMissingLabel")}<small>{t("movieMissingHint")}</small></span></label>
        {@render actions(t("tmdbSearch"), "movieSummary")}
        {@render subtitles()}
      {:else if r.type === "series"}
        <p class="hint top">{t("seriesHint")}</p>
        <label>{t("seriesSource")}
          <select bind:value={r.source}>
            <option value="tmdb">{t("seriesSourceTmdb")}</option>
            <option value="anilist">{t("seriesSourceAnilist")}</option>
          </select>
        </label>
        {#if r.source === "anilist"}<p class="hint">{t("animeHint")}</p>{/if}
        <label>{t("seriesShow")} <input bind:value={r.show} placeholder={t(r.source === "anilist" ? "seriesShowPlaceholderAnilist" : "seriesShowPlaceholder")} spellcheck="false" /></label>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("seriesTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colon} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{t("seriesFoldersLabel")}<small>{t("seriesFoldersHint")}</small></span></label>
        {#if r.folders}
          <label class="sub">{t("seriesFolderName")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        {#if r.source !== "anilist"}
          <label class="check"><input type="checkbox" bind:checked={r.sidecar} /> <span>{t("seriesSidecarLabel")}<small>{t("seriesSidecarHint")}</small></span></label>
          {#if r.sidecar}
            <label class="check sub"><input type="checkbox" bind:checked={r.fanart} /> {t("fanartLabel")}</label>
          {/if}
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{t("seriesMissingLabel")}<small>{t("seriesMissingHint")}</small></span></label>

        {#if lookup?.show}<p class="hint">{t("seriesCurrent", { show: lookup.show })}</p>{/if}
        {@render actions(t("tvSearch"), "seriesSummary")}
        {#if r.source !== "anilist"}{@render subtitles()}{/if}
      {:else if r.type === "photo"}
        {@const ready = PHOTO_TEMPLATES[settings.lang]}
        <p class="hint top">{t("photoHint")}</p>
        <label>{t("photoReady")}
          <select value={ready.names.includes(r.template as never) ? r.template : ""} onchange={(e) => { const v = (e.currentTarget as HTMLSelectElement).value; if (v) r.template = v; }}>
            <option value="" disabled>{t("comicPresetCustom")}</option>
            {#each ready.names as n, i (n)}<option value={n}>{t(`photoReady${i}` as Key)}</option>{/each}
          </select>
        </label>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("photoTemplateHint")}</p>
        <label class="check"><input type="checkbox" bind:checked={r.fromName} /> <span>{t("photoFromName")}<small>{t("photoFromNameHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.modified} /> <span>{t("photoModified")}<small>{t("photoModifiedHint")}</small></span></label>
        <label class="shift">{t("photoShift")}
          <input type="number" min="-48" max="48" step="1" bind:value={r.shiftHours} />
        </label>
        <p class="hint">{t("photoShiftHint")}</p>
        {#if edit}
          <div class="movie-actions">
            <button disabled={!edit.n} onclick={edit.onclick}>{edit.label}</button>
          </div>
          <p class="hint">{edit.hint}{edit.typed ? ` ${t("photoEditTypedN", { n: edit.typed })}` : ""}</p>
        {/if}
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
        <label class="check"><input type="checkbox" bind:checked={r.setModified} /> <span>{t("photoSetModified")}<small>{t("photoSetModifiedHint")}</small></span></label>
        <p class="hint">{t("photoCopiesHint")}</p>
      {:else if r.type === "music"}
        <p class="hint top">{t("musicHint")}</p>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("musicTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colon} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.discPrefix} /> <span>{t("musicDiscPrefix")}<small>{t("musicDiscPrefixHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.fromName} /> <span>{t("musicFromName")}<small>{t("musicFromNameHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{t("musicFolders")}<small>{t("musicFoldersHint")}</small></span></label>
        {#if r.folders}
          <label class="sub">{t("musicFolderName")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.playlist} /> <span>{t("musicPlaylist")}<small>{t("musicPlaylistHint")}</small></span></label>
        <p class="hint top">{t("mbHint")}</p>
        <label class="check"><input type="checkbox" bind:checked={r.listen} /> <span>{t("musicListen")}<small>{t("musicListenHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.cover} /> <span>{t("musicCover")}<small>{t("musicCoverHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.writeTags} /> <span>{t("musicWriteTags")}<small>{t("musicWriteTagsHint")}</small></span></label>
        {#if r.writeTags}
          <label class="check sub"><input type="checkbox" bind:checked={r.embedCover} /> <span>{t("musicEmbedCover")}<small>{t("musicEmbedCoverHint")}</small></span></label>
          {#if r.embedCover}
            <label class="sub">{t("musicCoverSize")}
              <select bind:value={r.coverSize}>
                <option value={1200}>{t("musicCoverSize1200")}</option>
                <option value={500}>500 px</option>
                <option value={250}>250 px</option>
              </select>
            </label>
          {/if}
          <label class="check sub"><input type="checkbox" bind:checked={r.cleanTags} /> <span>{t("musicCleanTags")}<small>{t("musicCleanTagsHint")}</small></span></label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{t("musicMissingLabel")}<small>{t("musicMissingHint")}</small></span></label>
        {@render actions(t("mbSearch"), "albumSummary")}
      {:else if r.type === "book"}
        <p class="hint top">{t("bookHint")}</p>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("bookTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colon} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{t("bookFolders")}<small>{t("bookFoldersHint")}</small></span></label>
        {#if r.folders}
          <label class="sub">{t("musicFolderName")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.cover} /> <span>{t("bookCover")}<small>{t("bookCoverHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.opf} /> <span>{t("bookOpf")}<small>{t("bookOpfHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.writeEpub} /> <span>{t("bookWriteEpub")}<small>{t("bookWriteEpubHint")}</small></span></label>
        <label title={t("bookConvertHint")}>{t("bookConvert")}
          <select bind:value={r.convertTo}>
            <option value="">{t("bookConvertNone")}</option>
            <option value="epub">EPUB</option>
            <option value="azw3">{t("bookConvertAzw3")}</option>
            <option value="fb2">FB2</option>
            {#if canPrint || r.convertTo === "pdf"}<option value="pdf">PDF</option>{/if}
          </select>
        </label>
        <p class="hint">{t("bookConvertHint")}</p>
        {#if r.convertTo}
          <label class="sub">{t("comicOriginal")}
            <select bind:value={r.original}>
              <option value="keep">{t("bookOriginalKeep")}</option>
              <option value="delete">{t("comicOriginalDelete")}</option>
            </select>
          </label>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{t("bookMissingLabel")}<small>{t("bookMissingHint")}</small></span></label>
        {@render actions(t("olSearch"), "bookSummary")}
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
            <option value="renome">{t("comicPresetRenome")}</option>
            <option value="kavita">{t("comicPresetKavita")}</option>
          </select>
        </label>
        <p class="hint">{t("comicPresetHint")}</p>
        <label>{t("template")} <input class="mono" bind:value={r.template} spellcheck="false" /></label>
        <p class="hint mono">{t("comicTemplateHint")}</p>
        <label class="colon">{t("colonLabel")} <input class="mono sep" bind:value={r.colon} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={r.byFolder} /> <span>{t("comicByFolder")}<small>{t("comicByFolderHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.folders} /> <span>{t("comicFolders")}<small>{t("comicFoldersHint")}</small></span></label>
        <label>{t("comicSkipPublishers")} <input bind:value={r.skipPublishers} placeholder={t("comicSkipPublishersPlaceholder")} spellcheck="false" /></label>
        <p class="hint">{t("comicSkipPublishersHint")}</p>
        <label class="check"><input type="checkbox" bind:checked={r.writeInfo} /> <span>{t("comicWriteInfo")}<small>{t("comicWriteInfoHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.cleanInfo} /> <span>{t(r.writeInfo ? "comicCleanInfoWrite" : "comicCleanInfo")}<small>{t(r.writeInfo ? "comicCleanInfoWriteHint" : "comicCleanInfoHint")}</small></span></label>
        <label class="check"><input type="checkbox" bind:checked={r.convert} /> <span>{t("comicConvert")}<small>{t("comicConvertHint")}</small></span></label>
        {#if r.convert}
          <label class="sub">{t("comicOriginal")}
            <select bind:value={r.original}>
              <option value="keep">{t("comicOriginalKeep")}</option>
              <option value="delete">{t("comicOriginalDelete")}</option>
            </select>
          </label>
          <label class="sub" title={t("comicDrawPagesHint")}>{t("comicDrawPages")}
            <select bind:value={r.drawPages}>
              <option value={true}>{t("comicDrawPagesYes")}</option>
              <option value={false}>{t("comicDrawPagesNo")}</option>
            </select>
          </label>
          {#if r.drawPages !== false}
            <label class="sub" title={t("comicPageSizeHint")}>{t("comicPageSize")}
              <select bind:value={r.pageSize}>
                <option value={1600}>1600 px</option>
                <option value={2400}>{t("comicPageSize2400")}</option>
                <option value={3200}>3200 px</option>
              </select>
            </label>
          {/if}
        {/if}
        <label class="check"><input type="checkbox" bind:checked={r.missing} /> <span>{t("comicMissingLabel")}<small>{t("comicMissingHint")}</small></span></label>
        {#if r.folders}
          <label class="sub">{t("musicFolderName")} <input class="mono" bind:value={r.folderTemplate} spellcheck="false" /></label>
        {/if}
        <label>{t("comicSource")}
          <select bind:value={r.source}>
            <option value="gcd">{t("comicSourceGcd")}</option>
            <option value="metron">{t("comicSourceMetron")}</option>
            <option value="comicvine">{t("comicSourceCv")}</option>
            <option value="mangadex">{t("comicSourceMangadex")}</option>
            <option value="anilist">{t("comicSourceAnilist")}</option>
            <option value="mangaupdates">{t("comicSourceMangaupdates")}</option>
          </select>
        </label>
        <p class="hint">{t(SOURCE_HINT[r.source] ?? "gcdHint")}</p>
        {#if r.source === "metron"}
          <label class="check"><input type="checkbox" bind:checked={r.barcode} /> <span>{t("comicBarcodeLabel")}<small>{t("comicBarcodeHint")}</small></span></label>
        {/if}
        <label>{t("comicSeries")} <input bind:value={r.series} placeholder={t("comicSeriesPlaceholder")} spellcheck="false" /></label>
        {#if lookup?.show}<p class="hint">{t("seriesCurrent", { show: lookup.show })}</p>{/if}
        {@render actions(t("comicSearch"), "comicSummary")}
      {/if}

      {#if summary}<p class="hint">{summary}</p>{/if}
      {#if error}<p class="error">{error}</p>{/if}
    </fieldset>
  {/if}
</article>

{#snippet subtitles()}
  {#if r.type === "movie" || r.type === "series"}
    <label class="check"><input type="checkbox" bind:checked={r.subtitles} /> <span>{t("subsLabel")}<small>{t("subsHint")}</small></span></label>
    {#if r.subtitles}
      <label class="sub">{t("subsLangs")} <input value={r.subLangs ?? defaultSubLangs()} oninput={(e) => {
        if (r.type === "movie" || r.type === "series") r.subLangs = e.currentTarget.value;
      }} placeholder="pt-BR, en" spellcheck="false" /></label>
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
        <option value="name">{t("targetName")}</option>
        <option value="ext">{t("targetExt")}</option>
        <option value="full">{t("targetFull")}</option>
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
  .chev { display: inline-block; transition: transform 0.15s; color: var(--muted); }
  .chev.closed { transform: rotate(-90deg); }
  .actions { display: flex; gap: 2px; }
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
    font-size: 12px;
    color: var(--muted);
    flex: 1;
    min-width: 0;
  }
  label.narrow { flex: 0 0 80px; }
  label.check {
    flex-direction: row;
    align-items: center;
    gap: 6px;
    color: var(--text);
    flex: 0 1 auto;
  }
  .row { display: flex; gap: 10px; align-items: flex-end; flex-wrap: wrap; }
  .checks { display: flex; flex-direction: column; gap: 4px; }
  .hint { margin: -4px 0 0; font-size: 11px; color: var(--muted); }
  .error { margin: 0; font-size: 12px; color: var(--danger); }
  .sep { white-space: pre; }
  .hint.top { margin: 0; }
  label.colon { flex: none; align-self: flex-start; }
  label.colon input { width: 90px; }
  label.check span { display: flex; flex-direction: column; }
  label.check small { color: var(--muted); font-size: 11px; }
  label.sub { margin-left: 22px; }
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
    font-size: 12px;
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
    font-size: 11px;
  }
  .part span { overflow-wrap: anywhere; }
</style>
