<script lang="ts">
  // A needs-RomM panel (TVRommStorefront README § Cloud saves and
  // RetroAchievements: needs RomM, § Details: needs RomM; prototype `rNeed`):
  // the heading, why plus what still works (desktop's words), and TV's own
  // button: Set up RomM… (opens the RomM page), Sign in again… or Retry
  // ("Retrying…" while it runs). Warning-edged unless RomM is simply not
  // connected. Its button is focused first where it shows (`autofocus`).
  import { romm } from '../../stores/romm.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import { openRommPage, openSignIn, retryRomm } from './rommActions';
  import { needsPanel } from './rommTv';

  let { title, text, k = 'rneed', autofocus = true, testId = 'tv-needs-romm' }: { title: string; text: string; k?: string; autofocus?: boolean; testId?: string } = $props();

  let panel = $derived(needsPanel(romm.status, title, text));

  function act() {
    const a = panel?.action;
    if (!a || a.disabled) return;
    if (a.kind === 'page') openRommPage();
    else if (a.kind === 'signin') openSignIn('again');
    else void retryRomm();
  }
</script>

{#if panel}
  <div class="rneed" class:warn={panel.warn} data-testid={testId}>
    <div class="rneed-t"><b>{panel.title}</b><span data-testid={`${testId}-text`}>{panel.text}</span></div>
    <TvButton k={k} al={panel.action.al} {autofocus} disabled={panel.action.disabled} why={panel.action.why} testId={`${testId}-act`} onA={act}>
      {#if panel.action.disabled}<span class="sspin"></span>{/if}{panel.action.label}
    </TvButton>
  </div>
{/if}
