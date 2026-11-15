<script lang="ts">
  // The RomM sign-in dialog (RommStorefront README, "Sign-in dialog";
  // prototype `rmSignInHTML`, `.g-dialog.rmdlg`, 480px): the title and lede
  // for its mode over `RommSignInForm`, in the store dialogs' frame. The RomM
  // card opens it (Connect RomM…, Edit…, Sign in again…), and so does any
  // needs-RomM panel's Sign in again…. On success it pushes the notification
  // and closes; a connect from Settings that needs setup has already turned
  // the phase to Choose your systems by then (`connectRomm`).
  import { untrack } from 'svelte';
  import type { RommStatus } from '../api';
  import StoreDialog from '../gamestores/desktop/StoreDialog.svelte';
  import { pushToast, TOAST_DURATION_MS } from '../stores/toasts.svelte';
  import { romm, type ConnectFrom } from '../stores/romm.svelte';
  import RommSignInForm from './RommSignInForm.svelte';
  import { signedInToast, signInStart } from './signIn';
  import { signInCopy, type SignInMode } from './words';

  let {
    mode,
    from = 'settings',
    onClose,
    onConnected = undefined,
  }: {
    mode: SignInMode;
    from?: ConnectFrom;
    onClose: () => void;
    onConnected?: (status: RommStatus) => void;
  } = $props();

  const start = untrack(() => signInStart(romm.status, mode));
  const copy = untrack(() => signInCopy(mode, start.username, start.useToken));

  function connected(status: RommStatus) {
    pushToast(signedInToast(mode, status), 'success', TOAST_DURATION_MS);
    onConnected?.(status);
    onClose();
  }
</script>

<StoreDialog testId="romm-signin-dialog" label={copy.title} width={480} {onClose}>
  <h3 data-testid="romm-signin-title">{copy.title}</h3>
  <p class="note">{copy.lede}</p>
  <RommSignInForm {mode} {from} {start} onConnected={connected} onCancel={onClose} />
</StoreDialog>
