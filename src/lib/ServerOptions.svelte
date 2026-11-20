<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import Icon from "./Icon.svelte";
  import { api } from "./api";
  import { TABLE } from "./rules";
  import { t, tError } from "./i18n";
  import { secrets, settings } from "./settings.svelte";
  import type { MediaServer, ServerFolder } from "./types";

  /** The keys, by server id (kept as one secret; read and written as the
   *  command line does: `mediaserver::keys_of`, `with_key`). */
  let keyMap = $state<Record<string, string>>({});
  $effect(() => {
    const raw = settings.serverKeys || "";
    api.serverKeys(raw).then((k) => (keyMap = k), () => {});
  });
  const keys = () => keyMap;
  async function setKey(id: string, key: string) {
    keyMap = { ...keyMap, [id]: key };
    settings.serverKeys = await api.serverKeyWith(settings.serverKeys || "", id, key);
  }

  let show = $state<Record<string, boolean>>({});
  let status = $state<Record<string, { ok: boolean; text: string } | null>>({});
  let reading = $state<Record<string, boolean>>({});

  async function add() {
    settings.servers = [...settings.servers, await api.serverNew()];
  }

  async function remove(s: MediaServer) {
    await setKey(s.id, "");
    settings.servers = settings.servers.filter((x) => x.id !== s.id);
  }

  /** Reads the server's libraries; folders already paired keep their pair
   *  and whether they are on (the shared pairing, the command line's too). */
  async function read(s: MediaServer) {
    reading[s.id] = true;
    status[s.id] = null;
    try {
      const [read, libs, folders] = await api.serverRead($state.snapshot(s), (keys()[s.id] ?? "").trim());
      s.folders = read.folders;
      status[s.id] = { ok: true, text: t("serverRead", { libs, folders }) };
    } catch (e) {
      status[s.id] = { ok: false, text: tError(e) };
    } finally {
      reading[s.id] = false;
    }
  }

  /** Long paths are cut at the start (the column is right-to-left); the
   *  marks keep the slashes where they belong. */
  const lr = (p: string) => `\u200e${p}\u200e`;

  async function pick(f: ServerFolder) {
    const dir = await open({ directory: true, defaultPath: f.local || undefined });
    if (typeof dir === "string") f.local = dir;
  }
</script>

<section>
  <h3>{t("sectionServer")}</h3>
  <p class="hint-top"><small>{t("serverHint")}</small></p>

  {#each settings.servers as s (s.id)}
    {@const key = keys()[s.id] ?? ""}
    <div class="server">
      <div class="grid">
        <label for="srv-kind-{s.id}">{t("serverKind")}</label>
        <div class="row">
          <select id="srv-kind-{s.id}" bind:value={s.kind} onchange={() => { s.folders = []; status[s.id] = null; }}>
            {#each TABLE.options.serverKinds as [kind, name] (kind)}<option value={kind}>{name}</option>{/each}
          </select>
          <span class="grow"></span>
          <button class="icon" title={t("serverRemove")} aria-label={t("serverRemove")} onclick={() => remove(s)}><Icon name="x" size={14} /></button>
        </div>
        <label for="srv-url-{s.id}">{t("serverUrl")}</label>
        <input
          id="srv-url-{s.id}"
          bind:value={s.url}
          oninput={() => (status[s.id] = null)}
          placeholder={s.kind === "plex" ? "http://192.168.0.10:32400" : "http://192.168.0.10:8096"}
          spellcheck="false"
          autocomplete="off"
        />
        <label for="srv-key-{s.id}">{t(s.kind === "plex" ? "serverToken" : "tmdbKey")}</label>
        <div class="key">
          <input
            id="srv-key-{s.id}"
            type={show[s.id] ? "text" : "password"}
            value={key}
            oninput={(e) => { setKey(s.id, e.currentTarget.value); status[s.id] = null; }}
            disabled={!secrets.ready}
            spellcheck="false"
            autocomplete="off"
          />
          <button onclick={() => (show[s.id] = !show[s.id])}>{t(show[s.id] ? "hide" : "show")}</button>
          <button class="primary" disabled={!s.url.trim() || !key.trim() || reading[s.id]} onclick={() => read(s)}>{t(s.folders.length ? "serverReread" : "serverReadBtn")}</button>
        </div>
        <span></span>
        <small>
          {#if status[s.id]}{@const st = status[s.id]!}<b class:good={st.ok} class:bad={!st.ok}>{st.text}</b> · {/if}
          {t(s.kind === "plex" ? "serverPlexHint" : "serverJellyfinHint")}
        </small>
      </div>

      {#if s.folders.length}
        <p class="folders-hint"><small>{t("serverFoldersHint")}</small></p>
        <ul class="folders">
          {#each s.folders as f (f.libraryId + f.server)}
            <li class:off={!f.on}>
              <input type="checkbox" bind:checked={f.on} aria-label={t("serverFolderOn")} title={t("serverFolderOn")} />
              <div class="where">
                <span class="lib">{f.library}</span>
                <span class="path" title={`${t("serverFolderServer")}: ${f.server}`}>{lr(f.server)}</span>
                {#if f.local}
                  {#if f.local !== f.server}<span class="path here" title={`${t("serverFolderHere")}: ${f.local}`}>{lr(`↳ ${f.local}`)}</span>{/if}
                {:else}
                  <span class="missing">{t("serverFolderMissing")}</span>
                {/if}
              </div>
              <button onclick={() => pick(f)}>{t(f.local ? "serverFolderChange" : "serverFolderPick")}</button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/each}

  <div><button onclick={add}><Icon name="plus" size={14} />{t("serverAdd")}</button></div>
</section>

<style>
  section { display: flex; flex-direction: column; gap: 12px; }
  h3 { display: none; }
  .hint-top { margin: 0; }
  small { color: var(--muted); font-size: var(--fs-sm); }
  .server { min-width: 0; display: flex; flex-direction: column; gap: 10px; padding: 12px; border: 1px solid var(--border); border-radius: 8px; }
  .grid { display: grid; grid-template-columns: 140px minmax(0, 1fr); align-items: center; gap: 10px; }
  .grid input { min-width: 0; }
  button { display: inline-flex; align-items: center; gap: 6px; white-space: nowrap; }
  .row { display: flex; align-items: center; gap: 6px; }
  .grow { flex: 1; }
  .key { display: flex; gap: 6px; }
  .key input { flex: 1; min-width: 0; }
  .good { color: var(--success); }
  .bad { color: var(--danger); }
  .folders-hint { margin: 0; }
  .folders { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; border-top: 1px solid var(--row-line); }
  .folders li { display: flex; align-items: center; gap: 10px; padding: 6px 2px; border-bottom: 1px solid var(--row-line); }
  .folders li.off .where { opacity: 0.55; }
  .where { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .lib { font-weight: 600; font-size: var(--fs-md); }
  .path { font: var(--fs-sm) ui-monospace, monospace; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; direction: rtl; text-align: left; }
  .path.here { color: var(--text); }
  .missing { font-size: var(--fs-sm); color: var(--warn-text); }
</style>
