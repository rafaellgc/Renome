<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "./api";
  import { t, tError } from "./i18n";

  let {
    onclose,
  }: {
    /** "in": signed in; "anonymous": without an account; null: cancelled. */
    onclose: (how: "in" | "anonymous" | null) => void;
  } = $props();

  let email = $state("");
  let password = $state("");
  let busy = $state(false);
  let error = $state("");

  async function signIn(e: SubmitEvent) {
    e.preventDefault();
    if (!email.trim() || !password) return;
    busy = true;
    error = "";
    try {
      await api.gcdSignIn(email.trim(), password);
      password = "";
      onclose("in");
    } catch (err) {
      error = tError(err);
    } finally {
      busy = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="overlay" onclick={() => onclose(null)}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions, a11y_no_noninteractive_element_interactions -->
  <form class="modal login" onclick={(e) => e.stopPropagation()} onsubmit={signIn}>
    <h2>{t("gcdLoginTitle")}</h2>
    <p class="muted">{t("gcdLoginText")}</p>
    <label>{t("gcdEmail")} <input type="email" bind:value={email} autocomplete="username" spellcheck="false" /></label>
    <label>{t("gcdPassword")} <input type="password" bind:value={password} autocomplete="current-password" /></label>
    <p class="muted small">{t("gcdLoginPrivacy")}</p>
    {#if error}<p class="error">{error}</p>{/if}
    <p class="small">
      <button type="button" class="link" onclick={() => openUrl("https://www.comics.org/accounts/register/")}>{t("gcdRegister")}</button>
    </p>
    <div class="modal-foot">
      <button type="button" onclick={() => onclose(null)}>{t("cancel")}</button>
      <span class="spacer"></span>
      <button type="button" onclick={() => onclose("anonymous")}>{t("gcdAnonymous")}</button>
      <button type="submit" class="primary" disabled={busy || !email.trim() || !password}>{t("gcdSignIn")}</button>
    </div>
  </form>
</div>

<style>
  .login { width: min(480px, 94vw); gap: 10px; }
  h2 { margin: 0; font-size: 16px; }
  p { margin: 0; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
  .small { font-size: 12px; }
  .error { color: var(--danger); font-size: 13px; }
  .muted { color: var(--muted); }
  .link { border: none; background: none; height: auto; padding: 0; color: var(--accent); }
  .link:hover { text-decoration: underline; }
  .spacer { flex: 1; }
</style>
