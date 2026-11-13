<script lang="ts">
  // The store's sign-in over TV mode (TVStores README § Signing in;
  // prototype `sLogin`). The store's own page is the backend's login window
  // (its own webview, with no access to GRID), which opens over this layer:
  // GRID's frame for it — the title bar "Sign in to GOG — GRID", the
  // read-only address with a lock, the page while it loads, and the footer
  // with Cancel. The layer carries the hint bar (A Select · Y Keyboard · B
  // Cancel sign-in); Y opens the on-screen keyboard, whose Done types into
  // the store's page (`store_login_type`). B cancels silently. The layer
  // closes when the `store-login` event says how it ended.
  import type { StoreSource } from '../../api';
  import { storeName } from '../../gamestores/marks';
  import Glyph from '../glyphs/Glyph.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import TvStoreMark from './TvStoreMark.svelte';
  import { LOGIN_FOOT, loginHost } from './storeWords';

  let { source, onCancel }: { source: StoreSource; onCancel: () => void } = $props();

  let name = $derived(storeName(source));
  let host = $derived(loginHost(source));
</script>

<div class="swin" role="dialog" aria-label={`Sign in to ${name}`} data-testid="tv-store-login">
  <div class="swin-bar"><TvStoreMark {source} />Sign in to {name} — GRID</div>
  <div class="swin-url"><span class="lock">🔒</span> https://{host}</div>
  <div class="swin-page">
    <div class="swin-ph">
      <span><span class="sspin"></span> Loading {host.split('/')[0]}…</span>
      <span>The store’s page loads here, as in a browser. Move between its fields with the d-pad; <Glyph button="Y" /> opens the keyboard. GRID never sees what you type.</span>
    </div>
  </div>
  <div class="swin-foot" data-nav="h">
    <span class="note">{LOGIN_FOOT}</span>
    <TvButton k="lf-x" al="Cancel" testId="tv-store-login-cancel" onA={onCancel}>Cancel</TvButton>
  </div>
</div>
